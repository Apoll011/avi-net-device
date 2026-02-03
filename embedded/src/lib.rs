#![no_std]

use avi_p2p_protocol::{DownlinkMessage, PressType, SensorValue, UplinkMessage};
use core::future::Future;
use serde::Serialize;

// C API module
#[cfg(feature = "c-api")]
pub mod c_api;

pub trait UdpClient {
    type Error;
    fn send(&mut self, buf: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
    fn receive(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}

pub trait MessageHandler {
    fn on_message(&mut self, topic: &str, data: &[u8]);
}

pub struct AviEmbeddedConfig {
    pub device_id: u64,
}

pub struct AviEmbedded<'a, S: UdpClient, H: MessageHandler> {
    socket: S,
    config: AviEmbeddedConfig,
    scratch_buf: &'a mut [u8],
    is_connected: bool,
    handler: H,
}

impl<'a, S: UdpClient, H: MessageHandler> AviEmbedded<'a, S, H> {
    pub fn new(socket: S, config: AviEmbeddedConfig, buffer: &'a mut [u8], handler: H) -> Self {
        Self {
            socket,
            config,
            scratch_buf: buffer,
            is_connected: false,
            handler,
        }
    }

    pub async fn connect(&mut self) -> Result<(), S::Error> {
        // 1. Send Hello
        let hello = UplinkMessage::Hello {
            device_id: self.config.device_id,
        };
        self.send_packet(&hello).await?;

        let mut rx_buf = [0u8; 128];
        if let Ok(len) = self.socket.receive(&mut rx_buf).await {
            if let Ok(DownlinkMessage::Welcome) = postcard::from_bytes(&rx_buf[..len]) {
                self.is_connected = true;
                return Ok(());
            }
        }

        self.is_connected = false;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    // Pub/Sub Methods
    pub async fn subscribe(&mut self, topic: &str) -> Result<(), S::Error> {
        let msg = UplinkMessage::Subscribe { topic };
        self.send_packet(&msg).await
    }

    pub async fn unsubscribe(&mut self, topic: &str) -> Result<(), S::Error> {
        let msg = UplinkMessage::Unsubscribe { topic };
        self.send_packet(&msg).await
    }

    pub async fn publish(&mut self, topic: &str, data: &[u8]) -> Result<(), S::Error> {
        let msg = UplinkMessage::Publish { topic, data };
        self.send_packet(&msg).await
    }

    // Stream Methods
    pub async fn start_stream(
        &mut self,
        id: u8,
        target_peer: &str,
        reason: &str,
    ) -> Result<(), S::Error> {
        let msg = UplinkMessage::StreamStart {
            local_stream_id: id,
            target_peer_id: target_peer,
            reason,
        };
        self.send_packet(&msg).await
    }

    pub async fn send_audio(&mut self, id: u8, pcm_data: &[u8]) -> Result<(), S::Error> {
        let msg = UplinkMessage::StreamData {
            local_stream_id: id,
            data: pcm_data,
        };
        self.send_packet(&msg).await
    }

    pub async fn close_stream(&mut self, id: u8) -> Result<(), S::Error> {
        let msg = UplinkMessage::StreamClose {
            local_stream_id: id,
        };
        self.send_packet(&msg).await
    }

    // Event Methods
    pub async fn button_pressed(
        &mut self,
        button_id: u8,
        press_type: PressType,
    ) -> Result<(), S::Error> {
        let msg = UplinkMessage::ButtonPress {
            button_id,
            press_type,
        };
        self.send_packet(&msg).await
    }

    pub async fn update_sensor(&mut self, name: &str, data: SensorValue) -> Result<(), S::Error> {
        let msg = UplinkMessage::SensorUpdate {
            sensor_name: name,
            data,
        };
        self.send_packet(&msg).await
    }

    // Process incoming messages (call this in your main loop)
    pub async fn poll(&mut self) -> Result<(), S::Error> {
        let mut rx_buf = [0u8; 1024];

        // Non-blocking receive with timeout
        match self.socket.receive(&mut rx_buf).await {
            Ok(len) => {
                if let Ok(msg) = postcard::from_bytes::<DownlinkMessage>(&rx_buf[..len]) {
                    match msg {
                        DownlinkMessage::Message { topic, data } => {
                            self.handler.on_message(topic, data);
                        }
                        DownlinkMessage::SubscribeAck { topic: _ } => {
                            // Subscription confirmed
                        }
                        DownlinkMessage::UnsubscribeAck { topic: _ } => {
                            // Unsubscription confirmed
                        }
                        DownlinkMessage::Welcome => {
                            self.is_connected = true;
                        }
                        DownlinkMessage::Error { reason: _ } => {
                            // Handle error
                        }
                    }
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn send_packet<T: Serialize>(&mut self, msg: &T) -> Result<(), S::Error> {
        if let Ok(used_slice) = postcard::to_slice(msg, self.scratch_buf) {
            self.socket.send(used_slice).await?;
        }
        Ok(())
    }
}
