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