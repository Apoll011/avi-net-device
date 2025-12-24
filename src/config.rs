#[derive(Clone, Debug)]
pub struct AviP2pConfig {
    /// Identity name for the node (used in Identify protocol)
    pub node_name: String,

    /// Port to listen on (0 for random)
    pub listen_port: u16,

    /// List of Multiaddr strings to bootstrap from
    pub bootstrap_peers: Vec<String>,

    /// Enable local network discovery via mDNS
    pub enable_mdns: bool,

    /// Enable Kademlia DHT for peer discovery
    pub enable_kad: bool,

    /// Maximum connected peers allowed (soft limit)
    pub max_peers: usize,

    /// Maximum concurrent streams
    pub max_streams: usize,
}

impl Default for AviP2pConfig {
    fn default() -> Self {
        Self {
            node_name: "avi-node".to_string(),
            listen_port: 0,
            bootstrap_peers: vec![],
            enable_mdns: true,
            enable_kad: true,
            max_peers: 50,
            max_streams: 10,
        }
    }
}