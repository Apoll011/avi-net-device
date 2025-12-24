use avi_p2p::{AviP2p, AviP2pConfig, AviEvent};
use tokio::io::{self, AsyncBufReadExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start node
    let config = AviP2pConfig::default();
    let (node, mut event_rx) = AviP2p::start(config).await?;
    let handle = node.handle();

    println!("Node started. Type 'sub <topic>' to subscribe, 'pub <topic> <msg>' to publish.");

    // Handle events
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AviEvent::Started { local_peer_id, .. } => {
                    println!("Network started. Local ID: {}", local_peer_id);
                },
                AviEvent::Message { from, topic, data } => {
                    let msg = String::from_utf8_lossy(&data);
                    println!("[{}] {}: {}", topic, from, msg);
                },
                AviEvent::PeerConnected { peer_id, .. } => {
                    println!("Peer connected: {}", peer_id);
                },
                AviEvent::AudioStreamRequested { from, stream_id } => {
                    println!("Incoming audio stream {} from {}", stream_id, from);
                    // Automatically accept for demo
                    // In real app, you would use handle.accept_audio_stream(stream_id)
                },
                _ => {}
            }
        }
    });

    // Handle stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    while let Ok(Some(line)) = stdin.next_line().await {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "sub" if parts.len() > 1 => {
                handle.subscribe(parts[1]).await?;
                println!("Subscribed to {}", parts[1]);
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
            "quit" => break,
            _ => println!("Unknown command"),
        }
    }

    node.shutdown().await?;
    Ok(())
}