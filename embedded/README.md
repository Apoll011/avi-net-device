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
# AVI Embedded - C API for ESP-IDF

Complete C API wrapper for the AVI Embedded Rust library with full async support for ESP32 devices.

## 🚀 Quick Start

```bash
# 1. Build the library
chmod +x build.sh
./build.sh xtensa-esp32-espidf

# 2. Copy to your ESP-IDF project
cp output/* your-esp-project/components/avi-embedded/

# 3. Use in your C code
#include "avi_embedded.h"
```

## ✨ Features

- ✅ **Non-blocking API** - All functions return immediately
- ✅ **Async by default** - Uses Embassy async runtime under the hood
- ✅ **FreeRTOS friendly** - Works perfectly with ESP-IDF tasks
- ✅ **Auto-generated headers** - Using cbindgen for type safety
- ✅ **Zero-copy** - Efficient memory usage
- ✅ **Full protocol support** - Pub/Sub, Streaming, Sensors, Events

## 📦 What's Included

| File | Description |
|------|-------------|
| `lib.rs` | Original Rust code (unchanged) |
| `c_api.rs` | C FFI wrapper with async support |
| `avi_embedded.h` | C header file (auto-generated) |
| `Cargo.toml` | Build configuration |
| `cbindgen.toml` | Header generation config |
| `build.sh` | Convenience build script |
| `esp_idf_example.c` | Complete ESP-IDF example |
| `BUILD_GUIDE.md` | Detailed documentation |

## 🔧 Requirements

- Rust with ESP targets installed
- ESP-IDF (v4.4+)
- cbindgen (optional, for header generation)

## 📖 API Overview

### Initialization

```c
// Call once at startup
avi_embedded_init();

// Create instance
CAviEmbeddedConfig config = { .device_id = 0x123 };
CAviEmbedded* avi = avi_embedded_new(
    config, buffer, buf_len,
    udp_ctx, udp_send, udp_recv,
    msg_ctx, msg_callback
);
```

### Core Operations (All Non-blocking!)

```c
// Connection
avi_embedded_connect(avi);              // Returns immediately
avi_embedded_is_connected(avi);         // Check status

// Pub/Sub
avi_embedded_subscribe(avi, "topic", 5);
avi_embedded_publish(avi, "topic", 5, data, len);
avi_embedded_unsubscribe(avi, "topic", 5);

// Streaming
avi_embedded_start_stream(avi, id, peer, peer_len, reason, reason_len);
avi_embedded_send_audio(avi, id, pcm, pcm_len);
avi_embedded_close_stream(avi, id);

// Sensors & Events
avi_embedded_button_pressed(avi, btn_id, press_type);
avi_embedded_update_sensor_float(avi, "temp", 4, 25.5f);
avi_embedded_update_sensor_int(avi, "count", 5, 42);

// Message polling
avi_embedded_poll(avi);                 // Call in main loop
```

### Return Values

- `0` = Success (command queued)
- `-1` = Invalid parameters
- `-2` = Command queue full (retry later)

## 🎯 How It Works

### Async Architecture

```
┌─────────────┐
│   Your C    │  ← Non-blocking calls
│     App     │  ← Returns immediately
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Command   │  ← Queue of operations
│    Queue    │  ← (16 commands default)
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Embassy   │  ← Async processing
│   Runtime   │  ← Handles network I/O
└─────────────┘
```

### Key Benefits

1. **Non-blocking**: Your main loop never waits
2. **Backpressure**: Queue full? Just retry later
3. **Efficient**: Embassy runtime handles scheduling
4. **Simple**: Looks like sync code, works async

## 📝 Example

```c
void app_main(void) {
    // Initialize
    avi_embedded_init();
    
    // Create instance
    static uint8_t buffer[2048];
    CAviEmbedded* avi = avi_embedded_new(...);
    
    // Connect (non-blocking!)
    avi_embedded_connect(avi);
    
    // Subscribe (non-blocking!)
    avi_embedded_subscribe(avi, "sensors/temp", 12);
    
    // Main loop
    while (1) {
        // Poll for messages (non-blocking!)
        avi_embedded_poll(avi);
        
        // Send data (non-blocking!)
        avi_embedded_publish(avi, "status", 6, data, len);
        
        vTaskDelay(pdMS_TO_TICKS(100));
    }
}
```

## 🏗️ Building

### For ESP32

```bash
./build.sh xtensa-esp32-espidf
```

### For ESP32-C3

```bash
./build.sh riscv32imc-esp-espidf
```

### All Targets

| ESP Chip | Target |
|----------|--------|
| ESP32 | `xtensa-esp32-espidf` |
| ESP32-S2 | `xtensa-esp32s2-espidf` |
| ESP32-S3 | `xtensa-esp32s3-espidf` |
| ESP32-C3 | `riscv32imc-esp-espidf` |
| ESP32-C6 | `riscv32imac-esp-espidf` |

## 📚 Documentation

See `BUILD_GUIDE.md` for:
- Detailed build instructions
- ESP-IDF integration guide
- Complete API reference
- Troubleshooting tips
- Memory usage guidelines

See `esp_idf_example.c` for:
- Full working example
- UDP callback implementations
- Message handling
- Error handling patterns

## 🔍 Auto-Generated Headers

The header file is automatically generated using cbindgen:

```bash
cbindgen --config cbindgen.toml --output avi_embedded.h
```

Benefits:
- Always in sync with Rust code
- Type-safe
- Includes documentation
- No manual maintenance

## ⚡ Performance

Typical memory usage:
- AVI instance: ~200 bytes
- Scratch buffer: 2048 bytes (your allocation)
- Command queue: ~1KB
- **Total: ~3-4KB**

Command queue:
- Default size: 16 commands
- Adjustable in `c_api.rs`
- Fast: < 1μs per queue operation

## 🐛 Troubleshooting

### Queue Full (-2 errors)

```c
int ret = avi_embedded_publish(avi, ...);
if (ret == -2) {
    vTaskDelay(pdMS_TO_TICKS(10));  // Back off
    ret = avi_embedded_publish(avi, ...);  // Retry
}
```

### Stack Overflow

Increase task stack size:

```c
xTaskCreate(my_task, "avi", 8192, NULL, 5, NULL);
//                              ^^^^
//                          Increase this
```

### Linker Errors

Add to `CMakeLists.txt`:

```cmake
target_link_libraries(${COMPONENT_LIB} INTERFACE 
    -Wl,--whole-archive
    libavi_embedded.a
    -Wl,--no-whole-archive
    gcc m c stdc++
)
```

## 🤝 Contributing

The C API is generated from Rust code. To add features:

1. Add Rust function in `lib.rs` (original API)
2. Add C wrapper in `c_api.rs` (FFI layer)
3. Regenerate header: `cbindgen --output avi_embedded.h`
4. Update examples and docs

