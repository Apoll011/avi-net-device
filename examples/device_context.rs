use avi_device::device::{AviDevice, AviDeviceConfig, AviDeviceType};
use avi_device::DeviceCapabilities;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), String> {
    // 1. Configure the device
    let config = AviDeviceConfig {
        node_name: "example-context-node".to_string(),
        device_type: AviDeviceType::NODE,
        can_gateway_embedded: false,
        capabilities: DeviceCapabilities::default(),
    };

    // 2. Initialize the device
    let device = AviDevice::new(config).await?;

    // 3. Start the event loop
    let device_clone = device.clone();
    tokio::spawn(async move {
        device_clone.start_event_loop().await;
    });

    println!("🧠 Distributed Context Example");

    // 4. Update nested context
    println!("📝 Updating volume...");
    device.update_ctx("avi.device.audio.volume", json!(75))
        .await
        .map_err(|e| e.to_string())?;

    device.update_ctx("avi.device.audio.muted", json!(false))
        .await
        .map_err(|e| e.to_string())?;

    // Wait for propagation (simulated in this single-node example)
    sleep(Duration::from_millis(500)).await;

    // 5. Retrieve values
    let volume = device.get_ctx("avi.device.audio.volume")
        .await
        .map_err(|e| e.to_string())?;
    println!("🔈 Current Volume: {}", volume);

    // 6. Update whole object
    println!("📝 Updating status object...");
    device.update_ctx("avi.device.status", json!({
        "online": true,
        "battery": 92,
        "mode": "active"
    })).await.map_err(|e| e.to_string())?;

    sleep(Duration::from_millis(500)).await;

    // 7. Get nested value from the object we just uploaded
    let battery = device.get_ctx("avi.device.status.battery")
        .await
        .map_err(|e| e.to_string())?;
    println!("🔋 Battery Level: {}%", battery);

    // 8. Get full context
    let full_ctx = device.get_ctx("")
        .await
        .map_err(|e| e.to_string())?;
    println!("📊 Full Context: {}", serde_json::to_string_pretty(&full_ctx).unwrap());

    println!("✅ Context example finished.");
    Ok(())
}
