use avi_p2p::{AviP2p, AviP2pConfig, AviEvent};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AviP2pConfig {
        node_name: "monitor-node".to_string(),
        enable_mdns: true,
        enable_kad: true,
        ..Default::default()
    };

    let (node, mut event_rx) = AviP2p::start(config).await?;
    let handle = node.handle();

    println!("🔎 Network Monitor Started.");

    // Background task to trigger discovery periodically
    let discovery_handle = handle.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(10)).await;
            println!("🔄 Triggering manual Kademlia bootstrap...");
            if let Err(e) = discovery_handle.discover_peers().await {
                eprintln!("Discovery failed: {}", e);
            }

            // Query connected peers
            match discovery_handle.connected_peers().await {
                Ok(peers) => println!("📊 Currently Connected: {} peers", peers.len()),
                Err(e) => eprintln!("Failed to fetch peers: {}", e),
            }
        }
    });

    // Main event loop
    while let Some(event) = event_rx.recv().await {
        match event {
            AviEvent::Started { local_peer_id, listen_addresses } => {
                println!("🟢 Node Started: {}", local_peer_id);
                println!("   Listening on: {:?}", listen_addresses);
            }
            AviEvent::PeerDiscovered { peer_id } => {
                // This fires when mDNS or DHT finds an address, but before connection
                println!("👀 Discovered Peer: {}", peer_id);
            }
            AviEvent::PeerConnected { peer_id, address } => {
                // This fires when a TCP connection is established
                println!("🔗 Connected to {} via {}", peer_id, address);
            }
            AviEvent::PeerDisconnected { peer_id } => {
                println!("🔌 Disconnected from {}", peer_id);
            }
            _ => {} // Ignore audio/pubsub events
        }
    }

    Ok(())
}