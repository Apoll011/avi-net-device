use avi_device::device::{AviDevice, AviDeviceConfig, AviDeviceType};
use avi_device::DeviceQuery;
use avi_device::capability::{CapabilityBuilder, SensorCapability};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), String> {
    // 1. Define capabilities for our device
    let caps = CapabilityBuilder::new()
        .sensor("microphone", SensorCapability::Microphone {
            present: true,
            array_size: 4,
            sampling_rate_khz: 48,
            max_spl_db: 120,
        })
        .sensor("temperature", SensorCapability::Temperature {
            present: true,
            accuracy_celsius: 0.5,
            current_value: Some(22.5),
        })
        .build();

    // 2. Configure the device
    let config = AviDeviceConfig {
        node_name: "smart-sensor-01".to_string(),
        device_type: AviDeviceType::NODE,
        capabilities: caps,
    };

    // 3. Initialize the device
    let device = AviDevice::new(config).await?;

    // 4. Start the event loop
    let device_clone = device.clone();
    tokio::spawn(async move {
        device_clone.start_event_loop().await;
    });

    println!("🔍 Device Query & Discovery Example");
    
    // Wait for the node to start and update its own capabilities in the context
    sleep(Duration::from_secs(1)).await;

    // 5. Create a query to find devices with microphones
    let query = DeviceQuery::new()
        .sensor("microphone", |s| {
            if let SensorCapability::Microphone { present, .. } = s {
                *present
            } else {
                false
            }
        });

    // 6. Execute the query
    println!("🔎 Searching for devices with microphones...");
    let results = device.execute_query(query)
        .await
        .map_err(|e| e.to_string())?;

    println!("✅ Found {} device(s): {:?}", results.len(), results);

    // 7. Search for temperature sensors
    let temp_query = DeviceQuery::new()
        .sensor("temperature", |s| matches!(s, SensorCapability::Temperature { present: true, .. }));

    println!("🔎 Searching for devices with temperature sensors...");
    let temp_results = device.execute_query(temp_query)
        .await
        .map_err(|e| e.to_string())?;

    println!("✅ Found {} device(s): {:?}", temp_results.len(), temp_results);

    println!("✅ Query example finished.");
    Ok(())
}
