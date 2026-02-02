use avi_p2p::{AviP2p, AviP2pConfig, AviEvent};
use avi_p2p::bridge::{EmbeddedBridge, BridgeConfig};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=================================================");
    println!("  📡 AVI P2P GATEWAY WITH PUB/SUB BRIDGE");
    println!("=================================================\n");

    // ==========================================
    // GATEWAY NODE
    // ==========================================
    let mut config = AviP2pConfig::default();
    config.node_name = "gateway-hub".to_string();
    config.listen_port = 0;

    let (gateway_node, mut gateway_events) = AviP2p::start(config).await?;
    let gateway_handle = gateway_node.handle();

    println!("✅ Gateway started");

    // Start the UDP Bridge on Port 8888
    println!("🌉 Starting UDP Bridge on port 8888...");
    EmbeddedBridge::start(
        gateway_handle.clone(),
        BridgeConfig { udp_port: 8888 }
    ).await?;

    println!("✅ Bridge listening on UDP 0.0.0.0:8888");
    println!("\n📝 Embedded devices can now connect to this gateway");
    println!("   UDP Address: 127.0.0.1:8888 (or your LAN IP)\n");

    // ==========================================
    // MONITOR NODE (Optional)
    // ==========================================
    println!("🚀 Starting Monitor Node...");

    let mut monitor_config = AviP2pConfig::default();
    monitor_config.node_name = "dashboard-monitor".to_string();

    let (monitor_node, mut monitor_events) = AviP2p::start(monitor_config).await?;
    let monitor_handle = monitor_node.handle();

    // Wait for mesh connection
    sleep(Duration::from_secs(2)).await;

    // Subscribe to topics we want to monitor
    println!("📥 Monitor subscribing to topics...");
    monitor_handle.subscribe("avi/home/+/button").await?;
    monitor_handle.subscribe("avi/home/+/sensor/#").await?;
    monitor_handle.subscribe("avi/home/+/status").await?;
    monitor_handle.subscribe("avi/home/broadcast").await?;
    println!("   ✓ Subscribed to device events\n");

    println!("=================================================");
    println!("  🎉 SYSTEM READY!");
    println!("=================================================");
    println!("\n📍 Next steps:");
    println!("   1. Run the simulated embedded device:");
    println!("      cargo run --example simulated_mcu_pubsub");
    println!("\n   2. Or flash to ESP32:");
    println!("      cd embedded-esp && cargo run --example esp32_device");
    println!("\n   3. Watch this terminal for mesh events!");
    println!("\n=================================================\n");

    // ==========================================
    // EVENT MONITORING LOOP
    // ==========================================
    
    // Spawn gateway event handler
    tokio::spawn(async move {
        while let Some(event) = gateway_events.recv().await {
            match event {
                AviEvent::Message { from, topic, data } => {
                    let preview = if data.len() <= 100 {
                        String::from_utf8_lossy(&data).to_string()
                    } else {
                        format!("{} bytes", data.len())
                    };
                    println!("[GATEWAY] 📨 Message from {}", from);
                    println!("          Topic: {}", topic);
                    println!("          Data: {}\n", preview);
                }
                AviEvent::PeerConnected { peer_id, address } => {
                    println!("[GATEWAY] 🔗 Peer connected: {}", peer_id);
                    println!("          Address: {}\n", address);
                }
                AviEvent::PeerDisconnected { peer_id } => {
                    println!("[GATEWAY] ❌ Peer disconnected: {}\n", peer_id);
                }
                _ => {}
            }
        }
    });

    // Monitor node event handler
    let mut message_count = 0;
    while let Some(event) = monitor_events.recv().await {
        match event {
            AviEvent::Message { from, topic, data } => {
                message_count += 1;
                
                println!("┌─────────────────────────────────────────────");
                println!("│ [MONITOR] Message #{}", message_count);
                println!("├─────────────────────────────────────────────");
                println!("│ 📡 From: {}", from);
                println!("│ 📋 Topic: {}", topic);
                
                // Try to parse as JSON for prettier output
                if let Ok(json_str) = String::from_utf8(data.clone()) {
                    if json_str.trim().starts_with('{') {
                        println!("│ 📦 Data (JSON):");
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                            let pretty = serde_json::to_string_pretty(&parsed).unwrap_or(json_str);
                            for line in pretty.lines() {
                                println!("│    {}", line);
                            }
                        } else {
                            println!("│    {}", json_str);
                        }
                    } else {
                        println!("│ 📦 Data: {}", json_str);
                    }
                } else {
                    println!("│ 📦 Data: {} bytes (binary)", data.len());
                }
                println!("└─────────────────────────────────────────────\n");
            }
            AviEvent::PeerConnected { peer_id, address } => {
                println!("🔗 [MONITOR] Connected to peer: {}", peer_id);
                println!("   Address: {}\n", address);
            }
            AviEvent::PeerDisconnected { peer_id } => {
                println!("❌ [MONITOR] Disconnected from: {}\n", peer_id);
            }
            AviEvent::ContextUpdated { peer_id, context } => {
                println!("🔄 [MONITOR] Context updated from: {}", peer_id);
                if let Ok(pretty) = serde_json::to_string_pretty(&context) {
                    println!("   {}\n", pretty);
                }
            }
            _ => {}
        }
    }

    Ok(())
}
