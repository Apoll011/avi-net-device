use tokio::sync::{mpsc, oneshot};
use crate::config::AviP2pConfig;
use crate::error::AviP2pError;
use crate::events::{AviEvent, PeerId, StreamId};
use crate::command::Command;
use crate::behaviour::AviBehaviour;
use crate::runtime::Runtime;

use libp2p::{
    gossipsub,
    tcp,
    noise,
    yamux,
    Multiaddr,
    SwarmBuilder,
    identity::Keypair,
};
use std::time::Duration;
use std::str::FromStr;
use serde_json::Value;

/// Main entry point for the AVI P2P node.
pub struct AviP2p {
    handle: AviP2pHandle,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

/// Cloneable handle for interacting with the P2P node.
#[derive(Clone)]
pub struct AviP2pHandle {
    command_tx: mpsc::Sender<Command>,
}

impl AviP2p {
    /// Create and start the P2P node.
    pub async fn start(config: AviP2pConfig)
                       -> Result<(AviP2p, mpsc::Receiver<AviEvent>), AviP2pError>
    {
        // 1. Setup keys and identity
        let local_key = Keypair::generate_ed25519();

        // 2. Setup Transport
        let swarm = SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            ).map_err(|e| AviP2pError::NetworkError(e.to_string()))?
            .with_dns().map_err(|e| AviP2pError::NetworkError(e.to_string()))?
            .with_behaviour(|key| {
                let gossip_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(1))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .max_transmit_size(1024 * 1024)
                    .build()
                    .expect("Valid gossipsub config");

                AviBehaviour::new(
                    key.clone(),
                    gossip_config,
                    config.node_name.clone(),
                )
            })
            .map_err(|e| AviP2pError::NetworkError(e.to_string()))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();


        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.listen_port)
            .parse()
            .map_err(|e: libp2p::multiaddr::Error| AviP2pError::NetworkError(e.to_string()))?;

        let mut swarm = swarm;
        swarm.listen_on(listen_addr)
            .map_err(|e| AviP2pError::NetworkError(e.to_string()))?;

        for addr_str in config.bootstrap_peers {
            if let Ok(ma) = Multiaddr::from_str(&addr_str) {
                if let Some(peer_id) = extract_peer_id_from_multiaddr(&ma) {
                    swarm.behaviour_mut().kad.add_address(&peer_id, ma.clone());
                }


                if let Err(e) = swarm.dial(ma) {
                    eprintln!("Warning: Failed to dial bootstrap peer: {}", e);
                }
            }
        }

        // 5. Setup Channels
        let (command_tx, command_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // 6. Spawn Runtime
        let runtime = Runtime::new(swarm, command_rx, event_tx);
        tokio::spawn(async move {
            tokio::select! {
                _ = runtime.run() => {},
                _ = shutdown_rx => {}
            }
        });

        let node = AviP2p {
            handle: AviP2pHandle { command_tx },
            shutdown_tx: Some(shutdown_tx),
        };

        Ok((node, event_rx))
    }

    pub fn handle(&self) -> AviP2pHandle {
        self.handle.clone()
    }

    pub async fn shutdown(mut self) -> Result<(), AviP2pError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        Ok(())
    }
}

impl AviP2pHandle {
    pub async fn subscribe(&self, topic: &str) -> Result<(), AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::Subscribe { topic: topic.to_string(), respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn unsubscribe(&self, topic: &str) -> Result<(), AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::Unsubscribe { topic: topic.to_string(), respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn publish(&self, topic: &str, data: Vec<u8>) -> Result<(), AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::Publish { topic: topic.to_string(), data, respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn request_stream(&self, peer_id: PeerId) -> Result<StreamId, AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::RequestStream { peer_id, respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn accept_stream(&self, stream_id: StreamId) -> Result<(), AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::AcceptStream { stream_id, respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn send_stream_data(&self, stream_id: StreamId, data: Vec<u8>) -> Result<(), AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::SendStreamData { stream_id, data, respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn close_stream(&self, stream_id: StreamId) -> Result<(), AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::CloseStream { stream_id, respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn connected_peers(&self) -> Result<Vec<PeerId>, AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::GetConnectedPeers { respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn discover_peers(&self) -> Result<(), AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::DiscoverPeers { respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    pub async fn update_context(&self, patch: Value) -> Result<(), AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::UpdateSelfContext { patch, respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }

    /// Get the context of a specific peer, or local context if None.
    pub async fn get_context(&self, peer_id: Option<PeerId>) -> Result<Value, AviP2pError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(Command::GetPeerContext { peer_id, respond_to: tx })
            .await.map_err(|_| AviP2pError::ChannelClosed)?;
        rx.await.map_err(|_| AviP2pError::ChannelClosed)?
    }
}

fn extract_peer_id_from_multiaddr(ma: &Multiaddr) -> Option<libp2p::PeerId> {
    use libp2p::core::multiaddr::Protocol;
    ma.iter().find_map(|p| match p {
        Protocol::P2p(id) => Some(id),
        _ => None,
    })
}