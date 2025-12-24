use avi_p2p::{AviP2p, AviP2pConfig, AviEvent};
use tokio::io::{self, AsyncBufReadExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let mut config = AviP2pConfig::default();

    // We enable mDNS, but we also support explicit arguments for robustness
    config.enable_mdns = true;

    if args.len() > 1 {
        let peer_addr = args[1].clone();
        config.bootstrap_peers.push(peer_addr.clone());
        println!("🚀 Bootstrapping to peer: {}", peer_addr);
    }

    let (node, mut event_rx) = AviP2p::start(config).await?;
    let handle = node.handle();

    // 3. Event Loop
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AviEvent::Started { local_peer_id, listen_addresses } => {
                    println!("\n✅ NODE STARTED");
                    println!("   ID: {}", local_peer_id);

                    let mut found_robust_addr = false;

                    println!("   Listening on:");
                    for addr in &listen_addresses {
                        println!("    - {}", addr);
                        // Filter specifically for 127.0.0.1 for the easy copy-paste command.
                        // This bypasses issues where 0.0.0.0 resolves to a LAN IP blocked by firewall.
                        if addr.contains("127.0.0.1") {
                            println!("\n📋 TO CONNECT TERMINAL 2, RUN:");
                            println!("   cargo run --example simple_node -- {}/p2p/{}", addr, local_peer_id);
                            found_robust_addr = true;
                        }
                    }

                    if !found_robust_addr && !listen_addresses.is_empty() {
                        // Fallback if 127.0.0.1 isn't strictly listed (rare, but happens)
                        println!("\n📋 TO CONNECT TERMINAL 2, RUN:");
                        println!("   cargo run --example simple_node -- {}/p2p/{}", listen_addresses[0], local_peer_id);
                    }
                },
                AviEvent::PeerDiscovered { peer_id } => {
                    println!("👀 Discovered Peer (mDNS/DHT): {}", peer_id);
                },
                AviEvent::PeerConnected { peer_id, address } => {
                    println!("🔗 PEER CONNECTED: {} ({})", peer_id, address);
                },
                AviEvent::Message { from, topic, data } => {
                    let msg = String::from_utf8_lossy(&data);
                    println!("📩 [{}] {}: {}", topic, from, msg);
                },
                _ => {}
            }
        }
    });

    println!("commands: 'sub <topic>', 'pub <topic> <msg>'");

    let mut stdin = io::BufReader::new(io::stdin()).lines();
    while let Ok(Some(line)) = stdin.next_line().await {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "sub" if parts.len() > 1 => {
                handle.subscribe(parts[1]).await?;
                println!("Subscribed to '{}'", parts[1]);
            },
            "pub" if parts.len() > 2 => {
                let topic = parts[1];
                let content = parts[2..].join(" ");
                handle.publish(topic, content.into_bytes()).await?;
            },
            "peers" => {
                let peers = handle.connected_peers().await?;
                println!("Connected peers: {:?}", peers);
            },
            _ => println!("Unknown command"),
        }
    }

    node.shutdown().await?;
    Ok(())
}