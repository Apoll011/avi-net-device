# ESP32 Setup Guide for AVI P2P Embedded

## Prerequisites

### 1. Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Install ESP-IDF Prerequisites

**Linux/macOS:**
```bash
# Install dependencies
sudo apt-get install git wget flex bison gperf python3 python3-pip python3-venv cmake ninja-build ccache libffi-dev libssl-dev dfu-util libusb-1.0-0
```

**macOS:**
```bash
brew install cmake ninja dfu-util
```

### 3. Install ESP Rust Toolchain
```bash
# Install espup
cargo install espup

# Install ESP toolchains
espup install

# Source the export file (add to .bashrc/.zshrc)
source $HOME/export-esp.sh
```

### 4. Install espflash
```bash
cargo install espflash
```

## Project Setup

### 1. Clone the Repository
```bash
git clone <your-repo>
cd avi-p2p
```

### 2. Configure WiFi Credentials

Edit `examples/esp32_device.rs`:
```rust
let wifi_config = WifiConfig {
    ssid: "YourNetworkName",      // <- Change this
    password: "YourPassword",      // <- Change this
};
```

### 3. Configure Gateway IP

Edit the socket creation line:
```rust
let socket = match EspUdpSocket::new([192, 168, 1, 100], 8888) {
//                                     ^^^^^^^^^^^^^^^^^^^
//                                     Change to your gateway IP
```

## Building and Flashing

### Build for ESP32
```bash
cd embedded-esp
cargo build --release --example esp32_device
```

### Flash to Device
```bash
# Flash and monitor
cargo run --release --example esp32_device

# Or manually
espflash flash --monitor target/xtensa-esp32-espidf/release/esp32_device
```

### Monitor Serial Output
```bash
espflash monitor
```

## Supported ESP32 Variants

- **ESP32** - Original (Xtensa)
- **ESP32-S2** - Single core (Xtensa)
- **ESP32-S3** - Dual core (Xtensa)
- **ESP32-C3** - RISC-V (requires different target)

To target ESP32-C3, change `.cargo/config.toml`:
```toml
[build]
target = "riscv32imc-esp-espidf"
```

## Memory Considerations

### Stack Size
If you encounter stack overflows, increase in `sdkconfig.defaults`:
```
CONFIG_ESP_MAIN_TASK_STACK_SIZE=16000
```

### Heap Size
Monitor heap usage:
```rust
esp_println::println!("Free heap: {} bytes", 
    esp_idf_sys::esp_get_free_heap_size());
```

### Buffer Optimization
Reduce buffer if memory is tight:
```rust
let mut scratch_buffer = [0u8; 512];  // Instead of 1024
```

## Common Issues

### 1. "Cannot connect to WiFi"
- Check SSID and password
- Verify WiFi is 2.4GHz (ESP32 doesn't support 5GHz)
- Check signal strength

**Solution:**
```rust
// Add retry logic
for attempt in 1..=5 {
    match init_wifi(wifi_config) {
        Ok(_) => break,
        Err(e) => {
            println!("Attempt {}/5 failed: {:?}", attempt, e);
            esp_idf_sys::vTaskDelay(2000);
        }
    }
}
```

### 2. "Socket creation failed"
- Gateway may not be reachable
- Check IP address configuration
- Verify gateway is running

**Debug:**
```rust
// Test network connectivity first
use esp_idf_svc::ping::EspPing;
let mut ping = EspPing::new(0);
match ping.ping([192, 168, 1, 100].into(), &Default::default()) {
    Ok(_) => println!("Gateway reachable!"),
    Err(e) => println!("Cannot reach gateway: {:?}", e),
}
```

### 3. "Task watchdog timeout"
- Main task is blocking
- Add delays in tight loops
- Yield to FreeRTOS

**Solution:**
```rust
loop {
    // Your code here
    
    // Yield to other tasks
    esp_idf_sys::vTaskDelay(10);  // 10ms delay
}
```

### 4. Compilation Errors
```bash
# Clean build
cargo clean

# Update toolchain
espup update

# Reinstall ESP toolchain
espup install
```

## Development Workflow

### 1. Start Gateway on PC
```bash
cd avi-p2p
cargo run --example gateway_node
```

### 2. Flash ESP32
```bash
cd embedded-esp
cargo run --release --example esp32_device
```

### 3. Monitor Both
Terminal 1: Gateway logs
Terminal 2: ESP32 serial monitor

### 4. Test Pub/Sub
From another terminal, run:
```bash
cargo run --example monitor_node
```

## Performance Tips

### Optimize Binary Size
In `Cargo.toml`:
```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

### Faster Compilation
```bash
# Use mold linker (Linux)
sudo apt install mold
export RUSTFLAGS="-C link-arg=-fuse-ld=mold"

# Or use lld
export RUSTFLAGS="-C link-arg=-fuse-ld=lld"
```

### Reduce Flash Time
```bash
# Only flash, don't rebuild if not needed
espflash flash target/xtensa-esp32-espidf/release/esp32_device
```

## Testing Without Hardware

Use the PC simulator:
```bash
cargo run --example simulated_mcu_pubsub
```

This is useful for:
- Protocol testing
- Message handler development
- Integration testing

## Production Considerations

### 1. OTA Updates
Add OTA support for remote firmware updates.

### 2. TLS/Encryption
For production, encrypt UDP packets:
```rust
// Example: Use AES-GCM for packet encryption
```

### 3. Reconnection Logic
```rust
loop {
    if !device.is_connected() {
        println!("Reconnecting...");
        while device.connect().await.is_err() {
            esp_idf_sys::vTaskDelay(5000);
        }
    }
    
    // Normal operation
    let _ = device.poll().await;
    esp_idf_sys::vTaskDelay(100);
}
```

### 4. Watchdog Management
```rust
use esp_idf_sys::{esp_task_wdt_add, esp_task_wdt_reset};

unsafe {
    esp_task_wdt_add(core::ptr::null_mut());
    
    loop {
        // Your code
        esp_task_wdt_reset();
    }
}
```

## Resources

- [ESP-IDF Documentation](https://docs.espressif.com/projects/esp-idf/en/latest/)
- [ESP Rust Book](https://esp-rs.github.io/book/)
- [Awesome ESP Rust](https://github.com/esp-rs/awesome-esp-rust)

## Getting Help

1. Check serial output for error messages
2. Enable debug logging: `cargo run --release --example esp32_device -- --log-level debug`
3. Review ESP-IDF logs in `build/` directory
4. Join ESP32 Rust community forums
