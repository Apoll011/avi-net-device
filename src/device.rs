use std::collections::HashMap;
use std::fmt::format;
use tokio::sync::mpsc::Receiver;
use avi_p2p::{AviEvent, AviP2p, AviP2pConfig, AviP2pError, AviP2pHandle, PeerId};
use crate::capability::DeviceCapabilities;
use crate::DeviceQuery;

pub enum AviDeviceType {
    CORE = 0,
    NODE = 1
}

impl Default for AviDeviceType {
    fn default() -> Self {
        Self::NODE
    }
}

pub struct AviDeviceConfig {
    node_name: String,
    device_type: AviDeviceType,

    capabilities: DeviceCapabilities,
}

pub struct AviDevice {
    config: AviDeviceConfig,

    p2p_config: AviP2pConfig,
    node: AviP2p,
    events: Receiver<AviEvent>,
    handler: AviP2pHandle,

    peer_id: Option<PeerId>,

    subscription_handlers: HashMap<String, Box<dyn Fn(PeerId, String, Vec<u8>)>>,
}

impl AviDevice {
    pub async fn new(config: AviDeviceConfig) -> Result<Self, String> {
        let p2p_config = AviP2pConfig::new(&config.node_name);

        match AviP2p::start(p2p_config.clone()).await {
            Ok((node, events)) => Ok(Self {
                config,
                p2p_config,
                handler: node.handle(),
                node,
                events,
                peer_id: None,
                subscription_handlers: HashMap::new(),
            }),
            Err(e) => Err(format!("Failed to start AVI P2P node: {}", e))
        }
    }

    async fn handle_event(&mut self, event: AviEvent) {
        match event {
            AviEvent::Started { local_peer_id, .. } => {
                self.peer_id = Some(local_peer_id.clone());
                self.update_capabilities(local_peer_id.clone().to_string()).await;

                let is_core = if let AviDeviceType::CORE = self.config.device_type { true } else { false };

                if !(is_core && self.get_ctx("avi.core").await.is_err()) {
                    println!("AVI Core already initialized");
                    return;
                }

                match self.update_ctx("avi.core", serde_json::Value::String(local_peer_id.to_string())).await {
                    Ok(..) => {},
                    Err(e) => println!("Failed to update AVI Core peer id: {}", e)
                }
            },

            AviEvent::PeerDiscovered { peer_id } => {},
            AviEvent::PeerConnected { peer_id, .. } => {
                match self.peer_id.clone() {
                    Some(local_peer_id) => {
                        self.update_capabilities(local_peer_id.to_string()).await;
                    },
                    _ => {}
                }
            },
            AviEvent::PeerDisconnected { peer_id } => {},

            AviEvent::Message { from, topic, data } => {
                if let Some(handler) = self.subscription_handlers.get_mut(&topic) {
                    handler(from, topic, data);
                }
            },

            AviEvent::ContextUpdated {peer_id, context} => {},

            AviEvent::StreamRequested { from, stream_id } => {},
            AviEvent::StreamAccepted { peer_id, stream_id } => {},
            AviEvent::StreamData { from, stream_id, data } => {},
            AviEvent::StreamClosed { peer_id, stream_id, reason } => {},
        };
    }
    fn set_nested_value(data: &mut serde_json::Value, path: &str, new_value: serde_json::Value) -> Result<(), AviP2pError> {
        let keys: Vec<&str> = path.split('.').collect();

        if keys.is_empty() {
            return Err(AviP2pError::InvalidPath("Empty path".to_string()));
        }

        if !data.is_object() {
            *data = serde_json::Value::Object(serde_json::Map::new());
        }

        let mut current = data;

        for (i, &key) in keys.iter().enumerate() {
            let is_last = i == keys.len() - 1;

            if is_last {
                return if let Some(obj) = current.as_object_mut() {
                    obj.insert(key.to_string(), new_value);
                    Ok(())
                } else {
                    Err(AviP2pError::InvalidPath("Parent is not an object".to_string()))
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
    async fn update_capabilities(&mut self, local_peer_id: String) {
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
    pub async fn start_event_loop(&mut self) {
        while let Some(event) = self.events.recv().await {
            self.handle_event(event).await;
        }
    }

    pub async fn get_peers(&self) -> Result<Vec<PeerId>, AviP2pError> {
        self.handler.connected_peers().await
    }
    pub async fn publish(&self, topic: &str, data: Vec<u8>) -> Result<(), AviP2pError> {
        self.handler.publish(topic, data).await
    }

    pub async fn subscribe(&mut self, topic: &str, handler: impl Fn(PeerId, String, Vec<u8>) + 'static) -> Result<(), AviP2pError> {
        self.subscription_handlers.insert(topic.to_string(), Box::new(handler));
        self.handler.subscribe(topic).await
    }

    pub async fn get_ctx(&self, path: &str) -> Result<serde_json::Value, AviP2pError> {
        let keys: Vec<&str> = path.split('.').collect();

        match self.handler.get_context(None).await {
            Ok(data) => {
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
            Err(e) => Err(AviP2pError::Serialization("Error".to_string()))
        }
    }

    pub async fn get_core_id(&self) -> Result<String, AviP2pError> {
        match self.get_ctx("avi.core").await {
            Ok(v) => Ok(serde_json::from_value(v).expect("Failed to deserialize core peer id")),
            Err(e) => Err(e)
        }
    }
}
