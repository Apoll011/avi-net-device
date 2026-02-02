use avi_p2p_embedded::{AviEmbedded, AviEmbeddedConfig, UdpClient, MessageHandler};
use avi_p2p_protocol::{PressType, SensorValue};
use tokio::net::UdpSocket;
use std::net::SocketAddr;
use std::io;

// --- MOCK HARDWARE LAYER ---
struct PcSocket {
    inner: UdpSocket,
    target: SocketAddr,
}

impl PcSocket {
    async fn new(bind_addr: &str, target_addr: &str) -> io::Result<Self> {
        let sock = UdpSocket::bind(bind_addr).await?;
        sock.set_nonblocking(true)?;
        let target = target_addr.parse().unwrap();
        Ok(Self { inner: sock, target })
    }
}

impl UdpClient for PcSocket {
    type Error = io::Error;

    async fn send(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.inner.send_to(buf, self.target).await.map(|_| ())
    }

    async fn receive(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self.inner.try_recv_from(buf) {
            Ok((len, _)) => Ok(len),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                Err(e)
            }
            Err(e) => Err(e),
        }
    }
}

// --- MESSAGE HANDLER ---
struct DeviceMessageHandler {
    device_id: u64,
}

impl MessageHandler for DeviceMessageHandler {
    fn on_message(&mut self, topic: &str, data: &[u8]) {
        println!("📨 [Device {}] Message on topic: {}", self.device_id, topic);
        
        if let Ok(msg) = std::str::from_utf8(data) {
            println!("   Content: {}", msg);
            
            // Handle commands
            if topic.contains("/command") {
                match msg.trim() {
                    "status" => println!("   ✅ Status: Online and operational"),
                    "temp" => println!("   🌡️  Temperature sensor ready"),
                    _ => println!("   ⚠️  Unknown command: {}", msg),
                }
            }
        } else {
            println!("   Binary data: {} bytes", data.len());
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 SIMULATED EMBEDDED DEVICE WITH PUB/SUB");
    println!("==========================================\n");

    // 1. Setup Network
    let socket = PcSocket::new("127.0.0.1:0", "127.0.0.1:8888").await?;

    // 2. Setup Embedded Library
    let device_id = 5555;
    let mut scratch_buffer = [0u8; 1024];
    let config = AviEmbeddedConfig { device_id };
    let handler = DeviceMessageHandler { device_id };

    let mut mcu = AviEmbedded::new(socket, config, &mut scratch_buffer, handler);

    // 3. Connect Phase
    println!("⏳ Connecting to Gateway...");
    loop {
        if mcu.connect().await.is_ok() && mcu.is_connected() {
            println!("✅ Connected!\n");
            break;
        }
        println!("⚠️  Retrying...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    // 4. Subscribe to Topics
    println!("📥 Subscribing to topics...");
    
    // Subscribe to commands for this device
    mcu.subscribe(&format!("avi/home/device_{}/command", device_id)).await?;
    println!("   ✓ Subscribed to device commands");
    
    // Subscribe to broadcasts
    mcu.subscribe("avi/home/broadcast").await?;
    println!("   ✓ Subscribed to broadcast messages");
    
    // Subscribe to other devices' sensor data (for monitoring)
    mcu.subscribe("avi/home/+/sensor/#").await?;
    println!("   ✓ Subscribed to all sensor updates\n");

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 5. Main Device Loop
    println!("🎉 Device Ready! Starting main loop...\n");
    let mut counter = 0;

    loop {
        // Poll for incoming messages (non-blocking)
        let _ = mcu.poll().await;

        // Periodic operations
        if counter % 20 == 0 {
            // Simulate button press
            println!("👉 [MCU] Button pressed (double-click)");
            mcu.button_pressed(1, PressType::Double).await?;
        }

        if counter % 30 == 0 {
            // Simulate temperature reading
            let temp = 20.0 + (counter as f32 * 0.2) % 15.0;
            println!("🌡️  [MCU] Temperature: {:.1}°C", temp);
            mcu.update_sensor("kitchen_temp", SensorValue::Temperature(temp)).await?;
        }

        if counter % 50 == 0 {
            // Publish custom message
            let status = format!(r#"{{"device_id":{},"uptime":{},"status":"ok"}}"#, device_id, counter);
            println!("📤 [MCU] Publishing status...");
            mcu.publish(
                &format!("avi/home/device_{}/status", device_id),
                status.as_bytes()
            ).await?;
        }

        if counter % 40 == 0 {
            // Simulate humidity reading
            let humidity = 45.0 + (counter as f32 * 0.15) % 30.0;
            println!("💧 [MCU] Humidity: {:.1}%", humidity);
            mcu.update_sensor("kitchen_humidity", SensorValue::Humidity(humidity)).await?;
        }

        counter += 1;
        println!("--------------------------------");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
}
