use std::collections::HashMap;
use std::sync::Arc;
use futures::future::BoxFuture;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{RwLock, Mutex};
use avi_p2p::{set_nested_value, AviEvent, AviP2p, AviP2pConfig, AviP2pError, AviP2pHandle, BridgeConfig, EmbeddedBridge, PeerId, StreamId};
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

    pub can_gateway_embedded: bool,

    pub capabilities: DeviceCapabilities,
}

#[derive(Clone)]
pub struct AviDevice {
    config: Arc<AviDeviceConfig>,

    #[allow(dead_code)]
    node: Arc<Mutex<Option<AviP2p>>>,
    events: Arc<Mutex<Option<Receiver<AviEvent>>>>,
    handler: AviP2pHandle,

    peer_id: Arc<RwLock<Option<PeerId>>>,

    stream_dispatcher: Arc<StreamDispatcher>,

    subscription_handlers: Arc<RwLock<HashMap<String, Arc<dyn Fn(PeerId, String, Vec<u8>) -> BoxFuture<'static, ()> + Send + Sync>>>>,
    on_started: Arc<RwLock<Option<Arc<dyn Fn(AviDevice, String, Vec<String>) -> BoxFuture<'static, ()> + Send + Sync>>>>,
    on_peer_discovered: Arc<RwLock<Option<Arc<dyn Fn(AviDevice, String) -> BoxFuture<'static, ()> + Send + Sync>>>>,
    on_peer_connected: Arc<RwLock<Option<Arc<dyn Fn(AviDevice, String, String) -> BoxFuture<'static, ()> + Send + Sync>>>>,
    on_peer_disconnected: Arc<RwLock<Option<Arc<dyn Fn(AviDevice, String) -> BoxFuture<'static, ()> + Send + Sync>>>>,
}

impl AviDevice {
    pub async fn new(config: AviDeviceConfig) -> Result<Self, String> {
        match AviP2p::start(AviP2pConfig::new(&config.node_name)).await {
            Ok((node, events)) =>{
                if config.can_gateway_embedded {
                    match EmbeddedBridge::start(
                        node.handle(),
                        BridgeConfig { udp_port: 8888 }
                    ).await {
                        Ok(..) => {},
                        Err(e) => println!("Failed to start embedded bridge: {}", e)
                    }
                }

                Ok(Self {
                    config: Arc::new(config),
                    handler: node.handle(),
                    stream_dispatcher: Arc::new(StreamDispatcher::new(node.handle())),
                    node: Arc::new(Mutex::new(Some(node))),
                    events: Arc::new(Mutex::new(Some(events))),
                    peer_id: Arc::new(RwLock::new(None)),
                    subscription_handlers: Arc::new(RwLock::new(HashMap::new())),
                    on_started: Arc::new(RwLock::new(None)),
                    on_peer_discovered: Arc::new(RwLock::new(None)),
                    on_peer_connected: Arc::new(RwLock::new(None)),
                    on_peer_disconnected: Arc::new(RwLock::new(None)),
                })
            },
            Err(e) => Err(format!("Failed to start AVI P2P node: {}", e))
        }
    }

    async fn handle_event(&self, event: AviEvent) {
        match event {
            AviEvent::Started { local_peer_id, listen_addresses } => {
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

                let handler = self.on_started.read().await;
                if let Some(handler) = &*handler {
                    handler(self.clone(), local_peer_id.to_string(), listen_addresses).await;
                }
            },

            AviEvent::PeerDiscovered { peer_id } => {
                let handler = self.on_peer_discovered.read().await;
                if let Some(handler) = &*handler {
                    handler(self.clone(), peer_id.to_string()).await;
                }
            },
            AviEvent::PeerConnected { peer_id, address } => {
                let local_id = {
                    self.peer_id.read().await.clone()
                };
                if let Some(local_peer_id) = local_id {
                    self.update_capabilities(local_peer_id.to_string()).await;
                }

                let handler = self.on_peer_connected.read().await;
                if let Some(handler) = &*handler {
                    handler(self.clone(), peer_id.to_string(), address).await;
                }
            },
            AviEvent::PeerDisconnected { peer_id } => {
                let handler = self.on_peer_disconnected.read().await;
                if let Some(handler) = &*handler {
                    handler(self.clone(), peer_id.to_string()).await;
                }
            },

            AviEvent::Message { from, topic, data } => {
                let handlers = self.subscription_handlers.read().await;
                if let Some(handler) = handlers.get(&topic) {
                    handler(from, topic, data).await;
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
            handlers.insert(topic.to_string(), Arc::new(move |peer_id, topic, data| {
                handler(peer_id, topic, data);
                Box::pin(async {})
            }));
        }
        self.handler.subscribe(topic).await
    }

    pub async fn subscribe_async<F, Fut>(&self, topic: &str, handler: F) -> Result<(), AviP2pError>
    where
        F: Fn(PeerId, String, Vec<u8>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output=()> + Send + 'static,
    {
        {
            let mut handlers = self.subscription_handlers.write().await;
            handlers.insert(topic.to_string(), Arc::new(move |peer_id, topic, data| {
                Box::pin(handler(peer_id, topic, data))
            }));
        }
        self.handler.subscribe(topic).await
    }

    pub async fn update_ctx(&self, path: &str, value: serde_json::Value) -> Result<(), AviP2pError> {
        let mut current_ctx = self.get_ctx("").await?;

        set_nested_value(&mut current_ctx, path, value)?;

        self.handler.update_context(current_ctx).await
    }

    pub async fn delete_ctx(&self, path: &str) -> Result<(), AviP2pError> {
        self.handler.delete_ctx(path).await
    }

    pub async fn clear_ctx(&self) -> Result<(), AviP2pError> {
        self.handler.clear_ctx().await
    }

    pub async fn has_ctx(&self, path: &str) -> Result<bool, AviP2pError> {
        self.handler.has_ctx(path).await
    }

    pub async fn get_ctx(&self, path: &str) -> Result<serde_json::Value, AviP2pError> {
       self.handler.get_ctx(path).await
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

    pub async fn get_id(&self) -> PeerId {
        self.peer_id.read().await.clone().unwrap()
    }

    pub fn get_config(&self) -> Arc<AviDeviceConfig> {
        self.config.clone()
    }

    pub async fn on_started<F, Fut>(&self, handler: F)
    where
        F: Fn(AviDevice, String, Vec<String>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut lock = self.on_started.write().await;
        *lock = Some(Arc::new(move |device, peer_id, addresses| {
            Box::pin(handler(device, peer_id, addresses))
        }));
    }

    pub async fn on_peer_discovered<F, Fut>(&self, handler: F)
    where
        F: Fn(AviDevice, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut lock = self.on_peer_discovered.write().await;
        *lock = Some(Arc::new(move |device, peer_id| {
            Box::pin(handler(device, peer_id))
        }));
    }

    pub async fn on_peer_connected<F, Fut>(&self, handler: F)
    where
        F: Fn(AviDevice, String, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut lock = self.on_peer_connected.write().await;
        *lock = Some(Arc::new(move |device, peer_id, address| {
            Box::pin(handler(device, peer_id, address))
        }));
    }

    pub async fn on_peer_disconnected<F, Fut>(&self, handler: F)
    where
        F: Fn(AviDevice, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut lock = self.on_peer_disconnected.write().await;
        *lock = Some(Arc::new(move |device, peer_id| {
            Box::pin(handler(device, peer_id))
        }));
    }
}
