# AVI P2P

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../LICENSE)
**The central nervous system of the AVI Core ecosystem.**

`avi-p2p` is an opinionated, production-ready networking library designed for decentralized voice assistants and smart home devices. It provides a high-level abstraction over `libp2p` for Linux-based nodes and a lightweight UDP satellite library for bare-metal microcontrollers (ESP32, STM32).

---

## 🌟 Key Features

*   **Zero-Config Mesh:** Nodes automatically discover each other via mDNS and Kademlia DHT. No manual IP configuration required.
*   **Actor Model Architecture:** A single, isolated runtime manages the network state. Your application interacts via a thread-safe handle.
*   **📢 Pub/Sub Messaging:** Efficient, topic-based broadcasting using `GossipSub`.
*   **🧠 Distributed Context (CRDTs):** Built-in state synchronization using Vector Clocks. Perfect for syncing volume, lights, and device status across the home without a central server.
*   **🌉 Embedded Bridge:** A specialized UDP bridge that allows `no_std` microcontrollers to transparently participate in the P2P mesh.
*   **🎙️ Audio Streaming:** Logical stream management optimized for real-time voice data.

---

## 🏗️ Architecture & Workspace

This repository is organized as a Cargo Workspace containing three crates:

| Crate | Path | Description | Environment |
| :--- | :--- | :--- | :--- |
| **`avi-p2p`** | `./` | The main library. Runs the full Libp2p stack, DHT, and GossipSub. Acts as a Gateway. | **Linux / MacOS / Windows** |
| **`avi-p2p-embedded`** | `./embedded` | Lightweight client for microcontrollers. Uses raw UDP to talk to the main library. | **`no_std` / Bare Metal** |
| **`avi-p2p-protocol`** | `./protocol` | Shared binary protocol definitions (Messages, Enums, Serialization). | **Shared** |

---

## 🚀 Quick Start (Linux/PC)

### 1. Installation
Add this to your `Cargo.toml`:

```toml
[dependencies]
avi-p2p = { git = "https://github.com/apoll011/avi-p2p" }
tokio = { version = "1", features = ["full"] }
```

### 2. Basic Usage (Pub/Sub)
Spin up two nodes in two terminals to see them connect automatically.

**Terminal 1:**
```bash
cargo run --example simple_node
```

**Terminal 2:**
```bash
cargo run --example simple_node
```

They will discover each other, form a mesh, and you can exchange messages immediately.

---

## 🧠 Smart Home Context (CRDTs)

`avi-p2p` includes a synchronized state engine. You can update a "dictionary" of data on one device, and it propagates to all other devices, handling conflicts automatically.

### Usage
```rust
// 1. Update local context (e.g., changed volume)
let patch = serde_json::json!({
    "device": {
        "audio": { "volume": 75 }
    }
});
handle.update_context(patch).await?;

// 2. Listen for updates from others
while let Some(event) = event_rx.recv().await {
    if let AviEvent::ContextUpdated { peer_id, context } = event {
        println!("Device {} updated state: {:?}", peer_id, context);
    }
}
```

Run the example to see this in action:
```bash
cargo run --example context_sync
```

---

## 🌉 Embedded Device Integration

For microcontrollers (ESP32, STM32) that cannot run the full `libp2p` stack, `avi-p2p` provides a **UDP Bridge**.

### 1. The Gateway (Linux Side)
The Linux node acts as a bridge. It listens on a UDP port and forwards embedded messages into the P2P mesh.

```rust
// In your Linux Node
use avi_p2p::bridge::{EmbeddedBridge, BridgeConfig};

let (node, _) = AviP2p::start(config).await?;

// Start Bridge on Port 8888
EmbeddedBridge::start(
    node.handle(), 
    BridgeConfig { udp_port: 8888 }
).await.unwrap();
```

### 2. The Microcontroller (Embedded Side)
The embedded device uses the `avi-p2p-embedded` crate. It is `no_std` compatible and allocator-friendly.

```rust
use avi_p2p_embedded::{AviEmbedded, AviEmbeddedConfig};
use avi_p2p_protocol::{PressType, SensorValue};

// 1. Wrap your hardware UDP socket (e.g., embassy-net)
let mut avi = AviEmbedded::new(my_socket, config, &mut buffer);

// 2. Connect to Gateway
avi.connect().await.unwrap();

// 3. Send Events (Bridge publishes these to "avi/devices/<ID>/events")
avi.button_pressed(1, PressType::Double).await.unwrap();
avi.update_sensor("temp_kitchen", SensorValue::Temperature(24.5)).await.unwrap();
```

### Run the Simulation
You can simulate an MCU on your PC to test the bridge:

1.  **Terminal 1 (Gateway):** `cargo run --example gateway_node`
2.  **Terminal 2 (Fake ESP32):** `cargo run --example simulated_mcu`

---

## ⚙️ Configuration

You can customize the node behavior via `AviP2pConfig`.

```rust
let config = AviP2pConfig {
    // Identity
    node_name: "living-room-speaker".to_string(),
    
    // Network
    listen_port: 0, // 0 = Random available port
    
    // Discovery options
    enable_mdns: true, // Local network discovery
    enable_kad: true,  // DHT for routing
    
    // Bootstrap peers (if known)
    bootstrap_peers: vec!["/ip4/192.168.1.5/tcp/8000/p2p/..."].to_string()],
    
    ..Default::default()
};
```

---

## 📚 API Reference

### `AviP2pHandle`
The main interface for your application. It is cloneable and thread-safe.

*   `subscribe(topic)` / `unsubscribe(topic)`
*   `publish(topic, data)`
*   `request_audio_stream(peer_id)` -> `StreamId`
*   `update_context(json_patch)`
*   `get_context(peer_id)`

### `AviEvent`
Events emitted by the runtime.

*   `PeerConnected` / `PeerDisconnected`
*   `Message` (PubSub data)
*   `ContextUpdated` (CRDT state change)
*   `AudioData` (Incoming stream packets)

---

## 🛡️ Design Principles

1.  **Zero Leakage:** No `libp2p` types are exposed in the public API. You use `avi_p2p::PeerId`, never `libp2p::PeerId`.
2.  **Fail-Fast:** No silent failures. All commands return `Result`.
3.  **Automatic Healing:** The runtime includes a heartbeat that automatically attempts to reconnect to known peers if the connection drops.
4.  **Security:** All P2P traffic is encrypted via **Noise** and multiplexed via **Yamux**.

---

## 🤝 Contributing

1.  Fork the repository.
2.  Create a feature branch (`git checkout -b feature/amazing-feature`).
3.  Commit your changes.
4.  Push to the branch.
5.  Open a Pull Request.

## 📄 License

Distributed under the MIT License. See `LICENSE` for more information.
