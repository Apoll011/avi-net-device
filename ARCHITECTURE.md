# AVI P2P Architecture: Embedded Integration

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        AVI P2P MESH NETWORK                     │
│                                                                 │
│  ┌──────────────┐         ┌──────────────┐   ┌──────────────┐│
│  │   Desktop    │◄───────►│   Gateway    │◄─►│   Mobile     ││
│  │  Dashboard   │         │     Node     │   │     App      ││
│  └──────────────┘         └──────┬───────┘   └──────────────┘│
│         ▲                         │                             │
│         │                         │                             │
│         │        P2P GossipSub    │                             │
│         └─────────────────────────┘                             │
│                                   │                             │
└───────────────────────────────────┼─────────────────────────────┘
                                    │
                        ┌───────────▼───────────┐
                        │   UDP Bridge (8888)   │
                        │  - Protocol Gateway   │
                        │  - Pub/Sub Router     │
                        └───────────┬───────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
                    ▼               ▼               ▼
            ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
            │   ESP32 #1   │ │   ESP32 #2   │ │   ESP32 #3   │
            │  (Sensors)   │ │  (Controls)  │ │   (Audio)    │
            └──────────────┘ └──────────────┘ └──────────────┘
```

## Component Architecture

### 1. Protocol Layer (`avi-p2p-protocol`)

**Purpose:** Shared serialization protocol between embedded and gateway

**Key Types:**
```rust
// Messages from embedded → gateway
pub enum UplinkMessage<'a> {
    Hello { device_id: u64 },
    Subscribe { topic: &'a str },
    Unsubscribe { topic: &'a str },
    Publish { topic: &'a str, data: &'a [u8] },
    ButtonPress { button_id: u8, press_type: PressType },
    SensorUpdate { sensor_name: &'a str, data: SensorValue },
    StreamStart/Data/Close { ... },
}

// Messages from gateway → embedded
pub enum DownlinkMessage<'a> {
    Welcome,
    Message { topic: &'a str, data: &'a [u8] },
    SubscribeAck { topic: &'a str },
    UnsubscribeAck { topic: &'a str },
    Error { reason: u8 },
}
```

**Serialization:** Postcard (binary, efficient, no_std compatible)

### 2. Embedded Library (`avi-p2p-embedded`)

**Purpose:** Core no_std library for embedded devices

**Architecture:**
```rust
pub trait UdpClient {
    type Error;
    async fn send(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
    async fn receive(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}

pub trait MessageHandler {
    fn on_message(&mut self, topic: &str, data: &[u8]);
}

pub struct AviEmbedded<'a, S: UdpClient, H: MessageHandler> {
    socket: S,
    config: AviEmbeddedConfig,
    scratch_buf: &'a mut [u8],
    is_connected: bool,
    handler: H,
}
```

**Key Design Decisions:**
- Zero allocation (uses provided buffer)
- Async/await with generic socket trait
- Callback-based message handling
- Single-threaded (suitable for embedded)

### 3. ESP-IDF Implementation (`avi-p2p-embedded-esp`)

**Purpose:** ESP32-specific UDP and WiFi implementation

**Components:**
```rust
pub struct EspUdpSocket {
    sock_fd: i32,           // lwIP socket
    target_addr: sockaddr_in,
}

impl UdpClient for EspUdpSocket { ... }

// WiFi helpers
pub fn init_wifi(config: WifiConfig) -> Result<()> {
    // esp-idf-svc WiFi setup
}
```

**lwIP Integration:**
- Direct lwIP calls for minimal overhead
- Non-blocking socket operations
- Integration with FreeRTOS task system

### 4. Gateway Bridge (`p2p/src/bridge.rs`)

**Purpose:** Bridge UDP embedded devices to P2P mesh

**Architecture:**
```rust
pub struct EmbeddedBridge {
    socket: Arc<UdpSocket>,
    handle: AviP2pHandle,
    sessions: Arc<Mutex<HashMap<SocketAddr, DeviceSession>>>,
}

struct DeviceSession {
    device_id: u64,
    active_streams: HashMap<u8, StreamId>,
    subscriptions: HashSet<String>,
}
```

**Flow Handling:**

**Uplink (Embedded → Gateway):**
1. UDP packet received
2. Deserialize to `UplinkMessage`
3. Route based on message type:
   - `Subscribe` → Subscribe on mesh + track in session
   - `Publish` → Publish to mesh
   - `SensorUpdate` → Publish to mesh + update context
   - `ButtonPress` → Publish to mesh

**Downlink (Gateway → Embedded):**
1. Listen to P2P mesh events
2. Filter by device subscriptions
3. Serialize to `DownlinkMessage`
4. Send UDP packet to device

### 5. P2P Mesh (`avi-p2p`)

**Protocols Used:**
- **GossipSub:** Pub/sub messaging
- **Request-Response:** Bidirectional streams
- **Identify:** Peer discovery
- **Kademlia:** DHT routing
- **mDNS:** Local network discovery

## Message Flow Examples

### Example 1: Sensor Reading

```
[ESP32]                    [Gateway]                    [Mesh]
   |                          |                           |
   |--SensorUpdate(temp)----->|                           |
   |                          |--Publish("avi/.../temp")-->|
   |                          |                           |--[All Subscribers]
   |                          |                           |
   |                          |<--ContextUpdate-----------|
   |                          |  (temp stored in shared   |
   |                          |   context)                |
```

### Example 2: Command to Device

```
[Dashboard]                [Gateway]                  [ESP32]
   |                          |                           |
   |--Publish("avi/.../cmd")-->|                           |
   |                          |                           |
   |                          | (finds subscribed device) |
   |                          |--Message("avi/.../cmd")--->|
   |                          |                           |
   |                          |                         [Execute]
   |                          |<--SensorUpdate(ack)-------|
```

### Example 3: Device Subscribe Flow

```
[ESP32]                    [Gateway]                    [Mesh]
   |                          |                           |
   |--Subscribe("topic")----->|                           |
   |                          |--Subscribe("topic")------->|
   |                          |<--SubscribeAck------------|
   |<--SubscribeAck-----------|                           |
   |                          |                           |
   | (now receiving messages on "topic")                  |
   |                          |<--Message("topic", data)--|
   |<--Message("topic", data)-|                           |
   |                          |                           |
```

## Performance Characteristics

### Latency
- **Local mesh:** ~5-20ms
- **ESP32 → Gateway:** ~10-30ms
- **End-to-end (ESP32 → Mesh → ESP32):** ~50-100ms

### Throughput
- **Sensor updates:** 10-100 Hz per device
- **Pub/sub messages:** 1000+ msg/sec (mesh)
- **UDP packets:** Limited by WiFi (~1-10 Mbps)

### Memory (ESP32)
- **Stack:** 8KB minimum (configurable)
- **Heap:** ~2KB for buffers
- **Code:** ~100KB flash

## Security Considerations

### Current Implementation
- **No encryption** on UDP bridge
- **No authentication** of devices
- **Trust-based** mesh network

### Production Recommendations

1. **Mutual TLS for UDP:**
```rust
// Encrypt UDP packets before send
let encrypted = encrypt_aes_gcm(&packet, &device_key);
socket.send(&encrypted).await?;
```

2. **Device Authentication:**
```rust
UplinkMessage::Hello {
    device_id: 1234,
    auth_token: hash_hmac(&device_secret, &nonce),
}
```

3. **Message Signing:**
```rust
DownlinkMessage::Message {
    topic: "...",
    data: &payload,
    signature: sign_ed25519(&payload, &gateway_key),
}
```

## Scaling Considerations

### Devices per Gateway
- **Practical limit:** 100-500 devices
- **Bottleneck:** UDP packet processing
- **Solution:** Multiple gateway bridges

### Multiple Gateways
```
[ESP32 Group 1] → [Gateway 1] ─┐
[ESP32 Group 2] → [Gateway 2] ─┼→ [P2P Mesh]
[ESP32 Group 3] → [Gateway 3] ─┘
```

Each gateway handles subset of devices, all connected to same mesh.

### Geographic Distribution
```
[Building A] → [Gateway A] ──┐
                             [Internet VPN]
[Building B] → [Gateway B] ──┘
```

Use libp2p relay nodes for WAN connectivity.

## Error Handling

### Connection Failures
```rust
// Automatic reconnection
loop {
    if !device.is_connected() {
        while device.connect().await.is_err() {
            delay(5000);
        }
    }
    
    match device.poll().await {
        Ok(_) => {},
        Err(_) => { /* handle error */ }
    }
}
```

### Message Loss
- UDP is unreliable
- Important messages should be acknowledged
- Implement retry logic for critical operations

### Memory Exhaustion
- Fixed buffer sizes prevent allocation
- Gateway has session limits
- ESP32 monitors heap usage

## Testing Strategy

### Unit Tests
- Protocol serialization/deserialization
- Message handler callbacks
- Context updates

### Integration Tests
```bash
# Terminal 1: Gateway
cargo run --example gateway_with_pubsub

# Terminal 2: Simulated device
cargo run --example simulated_mcu_pubsub

# Terminal 3: Monitor
cargo run --example monitor_node
```

### Hardware Tests
- Flash multiple ESP32s
- Test concurrent connections
- Measure latency/throughput
- Stress test with packet loss

## Future Enhancements

### 1. Stream Compression
```rust
// Compress sensor data before transmission
let compressed = lz4::compress(&sensor_data);
device.publish("topic", &compressed).await?;
```

### 2. QoS Levels
```rust
device.publish_qos("topic", data, QoS::AtLeastOnce).await?;
```

### 3. Offline Buffering
```rust
// Store messages when disconnected
if !device.is_connected() {
    offline_buffer.push(message);
} else {
    for msg in offline_buffer.drain(..) {
        device.publish(&msg.topic, &msg.data).await?;
    }
}
```

### 4. CoAP Integration
Replace UDP with CoAP for better standardization.

### 5. BLE Bridge
```
[BLE Device] ←→ [ESP32 Gateway] ←→ [UDP] ←→ [P2P Gateway]
```

## Debugging Tips

### Enable Logging (ESP32)
```rust
esp_idf_svc::log::EspLogger::initialize_default();
log::set_max_level(log::LevelFilter::Debug);
```

### Network Capture
```bash
# Capture UDP traffic
sudo tcpdump -i any -n udp port 8888 -X

# Capture P2P mesh
sudo tcpdump -i any -n tcp portrange 0-65535
```

### Memory Profiling
```rust
println!("Free heap: {}", esp_idf_sys::esp_get_free_heap_size());
println!("Min free heap: {}", esp_idf_sys::esp_get_minimum_free_heap_size());
```

## Conclusion

This architecture provides:
- ✅ Seamless embedded device integration
- ✅ Scalable pub/sub messaging
- ✅ Real-time bidirectional communication
- ✅ Production-ready foundations

The system is designed to be extended with additional features while maintaining the core simplicity and reliability needed for embedded deployments.
