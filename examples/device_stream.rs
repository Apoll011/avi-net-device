use avi_device::device::{AviDevice, AviDeviceConfig, AviDeviceType};
use avi_device::{DeviceCapabilities, StreamHandler, StreamHandlerFactory, StreamContext, PeerId, StreamId, StreamCloseReason};
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::sleep;

// 1. Define a custom Stream Handler
pub struct ChatStreamHandler;

#[async_trait]
impl StreamHandler for ChatStreamHandler {
    async fn on_accepted(&mut self, ctx: &StreamContext) {
        println!("✅ Stream {} accepted with {}", ctx.stream_id, ctx.peer_id);
    }

    async fn on_rejected(&mut self, peer_id: PeerId, stream_id: StreamId, reason: String) {
        println!("❌ Stream {} rejected by {}. Reason: {}", stream_id, peer_id, reason);
    }

    async fn on_data(&mut self, ctx: &StreamContext, data: Vec<u8>) {
        let msg = String::from_utf8_lossy(&data);
        println!("💬 [{}] received: {}", ctx.peer_id, msg);
    }

    async fn on_closed(&mut self, peer_id: PeerId, stream_id: StreamId, reason: StreamCloseReason) {
        println!("🔇 Stream {} with {} closed. Reason: {:?}", stream_id, peer_id, reason);
    }
}

// 2. Define a Factory for the Handler
pub struct ChatStreamFactory;

#[async_trait]
impl StreamHandlerFactory for ChatStreamFactory {
    async fn create_handler(&self) -> Box<dyn StreamHandler> {
        Box::new(ChatStreamHandler)
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    // 3. Configure and start the device
    let config = AviDeviceConfig {
        node_name: "stream-node".to_string(),
        device_type: AviDeviceType::NODE,
        can_gateway_embedded: true,
        capabilities: DeviceCapabilities::default(),
    };

    let device = AviDevice::new(config).await?;
    let device_clone = device.clone();
    tokio::spawn(async move {
        device_clone.start_event_loop().await;
    });

    // 4. Register the stream handler for a specific "reason"
    println!("🔧 Registering 'chat' stream handler...");
    device.register_stream_handler("chat".to_string(), ChatStreamFactory).await;

    println!("🚀 Device ready. In a real scenario, another peer would request a 'chat' stream.");
    println!("Waiting 5 seconds before finishing the example...");
    
    // In this single-node example, we can't easily request a stream to ourselves 
    // unless we have another node. But we've demonstrated the setup.
    
    sleep(Duration::from_secs(5)).await;

    println!("✅ Stream example finished.");
    Ok(())
}
