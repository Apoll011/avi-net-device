use libp2p::{
    gossipsub,
    kad,
    mdns,
    identify,
    request_response,
    swarm::NetworkBehaviour,
    PeerId as LibPeerId,
    identity::Keypair,
};
use crate::protocols::stream::AviStreamCodec;

#[derive(NetworkBehaviour)]
pub struct AviBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    #[cfg(not(target_arch = "wasm32"))]
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub stream: request_response::Behaviour<AviStreamCodec>,
}

impl AviBehaviour {
    pub fn new(
        local_key: Keypair, // Now accepts Keypair
        pubsub_config: gossipsub::Config,
        node_name: String,
    ) -> Self {
        let local_peer_id = LibPeerId::from(local_key.public());

        // Kademlia
        let store = kad::store::MemoryStore::new(local_peer_id);
        let mut kad_config = kad::Config::default();
        kad_config.set_query_timeout(std::time::Duration::from_secs(60)); // Good practice for newer libp2p
        let kad = kad::Behaviour::with_config(local_peer_id, store, kad_config);

        // GossipSub
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()), // Use cloned Keypair
            pubsub_config,
        ).expect("Valid gossipsub config");

        // mDNS (Conditional compilation)
        #[cfg(not(target_arch = "wasm32"))]
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)
            .expect("Failed to create mDNS behaviour");

        // Identify
        let identify = identify::Behaviour::new(identify::Config::new(
            "/avi/1.0.0".into(),
            local_key.public(), // Use the public key from the Keypair
        ).with_agent_version(node_name));

        let stream = request_response::Behaviour::new(
            // `new` expects an IntoIterator of (Protocol, ProtocolSupport) tuples
            std::iter::once((
                crate::protocols::stream::AviStreamProtocol,
                request_response::ProtocolSupport::Full
            )),
            request_response::Config::default(),
        );

        Self {
            gossipsub,
            kad,
            #[cfg(not(target_arch = "wasm32"))] // Conditional compilation for mdns field
            mdns,
            identify,
            stream,
        }
    }
}