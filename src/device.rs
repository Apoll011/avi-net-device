use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{RwLock, Mutex};
use avi_p2p::{AviEvent, AviP2p, AviP2pConfig, AviP2pError, AviP2pHandle, PeerId, StreamId};
use crate::capability::DeviceCapabilities;
use crate::DeviceQuery;
use crate::stream::{StreamDispatcher, StreamHandlerFactory};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AviDeviceType {
    CORE = 0,
    NODE = 1
}

impl Default for AviDeviceType {
    fn default() -> Self {
        Self::NODE
    }
}

#[derive(Clone)]
pub struct AviDeviceConfig {
    pub node_name: String,
    pub device_type: AviDeviceType,

    pub capabilities: DeviceCapabilities,
}

#[derive(Clone)]
pub struct AviDevice {
    config: Arc<AviDeviceConfig>,

    node: Arc<Mutex<Option<AviP2p>>>,
    events: Arc<Mutex<Option<Receiver<AviEvent>>>>,
    handler: AviP2pHandle,

    peer_id: Arc<RwLock<Option<PeerId>>>,

    stream_dispatcher: Arc<StreamDispatcher>,

    subscription_handlers: Arc<RwLock<HashMap<String, Arc<dyn Fn(PeerId, String, Vec<u8>) + Send + Sync>>>>,
}

impl AviDevice {
    pub async fn new(config: AviDeviceConfig) -> Result<Self, String> {
        match AviP2p::start(AviP2pConfig::new(&config.node_name)).await {
            Ok((node, events)) => Ok(Self {
                config: Arc::new(config),
                handler: node.handle(),
                stream_dispatcher: Arc::new(StreamDispatcher::new(node.handle())),
                node: Arc::new(Mutex::new(Some(node))),
                events: Arc::new(Mutex::new(Some(events))),
                peer_id: Arc::new(RwLock::new(None)),
                subscription_handlers: Arc::new(RwLock::new(HashMap::new())),
            }),
            Err(e) => Err(format!("Failed to start AVI P2P node: {}", e))
        }
    }

    async fn handle_event(&self, event: AviEvent) {
        match event {
            AviEvent::Started { local_peer_id, .. } => {
                {
                    let mut id = self.peer_id.write().await;
                    *id = Some(local_peer_id.clone());
                }
                self.update_capabilities(local_peer_id.clone().to_string()).await;

                let is_core = if let AviDeviceType::CORE = self.config.device_type { true } else { false };

                if is_core {
                    if self.get_ctx("avi.core").await.is_err() {
                        match self.update_ctx("avi.core", serde_json::Value::String(local_peer_id.to_string())).await {
                            Ok(..) => {},
                            Err(e) => println!("Failed to update AVI Core peer id: {}", e)
                        }
                    } else {
                        // Already exists, maybe log it
                    }
                }
            },

            AviEvent::PeerDiscovered { .. } => {},
            AviEvent::PeerConnected { .. } => {
                let local_id = {
                    self.peer_id.read().await.clone()
                };
                if let Some(local_peer_id) = local_id {
                    self.update_capabilities(local_peer_id.to_string()).await;
                }
            },
            AviEvent::PeerDisconnected { .. } => {},

            AviEvent::Message { from, topic, data } => {
                let handlers = self.subscription_handlers.read().await;
                if let Some(handler) = handlers.get(&topic) {
                    handler(from, topic, data);
                }
            },

            AviEvent::ContextUpdated { .. } => {},

            AviEvent::StreamRejected { peer_id, stream_id, reason } => {
                if let Err(e) = self.stream_dispatcher.handle_stream_rejected(peer_id, stream_id, reason).await {
                    eprintln!("Error handling stream rejected: {}", e);
                }
            },
            AviEvent::StreamRequested { from, stream_id, reason } => {
                if let Err(e) = self.stream_dispatcher.handle_stream_requested(from, stream_id, reason).await {
                    eprintln!("Error handling stream request: {}", e);
                }
            }

            AviEvent::StreamAccepted { peer_id, stream_id } => {
                if let Err(e) = self.stream_dispatcher.handle_stream_accepted(peer_id, stream_id).await {
                    eprintln!("Error handling stream accepted: {}", e);
                }
            }

            AviEvent::StreamData { from, stream_id, data } => {
                if let Err(e) = self.stream_dispatcher.handle_stream_data(from, stream_id, data).await {
                    eprintln!("Error handling stream data: {}", e);
                }
            }

            AviEvent::StreamClosed { peer_id, stream_id, reason } => {
                if let Err(e) = self.stream_dispatcher.handle_stream_closed(peer_id, stream_id, reason).await {
                    eprintln!("Error handling stream closed: {}", e);
                }
            }
        };
    }
    fn set_nested_value(data: &mut serde_json::Value, path: &str, new_value: serde_json::Value) -> Result<(), AviP2pError> {
        let keys: Vec<&str> = path.split('.').collect();

        if keys.is_empty() || (keys.len() == 1 && keys[0].is_empty()) {
            *data = new_value;
            return Ok(());
        }

        if !data.is_object() {
            *data = serde_json::Value::Object(serde_json::Map::new());
        }

        let mut current = data;

        for (i, &key) in keys.iter().enumerate() {
            let is_last = i == keys.len() - 1;

            if is_last {
                if let Some(obj) = current.as_object_mut() {
                    obj.insert(key.to_string(), new_value);
                    return Ok(());
                } else {
                    return Err(AviP2pError::InvalidPath("Parent is not an object".to_string()));
                }
            } else {
                if current.get(key).is_none() {
                    if let Some(obj) = current.as_object_mut() {
                        obj.insert(key.to_string(), serde_json::Value::Object(serde_json::Map::new()));
                    }
                }

                current = current.get_mut(key)
                    .ok_or_else(|| AviP2pError::InvalidPath(format!("Failed to navigate to key: {}", key)))?;
            }
        }

        Err(AviP2pError::InvalidPath("Unexpected end of path".to_string()))
    }
    async fn update_capabilities(&self, local_peer_id: String) {
        match self.update_ctx(&format!("avi.device.caps.{}", local_peer_id), self.get_caps_as_json()).await {
            Ok(..) => {},
            Err(e) => println!("Failed to update device capabilities: {}", e)
        };
    }
    fn get_caps_as_json(&self) -> serde_json::Value {
        serde_json::to_value(self.config.capabilities.clone()).unwrap()
    }

    ///To call use
    ///
    ///tokio::spawn(async move {
    ///   device.start_event_loop().await;
    ///});
    pub async fn start_event_loop(&self) {
        let events = {
            let mut lock = self.events.lock().await;
            lock.take()
        };

        if let Some(mut rx) = events {
            while let Some(event) = rx.recv().await {
                self.handle_event(event).await;
            }
        }
    }

    pub async fn get_peers(&self) -> Result<Vec<PeerId>, AviP2pError> {
        self.handler.connected_peers().await
    }
    pub async fn publish(&self, topic: &str, data: Vec<u8>) -> Result<(), AviP2pError> {
        self.handler.publish(topic, data).await
    }

    pub async fn subscribe(&self, topic: &str, handler: impl Fn(PeerId, String, Vec<u8>) + Send + Sync + 'static) -> Result<(), AviP2pError> {
        {
            let mut handlers = self.subscription_handlers.write().await;
            handlers.insert(topic.to_string(), Arc::new(handler));
        }
        self.handler.subscribe(topic).await
    }

    pub async fn get_ctx(&self, path: &str) -> Result<serde_json::Value, AviP2pError> {
        match self.handler.get_context(None).await {
            Ok(data) => {
                if path.is_empty() {
                    return Ok(data);
                }
                let keys: Vec<&str> = path.split('.').collect();
                let mut current = &data;

                for key in keys {
                    current = current.get(key).ok_or_else(|| {
                        AviP2pError::Serialization(format!("Key '{}' not found in context", key))
                    })?;
                }
                Ok(current.clone())
            }
            Err(e) => Err(e)
        }
    }

    pub async fn update_ctx(&self, path: &str, value: serde_json::Value) -> Result<(), AviP2pError> {
        let mut current_ctx = self.get_ctx("").await?;

        Self::set_nested_value(&mut current_ctx, path, value)?;

        self.handler.update_context(current_ctx).await
    }

    pub async fn execute_query(&self, query: DeviceQuery) -> Result<Vec<String>, AviP2pError> {
        match serde_json::from_value(self.get_ctx("avi.device.caps").await?) {
            Ok(v) => Ok(query.execute(&v)),
            Err(_) => Err(AviP2pError::Serialization("Error".to_string()))
        }
    }

    pub async fn get_core_id(&self) -> Result<String, AviP2pError> {
        match self.get_ctx("avi.core").await {
            Ok(v) => Ok(serde_json::from_value(v).expect("Failed to deserialize core peer id")),
            Err(e) => Err(e)
        }
    }

    pub async fn register_stream_handler<F>(&self, reason: String, factory: F)
    where
        F: StreamHandlerFactory + 'static,
    {
        self.stream_dispatcher.register_handler(reason, factory).await;
    }

    pub async fn request_stream(&self, peer_id: PeerId, reason: String) -> Result<StreamId, String> {
        self.stream_dispatcher.request_stream(peer_id, reason).await
    }

    pub async fn close_stream(&self, stream_id: StreamId) -> Result<(), String> {
        self.stream_dispatcher.close_stream(stream_id).await
    }

    pub async fn send_stream_data(&self, stream_id: StreamId, data: Vec<u8>) -> Result<(), String> {
        self.handler.send_stream_data(stream_id, data).await
            .map_err(|e| e.to_string())
    }
}
