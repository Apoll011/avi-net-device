use avi_p2p::{AviP2pHandle, StreamId, PeerId, StreamCloseReason};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;

pub struct StreamContext {
    pub handle: AviP2pHandle,
    pub stream_id: StreamId,
    pub peer_id: PeerId,
}

impl StreamContext {
    pub async fn send(&self, data: Vec<u8>) -> Result<(), String> {
        self.handle
            .send_stream_data(self.stream_id, data)
            .await
            .map_err(|e| format!("Failed to send data: {}", e))
    }
}

#[async_trait]
pub trait StreamHandler: Send + Sync {
    async fn on_accepted(&mut self, ctx: &StreamContext);

    async fn on_rejected(&mut self, peer_id: PeerId, stream_id: StreamId, reason: String);

    async fn on_data(&mut self, ctx: &StreamContext, data: Vec<u8>);

    async fn on_closed(&mut self, peer_id: PeerId, stream_id: StreamId, reason: StreamCloseReason);
}

#[async_trait]
pub trait StreamHandlerFactory: Send + Sync {

    async fn create_handler(&self) -> Box<dyn StreamHandler>;
}

pub struct StreamDispatcher {
    handle: AviP2pHandle,
    factories: Arc<RwLock<HashMap<String, Arc<dyn StreamHandlerFactory>>>>,
    active_handlers: Arc<RwLock<HashMap<StreamId, (String, PeerId, Box<dyn StreamHandler>)>>>,
}

impl StreamDispatcher {
    pub fn new(handle: AviP2pHandle) -> Self {
        Self {
            handle,
            factories: Arc::new(RwLock::new(HashMap::new())),
            active_handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_handler<F>(&self, reason: String, factory: F)
    where
        F: StreamHandlerFactory + 'static,
    {
        let mut factories = self.factories.write().await;
        factories.insert(reason, Arc::new(factory));
    }

    pub async fn handle_stream_requested(
        &self,
        from: PeerId,
        stream_id: StreamId,
        reason: String,
    ) -> Result<(), String> {
        let factories = self.factories.read().await;

        if let Some(factory) = factories.get(&reason) {
            println!("✅ Accepting stream {} (reason: {})", stream_id, reason);

            self.handle
                .accept_stream(stream_id)
                .await
                .map_err(|e| format!("Failed to accept stream: {}", e))?;

            let mut handler = factory.create_handler().await;

            let ctx = StreamContext {
                handle: self.handle.clone(),
                stream_id,
                peer_id: from.clone(),
            };
            handler.on_accepted(&ctx).await;

            let mut active = self.active_handlers.write().await;
            active.insert(stream_id, (reason.clone(), from, handler));

            Ok(())
        } else {
            println!("❌ Refusing stream {} (unknown reason: {})", stream_id, reason);

            self.handle
                .refuse_stream(stream_id, "not_handled".to_string())
                .await
                .map_err(|e| format!("Failed to refuse stream: {}", e))?;

            Err(format!("No handler registered for reason: {}", reason))
        }
    }

    pub async fn handle_stream_accepted(
        &self,
        peer_id: PeerId,
        stream_id: StreamId,
    ) -> Result<(), String> {
        let mut active = self.active_handlers.write().await;

        if let Some((_, stored_peer, handler)) = active.get_mut(&stream_id) {
            *stored_peer = peer_id.clone();

            let ctx = StreamContext {
                handle: self.handle.clone(),
                stream_id,
                peer_id,
            };
            handler.on_accepted(&ctx).await;
            Ok(())
        } else {
            Err(format!("No handler found for stream {}", stream_id))
        }
    }

    pub async fn handle_stream_rejected(
        &self,
        peer_id: PeerId,
        stream_id: StreamId,
        reason: String,
    ) -> Result<(), String> {
        let mut active = self.active_handlers.write().await;

        if let Some((_, _, mut handler)) = active.remove(&stream_id) {
            handler.on_rejected(peer_id, stream_id, reason).await;
            Ok(())
        } else {
            Err(format!("No handler found for stream {}", stream_id))
        }
    }

    pub async fn handle_stream_data(
        &self,
        from: PeerId,
        stream_id: StreamId,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let mut active = self.active_handlers.write().await;

        if let Some((_, peer_id, handler)) = active.get_mut(&stream_id) {
            let ctx = StreamContext {
                handle: self.handle.clone(),
                stream_id,
                peer_id: peer_id.to_owned(),
            };
            handler.on_data(&ctx, data).await;
            Ok(())
        } else {
            Err(format!("No handler found for stream {}", stream_id))
        }
    }

    pub async fn handle_stream_closed(
        &self,
        peer_id: PeerId,
        stream_id: StreamId,
        reason: StreamCloseReason,
    ) -> Result<(), String> {
        let mut active = self.active_handlers.write().await;

        if let Some((_, stored_peer, mut handler)) = active.remove(&stream_id) {
            handler.on_closed(stored_peer, stream_id, reason).await;
            Ok(())
        } else {
            Err(format!("No handler found for stream {}", stream_id))
        }
    }

    pub async fn request_stream(
        &self,
        peer_id: PeerId,
        reason: String,
    ) -> Result<StreamId, String> {
        let factories = self.factories.read().await;

        if let Some(factory) = factories.get(&reason) {
            let stream_id = self.handle
                .request_stream(peer_id.clone(), reason.clone())
                .await
                .map_err(|e| format!("Failed to request stream: {}", e))?;

            let handler = factory.create_handler().await;
            let mut active = self.active_handlers.write().await;
            active.insert(stream_id, (reason, peer_id, handler));

            Ok(stream_id)
        } else {
            Err(format!("No handler registered for reason: {}", reason))
        }
    }

    pub async fn close_stream(&self, stream_id: StreamId) -> Result<(), String> {
        self.handle
            .close_stream(stream_id)
            .await
            .map_err(|e| format!("Failed to close stream: {}", e))
    }
}

/*
// ============================================================================
// EXEMPLO DE IMPLEMENTAÇÃO: Handler de Áudio (COM envio de dados)
// ============================================================================

use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

pub struct AudioHandler {
    packet_count: u64,
    tx_shutdown: Option<mpsc::Sender<()>>,
}

#[async_trait]
impl StreamHandler for AudioHandler {
    async fn on_accepted(&mut self, ctx: &StreamContext) {
        println!("🎵 Audio stream {} established with {}", ctx.stream_id, ctx.peer_id);

        // Inicia task para enviar dados simulados de microfone
        let (tx, mut rx) = mpsc::channel(1);
        self.tx_shutdown = Some(tx);

        let ctx_clone = StreamContext {
            handle: ctx.handle.clone(),
            stream_id: ctx.stream_id,
            peer_id: ctx.peer_id,
        };

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(50)); // 20Hz
            let mut packet_num = 0u64;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Simula captura de áudio
                        let mut audio_data = vec![0u8; 1024];
                        let count_bytes = packet_num.to_be_bytes();
                        audio_data[0..8].copy_from_slice(&count_bytes);

                        if let Err(e) = ctx_clone.send(audio_data).await {
                            eprintln!("🔴 Failed to send audio: {}", e);
                            break;
                        }

                        packet_num += 1;
                    }
                    _ = rx.recv() => {
                        println!("🎙️ Microphone stopped for stream {}", ctx_clone.stream_id);
                        break;
                    }
                }
            }
        });
    }

    async fn on_rejected(&mut self, peer_id: PeerId, stream_id: StreamId, reason: String) {
        println!("❌ Audio stream {} rejected by {}. Reason: {}", stream_id, peer_id, reason);
    }

    async fn on_data(&mut self, ctx: &StreamContext, data: Vec<u8>) {
        // Extrai o contador de pacotes
        if data.len() >= 8 {
            let mut count_bytes = [0u8; 8];
            count_bytes.copy_from_slice(&data[0..8]);
            let seq = u64::from_be_bytes(count_bytes);

            print!("\r🔊 Audio from {} [Stream {}] | Pkt #{} | Size: {}b   ",
                   ctx.peer_id, ctx.stream_id, seq, data.len());
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }

        self.packet_count += 1;
    }

    async fn on_closed(&mut self, peer_id: PeerId, stream_id: StreamId, reason: StreamCloseReason) {
        // Para o envio de dados
        if let Some(tx) = self.tx_shutdown.take() {
            let _ = tx.send(()).await;
        }

        println!("\n🔇 Audio stream {} with {} closed. Received packets: {}. Reason: {:?}",
                 stream_id, peer_id, self.packet_count, reason);
    }
}

pub struct AudioHandlerFactory;

#[async_trait]
impl StreamHandlerFactory for AudioHandlerFactory {
    async fn create_handler(&self) -> Box<dyn StreamHandler> {
        Box::new(AudioHandler {
            packet_count: 0,
            tx_shutdown: None,
        })
    }
}

// ============================================================================
// EXEMPLO: Handler Read-Only (sem enviar dados)
// ============================================================================

pub struct LogHandler {
    events: Vec<String>,
}

#[async_trait]
impl StreamHandler for LogHandler {
    async fn on_accepted(&mut self, ctx: &StreamContext) {
        let msg = format!("Stream {} accepted with {}", ctx.stream_id, ctx.peer_id);
        println!("📝 {}", msg);
        self.events.push(msg);
        // Note: não envia dados, só observa
    }

    async fn on_rejected(&mut self, peer_id: PeerId, stream_id: StreamId, reason: String) {
        let msg = format!("Stream {} rejected by {}. Reason: {}", stream_id, peer_id, reason);
        println!("📝 {}", msg);
        self.events.push(msg);
    }

    async fn on_data(&mut self, ctx: &StreamContext, data: Vec<u8>) {
        let msg = format!("Received {} bytes from {}", data.len(), ctx.peer_id);
        println!("📝 {}", msg);
        self.events.push(msg);
    }

    async fn on_closed(&mut self, peer_id: PeerId, stream_id: StreamId, reason: StreamCloseReason) {
        println!("📝 Stream {} closed. Total events logged: {}", stream_id, self.events.len());
    }
}

pub struct LogHandlerFactory;

#[async_trait]
impl StreamHandlerFactory for LogHandlerFactory {
    async fn create_handler(&self) -> Box<dyn StreamHandler> {
        Box::new(LogHandler { events: Vec::new() })
    }
}
*/