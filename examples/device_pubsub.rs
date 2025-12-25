use avi_device::device::{AviDevice, AviDeviceConfig, AviDeviceType};
use avi_device::DeviceCapabilities;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), String> {
    // 1. Configure the device
    let config = AviDeviceConfig {
        node_name: "example-pubsub-node".to_string(),
        device_type: AviDeviceType::NODE,
        capabilities: DeviceCapabilities::default(),
    };

    // 2. Initialize the device
    let device = AviDevice::new(config).await?;

    // 3. Start the event loop in a background task
    let device_clone = device.clone();
    tokio::spawn(async move {
        device_clone.start_event_loop().await;
    });

    // 4. Subscribe to a topic
    device.subscribe("home/sensors/temp", |from, topic, data| {
        let msg = String::from_utf8_lossy(&data);
        println!("📩 Received on '{}' from {}: {}", topic, from, msg);
    }).await.map_err(|e| e.to_string())?;

    println!("🚀 Device started. Publishing messages every 2 seconds...");

    // 5. Publish messages periodically
    let mut count = 0;
    loop {
        count += 1;
        let message = format!("Temperature Update #{}", count);
        println!("📤 Publishing: {}", message);
        
        device.publish("home/sensors/temp", message.into_bytes())
            .await
            .map_err(|e| e.to_string())?;

        sleep(Duration::from_secs(2)).await;
        
        if count >= 5 {
            break;
        }
    }

    println!("✅ Example finished.");
    Ok(())
}
