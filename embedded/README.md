# AVI P2P Embedded Library

A `no_std` Rust library for connecting embedded devices (ESP32, STM32, etc.) to the AVI P2P mesh network via a UDP gateway bridge.

## 🎯 Features

- **Zero-allocation** `no_std` compatible core
- **Pub/Sub messaging** - Subscribe to topics and receive messages
- **Sensor reporting** - Temperature, humidity, battery, status
- **Button events** - Single, double, long press detection
- **Audio streaming** - Bidirectional audio streams over the mesh
- **ESP-IDF support** - Full ESP32 implementation included

## 📦 Components

### 1. `avi-p2p-protocol`
Shared protocol definitions between embedded devices and gateway.

```rust
pub enum UplinkMessage {
    Hello { device_id: u64 },
    Subscribe { topic: &str },
    Unsubscribe { topic: &str },
    Publish { topic: &str, data: &[u8] },
    ButtonPress { button_id: u8, press_type: PressType },
    SensorUpdate { sensor_name: &str, data: SensorValue },
    // ... streaming methods
}

pub enum DownlinkMessage {
    Welcome,
    Message { topic: &str, data: &[u8] },
    SubscribeAck { topic: &str },
    // ...
}
```

### 2. `avi-p2p-embedded`
Core `no_std` library - works on any platform that implements `UdpClient` trait.

```rust
pub trait UdpClient {
    type Error;
    fn send(&mut self, buf: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
    fn receive(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}

pub trait MessageHandler {
    fn on_message(&mut self, topic: &str, data: &[u8]);
}
```

### 3. `avi-p2p-embedded-esp`
ESP-IDF specific implementation with WiFi and UDP stack.

## 🚀 Quick Start (PC Simulation)

```rust
use avi_p2p_embedded::{AviEmbedded, AviEmbeddedConfig, MessageHandler};

struct MyHandler;

impl MessageHandler for MyHandler {
    fn on_message(&mut self, topic: &str, data: &[u8]) {
        println!("Received on {}: {:?}", topic, data);
    }
}

#[tokio::main]
async fn main() {
    let socket = /* your UDP socket */;
    let mut buffer = [0u8; 1024];
    let config = AviEmbeddedConfig { device_id: 1234 };
    
    let mut device = AviEmbedded::new(socket, config, &mut buffer, MyHandler);
    
    // Connect
    device.connect().await.unwrap();
    
    // Subscribe to topics
    device.subscribe("avi/home/commands").await.unwrap();
    
    // Main loop
    loop {
        // Poll for messages
        let _ = device.poll().await;
        
        // Send sensor data
        device.update_sensor("temp", SensorValue::Temperature(22.5)).await.unwrap();
        
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```

## 🔧 ESP32 Usage

### Cargo.toml
```toml
[dependencies]
avi-p2p-embedded-esp = { path = "../embedded-esp" }
esp-idf-svc = "0.48"
```

### main.rs
```rust
#![no_std]
#![no_main]

use avi_p2p_embedded_esp::{init_wifi, EspUdpSocket, WifiConfig, helpers};
use avi_p2p_embedded::{AviEmbedded, AviEmbeddedConfig, MessageHandler};

struct MyHandler;
impl MessageHandler for MyHandler {
    fn on_message(&mut self, topic: &str, data: &[u8]) {
        esp_println::println!("📨 {}: {} bytes", topic, data.len());
    }
}

fn main() -> ! {
    // 1. Connect to WiFi
    let wifi = WifiConfig {
        ssid: "MyNetwork",
        password: "password123",
    };
    init_wifi(wifi).unwrap();
    
    // 2. Create socket to gateway
    let socket = EspUdpSocket::new([192, 168, 1, 100], 8888).unwrap();
    
    // 3. Setup device
    let mut buffer = [0u8; 1024];
    let config = AviEmbeddedConfig { device_id: 5555 };
    let mut device = AviEmbedded::new(socket, config, &mut buffer, MyHandler);
    
    // 4. Use executor for async
    let executor = esp_idf_svc::hal::task::executor::EspExecutor::new();
    executor.spawn_detached(async move {
        device.connect().await.unwrap();
        device.subscribe("avi/home/device_5555/command").await.unwrap();
        
        loop {
            let _ = device.poll().await;
            helpers::report_temperature(&mut device, "room", 23.0).await.unwrap();
            esp_idf_svc::sys::vTaskDelay(5000);
        }
    }).unwrap();
    
    loop { esp_idf_svc::sys::vTaskDelay(1000); }
}
```

## 🌉 Gateway Bridge

The gateway runs on a standard PC/server and bridges UDP packets to the P2P mesh.

### Starting the Gateway
```rust
use avi_p2p::{AviP2p, AviP2pConfig};
use avi_p2p::bridge::{EmbeddedBridge, BridgeConfig};

#[tokio::main]
async fn main() {
    let config = AviP2pConfig::default();
    let (node, mut events) = AviP2p::start(config).await.unwrap();
    
    // Start UDP bridge on port 8888
    EmbeddedBridge::start(
        node.handle(),
        BridgeConfig { udp_port: 8888 }
    ).await.unwrap();
    
    // Handle events
    while let Some(event) = events.recv().await {
        println!("{:?}", event);
    }
}
```

## 📡 Protocol Flow

### Device → Gateway (Uplink)
1. **Hello** - Initial handshake with device_id
2. **Subscribe** - Request topic subscription
3. **Publish** - Send data to topic
4. **SensorUpdate** - Report sensor readings
5. **ButtonPress** - Report button events

### Gateway → Device (Downlink)
1. **Welcome** - Handshake acknowledgment
2. **SubscribeAck** - Subscription confirmed
3. **Message** - Incoming pub/sub message

## 🎨 API Reference

### Connection Management
```rust
// Connect to gateway
device.connect().await?;

// Check connection status
if device.is_connected() { /* ... */ }
```

### Pub/Sub
```rust
// Subscribe to topics
device.subscribe("avi/home/sensors").await?;
device.subscribe("avi/home/device_1234/command").await?;

// Unsubscribe
device.unsubscribe("avi/home/sensors").await?;

// Publish messages
device.publish("avi/home/status", b"online").await?;

// Poll for incoming messages (call in main loop)
device.poll().await?;
```

### Sensor Reporting
```rust
use avi_p2p_protocol::SensorValue;

device.update_sensor("temp", SensorValue::Temperature(22.5)).await?;
device.update_sensor("humidity", SensorValue::Humidity(65.0)).await?;
device.update_sensor("battery", SensorValue::Battery(85)).await?;
device.update_sensor("status", SensorValue::Status(true)).await?;
```

### Button Events
```rust
use avi_p2p_protocol::PressType;

device.button_pressed(1, PressType::Single).await?;
device.button_pressed(2, PressType::Double).await?;
device.button_pressed(3, PressType::Long).await?;
```

### Audio Streaming
```rust
// Request stream to peer
device.start_stream(1, "peer_id_here", "voice_call").await?;

// Send audio data
device.send_audio(1, &pcm_buffer).await?;

// Close stream
device.close_stream(1).await?;
```

## 🧪 Testing

### Run Simulated Device (PC)
```bash
cargo run --example simulated_mcu_pubsub
```

### Run Gateway
```bash
cargo run --example gateway_node
```

### Run Monitor (to see messages)
```bash
cargo run --example monitor_node
```

## 🔍 Topic Naming Convention

**Recommended structure:**
- `avi/home/device_{id}/sensor/{name}` - Sensor updates
- `avi/home/device_{id}/button` - Button events
- `avi/home/device_{id}/command` - Commands to device
- `avi/home/device_{id}/status` - Device status
- `avi/home/broadcast` - Broadcast messages

**Wildcards in subscriptions:**
- `+` - Single level wildcard
- `#` - Multi-level wildcard

Examples:
```rust
device.subscribe("avi/home/+/sensor/temperature").await?;  // All temp sensors
device.subscribe("avi/home/device_1234/#").await?;         // All device 1234 topics
```

## 📊 Message Flow Example

```
[ESP32 Device]                [Gateway]                    [P2P Mesh]
     |                            |                              |
     |--Hello(device_id=1234)---->|                              |
     |<-------Welcome-------------|                              |
     |                            |                              |
     |--Subscribe("avi/cmd")----->|                              |
     |                            |--Subscribe("avi/cmd")------->|
     |<----SubscribeAck-----------|                              |
     |                            |                              |
     |--SensorUpdate(temp)------->|                              |
     |                            |--Publish("avi/.../temp")---->|
     |                            |                              |
     |                            |<---Message("avi/cmd")--------|
     |<----Message("avi/cmd")-----|                              |
```

## 💡 Best Practices

1. **Call `poll()` regularly** - This is how you receive messages
2. **Keep message handler fast** - Don't block in `on_message()`
3. **Use appropriate buffer sizes** - 1024 bytes is usually sufficient
4. **Handle connection failures** - Implement reconnection logic
5. **Namespace your topics** - Use device_id in topic names

## 🐛 Troubleshooting

**Device won't connect:**
- Verify gateway is running and UDP port 8888 is open
- Check WiFi connection on ESP32
- Confirm gateway IP address is correct

**Not receiving messages:**
- Make sure you're calling `poll()` in your main loop
- Verify subscription was successful
- Check topic names match exactly

**ESP32 crashes:**
- Increase stack size in `sdkconfig`
- Reduce buffer sizes if memory is tight
- Enable panic handler for debugging

## 📝 License

MIT
