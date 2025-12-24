use avi_p2p::{AviP2p, AviP2pConfig, AviEvent};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AviP2pConfig::default();
    let (node, mut event_rx) = AviP2p::start(config).await?;
    let handle = node.handle();

    let topic = "system-status";

    // 1. Subscribe
    println!("Subscribing to '{}'...", topic);
    handle.subscribe(topic).await?;

    // 2. Spawn a publisher task
    let pub_handle = handle.clone();
    tokio::spawn(async move {
        let mut count = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            count += 1;

            let msg = format!("Heartbeat #{}", count);

            // Fail-fast handling: If the network layer dies, this returns an error
            match pub_handle.publish("system-status", msg.as_bytes().to_vec()).await {
                Ok(_) => println!("Sent: {}", msg),
                Err(e) => eprintln!("Failed to publish: {}", e),
            }
        }
    });

    // 3. Process incoming messages with graceful shutdown support
    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(AviEvent::Message { from, topic, data }) => {
                        let text = String::from_utf8_lossy(&data);
                        println!("📩 Received on [{}]: '{}' from {}", topic, text, from);
                    }
                    None => break, // Channel closed
                    _ => {}
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("Initiating graceful shutdown...");
                break;
            }
        }
    }

    // 4. Cleanup
    handle.unsubscribe(topic).await?;
    node.shutdown().await?;
    println!("Shutdown complete.");

    Ok(())
}