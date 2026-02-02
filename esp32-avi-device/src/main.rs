#![no_std]
#![no_main]

extern crate alloc;
use alloc::string::String;

use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::sys as _;
use esp_println::println;

use avi_p2p_embedded::{AviEmbedded, AviEmbeddedConfig, MessageHandler};
use avi_p2p_embedded_esp::{EspUdpSocket, WifiConfig, init_wifi, helpers};
use avi_p2p_protocol::{PressType, SensorValue};

// Custom message handler with state
struct DeviceHandler {
    led_state: bool,
}

impl MessageHandler for DeviceHandler {
    fn on_message(&mut self, topic: &str, data: &[u8]) {
        println!("📨 Message on '{}': {} bytes", topic, data.len());
        
        // Parse command messages
        if topic == "avi/home/device_1234/command" {
            if let Ok(cmd) = core::str::from_utf8(data) {
                println!("   Command: {}", cmd);
                
                match cmd {
                    "led_on" => {
                        self.led_state = true;
                        println!("💡 LED turned ON");
                    }
                    "led_off" => {
                        self.led_state = false;
                        println!("💡 LED turned OFF");
                    }
                    _ => println!("⚠️  Unknown command"),
                }
            }
        }
        
        // Handle other topics
        if topic.starts_with("avi/home/") && topic.contains("/sensor/") {
            if let Ok(json_str) = core::str::from_utf8(data) {
                println!("   Sensor data: {}", json_str);
            }
        }
    }
}

#[esp_idf_svc::sys::link_section = ".iram1.text"]
fn main() -> ! {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("🚀 AVI Embedded Device Starting...");
    println!("====================================");

    // 1. Initialize WiFi
    println!("📡 Connecting to WiFi...");
    let wifi_config = WifiConfig {
        ssid: "YourSSID",
        password: "YourPassword",
    };
    
    match init_wifi(wifi_config) {
        Ok(_) => println!("✅ WiFi Connected!"),
        Err(e) => {
            println!("❌ WiFi Error: {:?}", e);
            loop {
                esp_idf_svc::sys::vTaskDelay(1000);
            }
        }
    }

    // Give network stack time to settle
    esp_idf_svc::sys::vTaskDelay(2000);

    // 2. Create UDP Socket to Gateway
    println!("🔌 Connecting to Gateway...");
    let socket = match EspUdpSocket::new([192, 168, 1, 100], 8888) {
        Ok(s) => s,
        Err(e) => {
            println!("❌ Socket Error: {}", e);
            loop {
                esp_idf_svc::sys::vTaskDelay(1000);
            }
        }
    };

    // 3. Setup AVI Client
    let mut scratch_buffer = [0u8; 1024];
    let config = AviEmbeddedConfig { device_id: 1234 };
    let handler = DeviceHandler { led_state: false };
    
    let mut device = AviEmbedded::new(socket, config, &mut scratch_buffer, handler);

    // 4. Main Task Loop
    let executor = esp_idf_svc::hal::task::executor::EspExecutor::new();
    
    executor.spawn_detached(async move {
        // Connect to gateway
        println!("⏳ Handshaking with Gateway...");
        loop {
            match device.connect().await {
                Ok(_) if device.is_connected() => {
                    println!("✅ Connected to Gateway!");
                    break;
                }
                _ => {
                    println!("⚠️  Retrying connection...");
                    esp_idf_svc::sys::vTaskDelay(2000);
                }
            }
        }

        // Subscribe to command topic
        println!("📥 Subscribing to command topic...");
        let _ = device.subscribe("avi/home/device_1234/command").await;
        
        // Subscribe to sensor updates from other devices
        let _ = device.subscribe("avi/home/+/sensor/#").await;

        println!("🎉 Device Ready!");
        println!("");

        let mut counter = 0u32;
        let mut button_pressed = false;

        loop {
            // Poll for incoming messages
            let _ = device.poll().await;

            // Simulate periodic sensor readings
            if counter % 50 == 0 {
                let temp = 20.0 + (counter as f32 * 0.1) % 10.0;
                println!("🌡️  Reading temperature: {:.1}°C", temp);
                
                let _ = helpers::report_temperature(
                    &mut device, 
                    "living_room", 
                    temp
                ).await;
            }

            // Simulate button press every 100 iterations
            if counter % 100 == 0 && !button_pressed {
                println!("👉 Button pressed (simulated)");
                let _ = helpers::report_button(
                    &mut device,
                    1,
                    PressType::Single
                ).await;
                button_pressed = true;
            }

            // Simulate humidity reading every 75 iterations
            if counter % 75 == 0 {
                let humidity = 45.0 + (counter as f32 * 0.05) % 20.0;
                println!("💧 Reading humidity: {:.1}%", humidity);
                
                let _ = helpers::report_humidity(
                    &mut device,
                    "living_room",
                    humidity
                ).await;
            }

            // Publish a status message every 200 iterations
            if counter % 200 == 0 {
                println!("📤 Publishing status update...");
                let status_msg = b"{\"online\":true,\"uptime\":";
                let _ = device.publish("avi/home/device_1234/status", status_msg).await;
            }

            counter += 1;
            button_pressed = counter % 100 != 0;

            // Small delay
            esp_idf_svc::sys::vTaskDelay(100);
        }
    }).unwrap();

    // Keep main task alive
    loop {
        esp_idf_svc::sys::vTaskDelay(1000);
    }
}
