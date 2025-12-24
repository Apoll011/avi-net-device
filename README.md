# AVI P2P

[![Crates.io](https://img.shields.io/crates/v/avi-p2p.svg)](https://crates.io/crates/avi-p2p)
[![Documentation](https://docs.rs/avi-p2p/badge.svg)](https://docs.rs/avi-p2p)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Production-grade, opinionated networking layer for the AVI Core platform.**

`avi-p2p` abstracts away the complexity of `libp2p`, providing a strict, type-safe, and asynchronous interface for mesh networking, pub/sub messaging, and audio streaming.

## Design Philosophy

- **Zero Leakage**: No `libp2p` types are exposed. You interact with `AviP2p` types only.
- **Fail-Fast**: No silent failures. All operations return clear `Result` types.
- **Actor Model**: A single event loop manages the network; you interact via a thread-safe handle.
- **Backpressure**: Internal channels are bounded to prevent memory exhaustion under load.

## Features

- 📢 **Pub/Sub**: Efficient topic-based broadcasting using GossipSub.
- 🎙️ **Audio Streaming**: Logical streams over custom request-response protocols with structured framing.
- 🕸️ **Mesh Discovery**: Automatic peer discovery via Kademlia DHT and mDNS.
- 🔒 **Security**: Noise encryption and Yamux multiplexing enabled by default.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
avi-p2p = "0.1.0"
tokio = { version = "1", features = ["full"] }

## Quick Start

```rust
use avi_p2p::{AviP2p, AviP2pConfig, AviEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure
    let config = AviP2pConfig::default();
    
    // 2. Start
    let (node, mut event_rx) = AviP2p::start(config).await?;
    let handle = node.handle();

    // 3. Subscribe
    handle.subscribe("lobby").await?;

    // 4. Event Loop
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AviEvent::Message { from, topic, data } => {
                    println!("Msg from {}: {:?}", from, data);
                }
                _ => {}
            }
        }
    });

    // 5. Publish
    handle.publish("lobby", b"Hello AVI!".to_vec()).await?;
    
    // Keep alive...
    tokio::signal::ctrl_c().await?;
    node.shutdown().await?;
    Ok(())
}
```

## Configuration

The default configuration is suitable for local development. For production, tune the limits:

```rust
let config = AviP2pConfig {
    node_name: "production-node-01".to_string(),
    listen_port: 9000,
    bootstrap_peers: vec!["/ip4/198.51.100.1/tcp/9000".to_string()],
    max_peers: 200,
    max_audio_streams: 50,
    ..Default::default()
};
```

## Architecture

`avi-p2p` spawns a background runtime (the "Swarm") that processes network I/O. Communication happens via:
1.  **Commands**: Sent from your application to the Swarm (e.g., `publish`, `request_stream`).
2.  **Events**: Sent from the Swarm to your application (e.g., `PeerDiscovered`, `AudioData`).

This separation ensures your application logic never blocks the network loop.

## License

MIT
```

---

### 2. Example: Audio Streaming (`examples/audio_chat.rs`)

This example demonstrates the complete lifecycle of an audio stream: requesting, accepting, sending data, and closing.

```rust
use avi_p2p::{AviP2p, AviP2pConfig, AviEvent, AviP2pHandle};
use std::time::Duration;
use tokio::time::sleep;

// Simulates an audio source (e.g., Microphone)
async fn run_sender(handle: AviP2pHandle, target_peer: String) {
    let peer_id = avi_p2p::PeerId::new(&target_peer);
    
    println!("Requesting audio stream to {}...", peer_id);
    match handle.request_audio_stream(peer_id).await {
        Ok(stream_id) => {
            println!("Stream request sent. ID: {}", stream_id);
            // Note: In a real app, we wait for AviEvent::AudioStreamAccepted 
            // before sending heavy data, but for this demo we start sending chunks.
            
            for i in 0..5 {
                let dummy_audio = vec![0u8; 1024]; // 1KB of silence
                if let Err(e) = handle.send_audio(stream_id, dummy_audio).await {
                    eprintln!("Failed to send audio: {}", e);
                    break;
                }
                println!("Sent chunk #{}", i);
                sleep(Duration::from_millis(200)).await;
            }
            
            let _ = handle.close_audio_stream(stream_id).await;
            println!("Stream closed locally.");
        }
        Err(e) => eprintln!("Failed to request stream: {}", e),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args();
    let _ = args.next();
    let mode = args.next().expect("Usage: cargo run --example audio_chat <listen|dial> [peer_addr]");

    let config = AviP2pConfig::default(); // Uses mDNS for local discovery
    let (node, mut event_rx) = AviP2p::start(config).await?;
    let handle = node.handle();

    // Event Loop
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AviEvent::AudioStreamRequested { from, stream_id } => {
                    println!("📞 Call from {} (Stream {})", from, stream_id);
                    // Auto-accept incoming calls
                    if let Err(e) = handle_clone.accept_audio_stream(stream_id).await {
                        eprintln!("Failed to accept: {}", e);
                    } else {
                        println!("✅ Accepted stream {}", stream_id);
                    }
                },
                AviEvent::AudioStreamAccepted { peer_id, stream_id } => {
                    println!("✅ Peer {} accepted stream {}", peer_id, stream_id);
                },
                AviEvent::AudioData { from, stream_id, data } => {
                    println!("🔊 Audio data from {} (Stream {}): {} bytes", from, stream_id, data.len());
                },
                AviEvent::AudioStreamClosed { peer_id, stream_id, reason } => {
                    println!("❌ Stream {} with {} ended: {:?}", stream_id, peer_id, reason);
                },
                _ => {}
            }
        }
    });

    if mode == "dial" {
        let target = args.next().expect("Provide peer ID to dial");
        // Give mDNS a second to find the peer before we try to stream
        println!("Waiting for discovery...");
        sleep(Duration::from_secs(2)).await;
        run_sender(handle, target).await;
    } else {
        println!("Listening for incoming streams... (Ctrl+C to quit)");
        println!("My Peer ID can be seen in the 'Started' event logs of other nodes.");
        tokio::signal::ctrl_c().await?;
    }

    node.shutdown().await?;
    Ok(())
}
```

---

### 3. Example: Mesh Discovery Monitor (`examples/discovery_monitor.rs`)

This example focuses on `AviEvent::PeerDiscovered`, `PeerConnected`, and manual Kademlia bootstrapping. It's useful for debugging network topology.

```rust
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
```

---

### 4. Example: Resilient Pub/Sub (`examples/resilient_pubsub.rs`)

Shows how to handle errors (like publishing before connected) and subscription management.

```rust
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
```