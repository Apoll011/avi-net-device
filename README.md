# AVI Device

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

**A high-level wrapper for `avi-p2p` to facilitate AVI device communications.**

`avi-device` simplifies the integration of the AVI P2P networking stack into your application. It provides a structured way to manage device capabilities, handle pub/sub messaging, synchronize distributed context, and manage real-time data streams.

---

## 🌟 Key Features

*   **Simplified Device Management:** Easy initialization of AVI nodes with configurable types (CORE or NODE) and capabilities.
*   **📢 Pub/Sub Messaging:** High-level abstractions for topic-based communication.
*   **🧠 Distributed Context:** Simple API to get and update shared state across the mesh with nested path support.
*   **🎙️ Stream Dispatcher:** Managed lifecycle for data streams (Audio, Logs, etc.) with trait-based handlers and factories.
*   **🔍 Device Discovery & Querying:** Find peers and query their capabilities using a flexible query system.

---

## 🚀 Quick Start

### 1. Installation

Add `avi-device` and `tokio` to your `Cargo.toml`:

```toml
[dependencies]
avi-device = { git = "https://github.com/apoll011/avi-p2p" }
tokio = { version = "1", features = ["full"] }
```

### 2. Basic Usage

```rust
use avi_device::device::{AviDevice, AviDeviceConfig, AviDeviceType};
use avi_device::DeviceCapabilities;

#[tokio::main]
async fn main() -> Result<(), String> {
    // 1. Configure the device
    let config = AviDeviceConfig {
        node_name: "living-room-speaker".to_string(),
        device_type: AviDeviceType::NODE,
        capabilities: DeviceCapabilities::default(), // Define your caps here
    };

    // 2. Initialize the device
    let mut device = AviDevice::new(config).await?;

    // 3. Start the event loop in a background task
    let device_clone = device.clone();
    tokio::spawn(async move {
        device_clone.start_event_loop().await;
    });

    // 4. Use the device
    let peers = device.get_peers().await.map_err(|e| e.to_string())?;
    println!("Connected peers: {:?}", peers);

    Ok(())
}
```

---

## 📖 Examples

Detailed examples for each major functionality can be found in the `examples/` directory:

*   **`device_pubsub.rs`**: Demonstrates topic-based Publish/Subscribe.
*   **`device_context.rs`**: Shows how to use the Distributed Context (CRDTs) with nested paths.
*   **`device_query.rs`**: How to define device capabilities and query them across the network.
*   **`device_stream.rs`**: Setting up custom stream handlers for real-time data transfer.
*   **`device_chat.rs`**: A comprehensive CLI-based chat application that uses all features (Pub/Sub, Context, Query, and Streams).

To run an example:
```bash
cargo run --example device_pubsub
```

---

## 📚 API Overview

### `AviDevice`

The main interface for interacting with the AVI network.

*   **`new(config)`**: Starts the underlying P2P node and initializes the dispatcher.
*   **`start_event_loop()`**: Processes incoming P2P events (connections, messages, streams). **Must be spawned in a task.**
*   **`publish(topic, data)`**: Broadcast data to the mesh.
*   **`subscribe(topic, handler)`**: Listen for messages on a specific topic.
*   **`update_ctx(path, value)`**: Update a piece of the distributed context (e.g., `"avi.device.status.volume"`).
*   **`get_ctx(path)`**: Retrieve data from the context.
*   **`request_stream(peer_id, reason)`**: Initiate a data stream to another peer.
*   **`register_stream_handler(reason, factory)`**: Register how to handle incoming or outgoing streams for a specific purpose.

### `DeviceCapabilities` & `DeviceQuery`

Used to define what a device can do and to find other devices that meet specific criteria.

---

## 🏗️ Architecture

`avi-device` acts as a coordination layer:

1.  **`AviP2p`**: The raw networking engine (Libp2p/GossipSub/Kademlia).
2.  **`StreamDispatcher`**: Manages multiple concurrent data streams and routes them to the correct `StreamHandler`.
3.  **`DeviceContext`**: Manages the local view of the global CRDT state.

---

## 📄 License

Distributed under the MIT License. See `LICENSE` for more information.
