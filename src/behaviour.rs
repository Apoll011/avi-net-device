use libp2p::{
    gossipsub,
    kad,
    mdns,
    identify,
    request_response,
    swarm::NetworkBehaviour,
    PeerId as LibPeerId,
};
use crate::protocols::audio::{AviAudioCodec};

#[derive(NetworkBehaviour)]
pub struct AviBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub audio: request_response::Behaviour<AviAudioCodec>,
}

impl AviBehaviour {
    pub fn new(
        local_peer_id: LibPeerId,
        pubsub_config: gossipsub::Config,
        node_name: String,
    ) -> Self {
        // Kademlia
        let store = kad::store::MemoryStore::new(local_peer_id);
        let kad_config = kad::Config::default();
        let kad = kad::Behaviour::with_config(local_peer_id, store, kad_config);

        // GossipSub
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_peer_id),
            pubsub_config,
        ).expect("Valid gossipsub config");

        // mDNS
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)
            .expect("Failed to create mDNS behaviour");

        // Identify
        let identify = identify::Behaviour::new(identify::Config::new(
            "/avi/1.0.0".into(),
            libp2p::identity::Keypair::generate_ed25519().public(), // Placeholder, real one injected in Swarm
        ).with_agent_version(node_name));

        // Audio Protocol (Request-Response)
        let audio = request_response::Behaviour::new(
            AviAudioCodec,
            vec![(crate::protocols::audio::AviAudioProtocol, request_response::ProtocolSupport::Full)],
            request_response::Config::default(),
        );

        Self {
            gossipsub,
            kad,
            mdns,
            identify,
            audio,
        }
    }
}