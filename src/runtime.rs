use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64};
use tokio::sync::mpsc;
use futures::StreamExt;
use tracing::{info};
use std::cmp::Ordering;
use std::time::Duration;

use libp2p::{
    Swarm,
    PeerId as LibPeerId,
    swarm::SwarmEvent,
    gossipsub,
    mdns,
    identify,
    request_response,
    Multiaddr
};

use crate::behaviour::{AviBehaviour, AviBehaviourEvent};
use crate::command::Command;
use crate::events::{AviEvent, PeerId, StreamId};
use crate::error::{AviP2pError, StreamCloseReason};
use crate::protocols::stream::StreamMessage;
use crate::context::{AviContext};

static NEXT_STREAM_ID: AtomicU64 = AtomicU64::new(1);

fn generate_stream_id() -> StreamId {
    StreamId(NEXT_STREAM_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
}

struct PeerState {
    #[allow(dead_code)]
    addr: Option<String>,
}

enum StreamStatus {
    Requested,
    #[allow(dead_code)]
    Accepted,
    #[allow(dead_code)]
    Active,
}

struct StreamState {
    peer: LibPeerId,
    #[allow(dead_code)]
    status: StreamStatus,
    #[allow(dead_code)]
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
    started: bool,
    discovered_peers: HashSet<LibPeerId>,
    context_store: HashMap<String, AviContext>,
    local_context: AviContext,

    known_peers: HashMap<LibPeerId, Multiaddr>,
}

impl Runtime {
    pub fn new(
        swarm: Swarm<AviBehaviour>,
        command_rx: mpsc::Receiver<Command>,
        event_tx: mpsc::Sender<AviEvent>,
    ) -> Self {
        let local_peer_id = swarm.local_peer_id().to_string();
        let local_context = AviContext::new(local_peer_id);

        Self {
            swarm,
            command_rx,
            event_tx,
            peers: HashMap::new(),
            streams: HashMap::new(),
            topics: HashSet::new(),
            started: false,
            discovered_peers: HashSet::new(),

            // New fields
            context_store: HashMap::new(),
            local_context,
            known_peers: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        let mut heartbeat = tokio::time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    for (peer_id, addr) in &self.known_peers {
                        if !self.swarm.is_connected(peer_id) {
                            let _ = self.swarm.dial(addr.clone());
                        }
                    }
                }

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
        // (This section remains exactly the same as before)
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
            Command::RequestStream { peer_id, respond_to } => {
                let res = if let Ok(target) = LibPeerId::try_from(peer_id.clone()) {
                    let id = generate_stream_id();
                    self.streams.insert(id.0, StreamState {
                        peer: target,
                        status: StreamStatus::Requested,
                        direction: StreamDirection::Outbound,
                    });
                    self.swarm.behaviour_mut().stream.send_request(&target, StreamMessage::RequestStream { stream_id: id.0 });
                    Ok(id)
                } else {
                    Err(AviP2pError::PeerNotFound(peer_id))
                };
                let _ = respond_to.send(res);
            }
            Command::AcceptStream { stream_id, respond_to } => {
                let res = if let Some(state) = self.streams.get_mut(&stream_id.0) {
                    state.status = StreamStatus::Accepted;
                    self.swarm.behaviour_mut().stream.send_request(
                        &state.peer,
                        StreamMessage::AcceptStream { stream_id: stream_id.0 }
                    );
                    Ok(())
                } else {
                    Err(AviP2pError::StreamNotFound(stream_id))
                };
                let _ = respond_to.send(res);
            }
            Command::SendStreamData { stream_id, data, respond_to } => {
                let res = if let Some(state) = self.streams.get(&stream_id.0) {
                    self.swarm.behaviour_mut().stream.send_request(
                        &state.peer,
                        StreamMessage::StreamData { stream_id: stream_id.0, data }
                    );
                    Ok(())
                } else {
                    Err(AviP2pError::StreamNotFound(stream_id))
                };
                let _ = respond_to.send(res);
            }
            Command::CloseStream { stream_id, respond_to } => {
                let res = if let Some(state) = self.streams.remove(&stream_id.0) {
                    self.swarm.behaviour_mut().stream.send_request(
                        &state.peer,
                        StreamMessage::CloseStream { stream_id: stream_id.0 }
                    );
                    Ok(())
                } else {
                    Err(AviP2pError::StreamNotFound(stream_id))
                };
                let _ = respond_to.send(res);
            }
            Command::GetConnectedPeers { respond_to } => {
                let peers = self.peers.keys()
                    .map(|p| PeerId::from(*p))
                    .collect();
                let _ = respond_to.send(Ok(peers));
            }
            Command::DiscoverPeers { respond_to } => {
                let _ = self.swarm.behaviour_mut().kad.bootstrap();
                let _ = respond_to.send(Ok(()));
            }
            Command::Shutdown { respond_to } => {
                let _ = respond_to.send(Ok(()));
                self.command_rx.close();
            }

            Command::UpdateSelfContext { patch, respond_to } => {
                self.local_context.apply_patch(patch);

                let my_id = self.local_context.device_id.clone();
                self.local_context.vector_clock.increment(&my_id);

                // 3. Broadcast to Mesh
                if let Ok(data) = serde_json::to_vec(&self.local_context) {
                    let topic = gossipsub::IdentTopic::new("avi-context-updates");
                    if !self.topics.contains("avi-context-updates") {
                        let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
                        self.topics.insert("avi-context-updates".to_string());
                    }

                    match self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                        Ok(_) => {}, // Success
                        Err(gossipsub::PublishError::InsufficientPeers) => {
                           info!("Context updated locally (broadcasting postponed: no peers yet)");
                        },
                        Err(e) => {
                            let _ = respond_to.send(Err(AviP2pError::NetworkError(e.to_string())));
                            return;
                        }
                    }
                }

                let _ = respond_to.send(Ok(()));
            }

            Command::GetPeerContext { peer_id, respond_to } => {
                let result = if let Some(pid) = peer_id {
                    // Get remote peer context
                    self.context_store.get(pid.as_str())
                        .map(|c| c.data.clone())
                        .ok_or(AviP2pError::PeerNotFound(pid))
                } else {
                    // Get self context
                    Ok(self.local_context.data.clone())
                };
                let _ = respond_to.send(result);
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<AviBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                if !self.started {
                    self.started = true;
                    let local_peer_id = *self.swarm.local_peer_id();
                    let _ = self.event_tx.send(AviEvent::Started {
                        local_peer_id: PeerId::from(local_peer_id),
                        listen_addresses: vec![address.to_string()],
                    }).await;
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            SwarmEvent::Behaviour(AviBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, multiaddr) in list {
                    self.swarm.behaviour_mut().kad.add_address(&peer_id, multiaddr.clone());
                    self.known_peers.insert(peer_id, multiaddr.clone());

                    if !self.swarm.is_connected(&peer_id) {
                        if let Err(_e) = self.swarm.dial(multiaddr) {}
                    }

                    self.emit_peer_discovered(peer_id).await;
                }
            }

            SwarmEvent::Behaviour(AviBehaviourEvent::Identify(identify::Event::Received { peer_id, info })) => {
                self.emit_peer_discovered(peer_id).await;
                for addr in info.listen_addrs {
                    self.swarm.behaviour_mut().kad.add_address(&peer_id, addr.clone());

                    self.known_peers.insert(peer_id, addr);
                }
            }

            // Connection ESTABLISHED
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, num_established, .. } => {
                if num_established.get() == 1 {
                    let addr = match endpoint {
                        libp2p::core::ConnectedPoint::Dialer { address, .. } => address.to_string(),
                        libp2p::core::ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr.to_string(),
                    };

                    self.peers.insert(peer_id, PeerState { addr: Some(addr.clone()) });

                    let topic = gossipsub::IdentTopic::new("avi-context-updates");
                    if !self.topics.contains("avi-context-updates") {
                        let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
                        self.topics.insert("avi-context-updates".to_string());
                    }

                    let _ = self.event_tx.send(AviEvent::PeerConnected {
                        peer_id: PeerId::from(peer_id),
                        address: addr,
                    }).await;
                }
            }

            // Connection CLOSED
            SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
                if num_established == 0 {
                    self.peers.remove(&peer_id);
                    self.discovered_peers.remove(&peer_id);

                    let ids_to_remove: Vec<u64> = self.streams.iter()
                        .filter(|(_, state)| state.peer == peer_id)
                        .map(|(id, _)| *id)
                        .collect();

                    for id in ids_to_remove {
                        self.streams.remove(&id);
                        let _ = self.event_tx.send(AviEvent::StreamClosed {
                            peer_id: PeerId::from(peer_id),
                            stream_id: StreamId(id),
                            reason: StreamCloseReason::RemoteClose,
                        }).await;
                    }

                    let _ = self.event_tx.send(AviEvent::PeerDisconnected {
                        peer_id: PeerId::from(peer_id),
                    }).await;
                }
            }

            SwarmEvent::Behaviour(AviBehaviourEvent::Gossipsub(gossipsub::Event::Message { propagation_source, message, .. })) => {
                let topic = message.clone().topic.into_string();

                if topic == "avi-context-updates" {
                    if let Ok(incoming_ctx) = serde_json::from_slice::<AviContext>(&message.data) {
                        let peer_id_str = incoming_ctx.device_id.clone();

                        let should_update = match self.context_store.get(&peer_id_str) {
                            Some(existing_ctx) => {
                                match incoming_ctx.vector_clock.partial_cmp(&existing_ctx.vector_clock) {
                                    Some(Ordering::Greater) => true, // Incoming is newer
                                    None => {
                                     incoming_ctx.timestamp > existing_ctx.timestamp
                                    }
                                    _ => false
                                }
                            },
                            None => true
                        };

                        if should_update {
                            // Update Store
                            self.context_store.insert(peer_id_str.clone(), incoming_ctx.clone());

                            // Notify User
                            let _ = self.event_tx.send(AviEvent::ContextUpdated {
                                peer_id: PeerId::new(&peer_id_str),
                                context: incoming_ctx.data,
                            }).await;
                        }
                    }
                    return;
                }

                let _ = self.event_tx.send(AviEvent::Message {
                    from: PeerId::from(propagation_source),
                    topic: message.topic.into_string(),
                    data: message.data,
                }).await;
            }
            SwarmEvent::Behaviour(AviBehaviourEvent::Stream(request_response::Event::Message { peer, message })) => {
                match message {
                    request_response::Message::Request { request, .. } => {
                        self.handle_stream_message(peer, request).await;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    async fn handle_stream_message(&mut self, peer: LibPeerId, msg: StreamMessage) {
        let peer_wrap = PeerId::from(peer);
        match msg {
            StreamMessage::RequestStream { stream_id } => {
                self.streams.insert(stream_id, StreamState {
                    peer,
                    status: StreamStatus::Requested,
                    direction: StreamDirection::Inbound,
                });
                let _ = self.event_tx.send(AviEvent::StreamRequested {
                    from: peer_wrap,
                    stream_id: StreamId(stream_id),
                }).await;
            }
            StreamMessage::AcceptStream { stream_id } => {
                if let Some(state) = self.streams.get_mut(&stream_id) {
                    state.status = StreamStatus::Active;
                    let _ = self.event_tx.send(AviEvent::StreamAccepted {
                        peer_id: peer_wrap,
                        stream_id: StreamId(stream_id),
                    }).await;
                }
            }
            StreamMessage::StreamData { stream_id, data } => {
                if self.streams.contains_key(&stream_id) {
                    let _ = self.event_tx.send(AviEvent::StreamData {
                        from: peer_wrap,
                        stream_id: StreamId(stream_id),
                        data,
                    }).await;
                }
            }
            StreamMessage::CloseStream { stream_id } => {
                self.streams.remove(&stream_id);
                let _ = self.event_tx.send(AviEvent::StreamClosed {
                    peer_id: peer_wrap,
                    stream_id: StreamId(stream_id),
                    reason: StreamCloseReason::RemoteClose,
                }).await;
            }
            _ => {}
        }
    }

    async fn emit_peer_discovered(&mut self, peer_id: LibPeerId) {
        if self.discovered_peers.contains(&peer_id) {
            return;
        }

        self.discovered_peers.insert(peer_id);
        let _ = self.event_tx.send(AviEvent::PeerDiscovered {
            peer_id: PeerId::from(peer_id),
        }).await;
    }
}