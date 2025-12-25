use avi_device::device::{AviDevice, AviDeviceConfig, AviDeviceType};
use avi_device::capability::{CapabilityBuilder, SensorCapability};
use avi_device::stream::{StreamContext, StreamHandler, StreamHandlerFactory};
use avi_p2p::{PeerId, StreamId, StreamCloseReason};
use async_trait::async_trait;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use serde_json::json;

// --- Stream Handler for Chat ---

struct ChatStreamHandler {
    peer_id: Option<PeerId>,
}

#[async_trait]
impl StreamHandler for ChatStreamHandler {
    async fn on_accepted(&mut self, ctx: &StreamContext) {
        self.peer_id = Some(ctx.peer_id.clone());
        println!("\n[Stream] Chat stream established with {}", ctx.peer_id);
        print!("> ");
        io::stdout().flush().unwrap();
    }

    async fn on_rejected(&mut self, peer_id: PeerId, _stream_id: StreamId, reason: String) {
        println!("\n[Stream] Chat request to {} rejected: {}", peer_id, reason);
        print!("> ");
        io::stdout().flush().unwrap();
    }

    async fn on_data(&mut self, ctx: &StreamContext, data: Vec<u8>) {
        if let Ok(msg) = String::from_utf8(data) {
            println!("\n[Stream Message from {}] {}", ctx.peer_id, msg);
        } else {
            println!("\n[Stream] Received non-utf8 data from {}", ctx.peer_id);
        }
        print!("> ");
        io::stdout().flush().unwrap();
    }

    async fn on_closed(&mut self, peer_id: PeerId, _stream_id: StreamId, reason: StreamCloseReason) {
        println!("\n[Stream] Chat with {} closed ({:?})", peer_id, reason);
        print!("> ");
        io::stdout().flush().unwrap();
    }
}

struct ChatStreamFactory;

#[async_trait]
impl StreamHandlerFactory for ChatStreamFactory {
    async fn create_handler(&self) -> Box<dyn StreamHandler> {
        Box::new(ChatStreamHandler { peer_id: None })
    }
}

// --- Main CLI Application ---

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("🚀 AVI Device Full Feature CLI Example");
    println!("Type 'help' for available commands.");

    // 1. Setup Device
    let caps = CapabilityBuilder::new()
        .sensor("microphone", SensorCapability::Microphone {
            present: true,
            array_size: 2,
            sampling_rate_khz: 44,
            max_spl_db: 110,
        })
        .sensor("cli_node", SensorCapability::Temperature {
            present: true,
            accuracy_celsius: 0.1,
            current_value: Some(1.0),
        })
        .build();

    let node_name = format!("cli-node-{}", std::process::id());
    let config = AviDeviceConfig {
        node_name,
        device_type: AviDeviceType::NODE,
        capabilities: caps,
        can_gateway_embedded: true,
    };

    let device = AviDevice::new(config).await?;
    
    // Register the chat stream handler
    device.register_stream_handler("chat".to_string(), ChatStreamFactory).await;

    // 2. Start Event Loop
    let device_clone = device.clone();
    tokio::spawn(async move {
        device_clone.start_event_loop().await;
    });

    // 3. Subscription Handler
    device.subscribe("global", move |from, topic, data| {
        let msg = String::from_utf8_lossy(&data);
        println!("\n[PubSub] {} on {}: {}", from, topic, msg);
        print!("> ");
        let _ = io::stdout().flush();
    }).await.map_err(|e| e.to_string())?;

    // 4. CLI Loop
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let active_stream: Arc<Mutex<Option<StreamId>>> = Arc::new(Mutex::new(None));

    print!("> ");
    io::stdout().flush().unwrap();

    while let Ok(Some(line)) = lines.next_line().await {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            print!("> ");
            io::stdout().flush().unwrap();
            continue;
        }

        match parts[0] {
            "help" => {
                println!("Available commands:");
                println!("  peers              - List connected peers");
                println!("  status             - Show local node info");
                println!("  pub <msg>          - Publish to 'global' topic");
                println!("  set <path> <val>   - Update context (e.g. set user.name \"Alice\")");
                println!("  get <path>         - Get context value");
                println!("  query              - Find other CLI nodes");
                println!("  call <peer_id>     - Open a chat stream");
                println!("  msg <text>         - Send message over active stream");
                println!("  hangup             - Close active stream");
                println!("  exit               - Quit");
            }
            "peers" => {
                match device.get_peers().await {
                    Ok(p) => println!("Connected peers: {:?}", p),
                    Err(e) => println!("Error: {}", e),
                }
            }
            "status" => {
                if let Ok(id) = device.get_core_id().await {
                    println!("Core ID: {}", id);
                } else {
                    println!("Core ID: Not set yet (waiting for discovery)");
                }
                // Note: local peer id is not directly exposed in AviDevice yet, 
                // but we can see it from events or context.
                if let Ok(ctx) = device.get_ctx("").await {
                    println!("Local Context Keys: {:?}", ctx.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                }
            }
            "sub" if parts.len() > 1 => {
                if let Err(e) = device.subscribe(parts[1], |from, _topic, data| println!("Got from {from} data: {:?}", data)).await {
                    println!("Failed to subscribe: {}", e);
                }
                println!("Subscribed to '{}'", parts[1]);
            },
            "pub" if parts.len() > 2 => {
                let topic = parts[1];
                let content = parts[2..].join(" ");
                if let Err(e) = device.publish(topic, content.into_bytes()).await {
                    println!("Failed to publish: {}", e);
                }
            },
            "set" => {
                if parts.len() < 3 {
                    println!("Usage: set <path> <value>");
                } else {
                    let path = parts[1];
                    let val = parts[2..].join(" ");
                    let json_val = serde_json::from_str(&val).unwrap_or(json!(val));
                    if let Err(e) = device.update_ctx(path, json_val).await {
                        println!("Failed to update context: {}", e);
                    }
                }
            }
            "get" => {
                let path = if parts.len() > 1 { parts[1] } else { "" };
                match device.get_ctx(path).await {
                    Ok(v) => println!("{}: {}", path, v),
                    Err(e) => println!("Error: {}", e),
                }
            }
            "query" => {
                use avi_device::DeviceQuery;
                let query = DeviceQuery::new().sensor("cli_node", |_| true);
                match device.execute_query(query).await {
                    Ok(results) => println!("Found CLI nodes: {:?}", results),
                    Err(e) => println!("Query failed: {}", e),
                }
            }
            "call" => {
                if parts.len() < 2 {
                    println!("Usage: call <peer_id>");
                } else {
                    let peer_id = PeerId::new(parts[1]);
                    match device.request_stream(peer_id, "chat".to_string()).await {
                        Ok(id) => {
                            println!("Stream request sent. ID: {}", id);
                            let mut lock = active_stream.lock().await;
                            *lock = Some(id);
                        }
                        Err(e) => println!("Failed to request stream: {}", e),
                    }
                }
            }
            "msg" => {
                if parts.len() < 2 {
                    println!("Usage: msg <text>");
                } else {
                    let stream_id = { *active_stream.lock().await };
                    if let Some(id) = stream_id {
                        let text = parts[1..].join(" ");
                        if let Err(e) = device.send_stream_data(id, text.into_bytes()).await {
                            println!("Failed to send stream message: {}", e);
                        }
                    } else {
                        println!("No active stream. Use 'call' first.");
                    }
                }
            }
            "hangup" => {
                let mut lock = active_stream.lock().await;
                if let Some(id) = lock.take() {
                    if let Err(e) = device.close_stream(id).await {
                        println!("Error closing stream: {}", e);
                    } else {
                        println!("Stream closed.");
                    }
                } else {
                    println!("No active stream.");
                }
            }
            "exit" => break,
            _ => println!("Unknown command: {}. Type 'help' for help.", parts[0]),
        }

        print!("> ");
        io::stdout().flush().unwrap();
    }

    Ok(())
}
