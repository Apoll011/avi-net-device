use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::str::FromStr;

use tokio::sync::{mpsc};
use futures::StreamExt;
use tracing::{info};

use libp2p::{
    Swarm,
    PeerId as LibPeerId,
    swarm::SwarmEvent,
    gossipsub,
    mdns,
    identify,
    request_response,
};

use crate::behaviour::{AviBehaviour, AviBehaviourEvent};
use crate::command::Command;
use crate::events::{AviEvent, PeerId, StreamId};
use crate::error::{AviP2pError, StreamCloseReason};
use crate::protocols::audio::AudioStreamMessage;

static NEXT_STREAM_ID: AtomicU64 = AtomicU64::new(1);

fn generate_stream_id() -> StreamId {
    StreamId(NEXT_STREAM_ID.fetch_add(1, Ordering::SeqCst))
}

struct PeerState {
    addr: Option<String>,
}

enum StreamStatus {
    Requested,
    Accepted,
    Active,
}

struct StreamState {
    peer: LibPeerId,
    #[allow(dead_code)] // Used for logic checks
    status: StreamStatus,
    direction: StreamDirection,
}

#[derive(PartialEq)]
enum StreamDirection {
    Inbound,
    Outbound,
}

pub struct Runtime {
    swarm: Swarm<AviBehaviour>,
    command_rx: mpsc::Receiver<Command>,
    event_tx: mpsc::Sender<AviEvent>,

    // State
    peers: HashMap<LibPeerId, PeerState>,
    streams: HashMap<u64, StreamState>,
    topics: HashSet<String>,
}

impl Runtime {
    pub fn new(
        swarm: Swarm<AviBehaviour>,
        command_rx: mpsc::Receiver<Command>,
        event_tx: mpsc::Sender<AviEvent>,
    ) -> Self {
        Self {
            swarm,
            command_rx,
            event_tx,
            peers: HashMap::new(),
            streams: HashMap::new(),
            topics: HashSet::new(),
        }
    }

    pub async fn run(mut self) {
        // Announce start
        let local_peer_id = *self.swarm.local_peer_id();
        let listeners: Vec<String> = self.swarm.listeners().map(|a| a.to_string()).collect();

        let _ = self.event_tx.send(AviEvent::Started {
            local_peer_id: PeerId::from(local_peer_id).into(),
            listen_addresses: listeners,
        }).await;

        // Main Loop
        loop {
            tokio::select! {
                cmd = self.command_rx.recv() => {
                    match cmd {
                        Some(c) => self.handle_command(c).await,
                        None => {
                            info!("Command channel closed, shutting down");
                            break;
                        }
                    }
                }
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::Subscribe { topic, respond_to } => {
                let topic_hash = gossipsub::IdentTopic::new(&topic);
                let res = match self.swarm.behaviour_mut().gossipsub.subscribe(&topic_hash) {
                    Ok(_) => {
                        self.topics.insert(topic);
                        Ok(())
                    },
                    Err(e) => Err(AviP2pError::NetworkError(e.to_string())),
                };
                let _ = respond_to.send(res);
            }
            Command::Unsubscribe { topic, respond_to } => {
                let topic_hash = gossipsub::IdentTopic::new(&topic);
                let res = match self.swarm.behaviour_mut().gossipsub.unsubscribe(&topic_hash) {
                    Ok(_) => {
                        self.topics.remove(&topic);
                        Ok(())
                    },
                    Err(e) => Err(AviP2pError::NetworkError(e.to_string())),
                };
                let _ = respond_to.send(res);
            }
            Command::Publish { topic, data, respond_to } => {
                let topic_hash = gossipsub::IdentTopic::new(&topic);
                let res = match self.swarm.behaviour_mut().gossipsub.publish(topic_hash, data) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(AviP2pError::NetworkError(e.to_string())),
                };
                let _ = respond_to.send(res);
            }
            Command::RequestAudioStream { peer_id, respond_to } => {
                let res = if let Ok(target) = LibPeerId::from_str(peer_id.as_str()) {
                    let id = generate_stream_id();
                    // Track internal state
                    self.streams.insert(id.0, StreamState {
                        peer: target,
                        status: StreamStatus::Requested,
                        direction: StreamDirection::Outbound,
                    });

                    // Send request packet
                    self.swarm.behaviour_mut().audio.send_request(&target, AudioStreamMessage::RequestStream { stream_id: id.0 });
                    Ok(id)
                } else {
                    Err(AviP2pError::PeerNotFound(peer_id))
                };
                let _ = respond_to.send(res);
            }
            Command::AcceptAudioStream { stream_id, respond_to } => {
                let res = if let Some(state) = self.streams.get_mut(&stream_id.0) {
                    state.status = StreamStatus::Accepted;
                    self.swarm.behaviour_mut().audio.send_request(
                        &state.peer,
                        AudioStreamMessage::AcceptStream { stream_id: stream_id.0 }
                    );
                    Ok(())
                } else {
                    Err(AviP2pError::StreamNotFound(stream_id))
                };
                let _ = respond_to.send(res);
            }
            Command::SendAudio { stream_id, data, respond_to } => {
                let res = if let Some(state) = self.streams.get(&stream_id.0) {
                    self.swarm.behaviour_mut().audio.send_request(
                        &state.peer,
                        AudioStreamMessage::AudioData { stream_id: stream_id.0, data }
                    );
                    Ok(())
                } else {
                    Err(AviP2pError::StreamNotFound(stream_id))
                };
                let _ = respond_to.send(res);
            }
            Command::CloseAudioStream { stream_id, respond_to } => {
                let res = if let Some(state) = self.streams.remove(&stream_id.0) {
                    self.swarm.behaviour_mut().audio.send_request(
                        &state.peer,
                        AudioStreamMessage::CloseStream { stream_id: stream_id.0 }
                    );
                    Ok(())
                } else {
                    Err(AviP2pError::StreamNotFound(stream_id))
                };
                let _ = respond_to.send(res);
            }
            Command::GetConnectedPeers { respond_to } => {
                let peers = self.peers.keys()
                    .map(|p| PeerId::new(&p.to_string()))
                    .collect();
                let _ = respond_to.send(Ok(peers));
            }
            Command::DiscoverPeers { respond_to } => {
                let _ = self.swarm.behaviour_mut().kad.bootstrap();
                let _ = respond_to.send(Ok(()));
            }
            Command::Shutdown { respond_to } => {
                // In a real impl, we might gracefully close connections
                let _ = respond_to.send(Ok(()));
                // Allow the select! loop to exit
                self.command_rx.close();
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<AviBehaviourEvent>) {
        match event {
            SwarmEvent::Behaviour(AviBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, multiaddr) in list {
                    self.swarm.behaviour_mut().kad.add_address(&peer_id, multiaddr);
                    self.emit_peer_discovered(peer_id).await;
                }
            }
            SwarmEvent::Behaviour(AviBehaviourEvent::Identify(identify::Event::Received { peer_id, info })) => {
                self.emit_peer_discovered(peer_id).await;
                // Add addresses to Kademlia
                for addr in info.listen_addrs {
                    self.swarm.behaviour_mut().kad.add_address(&peer_id, addr);
                }
            }
            SwarmEvent::Behaviour(AviBehaviourEvent::Gossipsub(gossipsub::Event::Message { propagation_source, message, .. })) => {
                let _ = self.event_tx.send(AviEvent::Message {
                    from: PeerId::new(&propagation_source.to_string()),
                    topic: message.topic.into_string(),
                    data: message.data,
                }).await;
            }
            SwarmEvent::Behaviour(AviBehaviourEvent::Audio(request_response::Event::Message { peer, message })) => {
                match message {
                    request_response::Message::Request { request, .. } => {
                        self.handle_audio_message(peer, request).await;
                    }
                    _ => {} // We don't use responses in this fire-and-forget logic
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                let addr = match endpoint {
                    libp2p::core::ConnectedPoint::Dialer { address, .. } => address.to_string(),
                    libp2p::core::ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr.to_string(),
                };

                self.peers.insert(peer_id, PeerState { addr: Some(addr.clone()) });

                let _ = self.event_tx.send(AviEvent::PeerConnected {
                    peer_id: PeerId::new(&peer_id.to_string()),
                    address: addr,
                }).await;
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                self.peers.remove(&peer_id);
                // Clean up streams associated with this peer
                let ids_to_remove: Vec<u64> = self.streams.iter()
                    .filter(|(_, state)| state.peer == peer_id)
                    .map(|(id, _)| *id)
                    .collect();

                for id in ids_to_remove {
                    self.streams.remove(&id);
                    let _ = self.event_tx.send(AviEvent::AudioStreamClosed {
                        peer_id: PeerId::new(&peer_id.to_string()),
                        stream_id: StreamId(id),
                        reason: StreamCloseReason::RemoteClose,
                    }).await;
                }

                let _ = self.event_tx.send(AviEvent::PeerDisconnected {
                    peer_id: PeerId::new(&peer_id.to_string()),
                }).await;
            }
            _ => {}
        }
    }

    async fn handle_audio_message(&mut self, peer: LibPeerId, msg: AudioStreamMessage) {
        let peer_wrap = PeerId::new(&peer.to_string());

        match msg {
            AudioStreamMessage::RequestStream { stream_id } => {
                // Inbound stream request
                self.streams.insert(stream_id, StreamState {
                    peer,
                    status: StreamStatus::Requested,
                    direction: StreamDirection::Inbound,
                });

                let _ = self.event_tx.send(AviEvent::AudioStreamRequested {
                    from: peer_wrap,
                    stream_id: StreamId(stream_id),
                }).await;
            }
            AudioStreamMessage::AcceptStream { stream_id } => {
                if let Some(state) = self.streams.get_mut(&stream_id) {
                    state.status = StreamStatus::Active;
                    let _ = self.event_tx.send(AviEvent::AudioStreamAccepted {
                        peer_id: peer_wrap,
                        stream_id: StreamId(stream_id),
                    }).await;
                }
            }
            AudioStreamMessage::AudioData { stream_id, data } => {
                // Only forward data if we know about the stream
                if self.streams.contains_key(&stream_id) {
                    let _ = self.event_tx.send(AviEvent::AudioData {
                        from: peer_wrap,
                        stream_id: StreamId(stream_id),
                        data,
                    }).await;
                }
            }
            AudioStreamMessage::CloseStream { stream_id } => {
                self.streams.remove(&stream_id);
                let _ = self.event_tx.send(AviEvent::AudioStreamClosed {
                    peer_id: peer_wrap,
                    stream_id: StreamId(stream_id),
                    reason: StreamCloseReason::RemoteClose,
                }).await;
            }
            _ => {}
        }
    }

    async fn emit_peer_discovered(&self, peer_id: LibPeerId) {
        let _ = self.event_tx.send(AviEvent::PeerDiscovered {
            peer_id: PeerId::new(&peer_id.to_string()),
        }).await;
    }
}