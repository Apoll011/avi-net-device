#![no_std]

use core::future::Future;
use avi_p2p_protocol::{UplinkMessage, DownlinkMessage, PressType, SensorValue};
use serde::Serialize;

pub trait UdpClient {
    type Error;
    fn send(&mut self, buf: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
    fn receive(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}

pub struct AviEmbeddedConfig {
    pub device_id: u64,
}

pub struct AviEmbedded<'a, S: UdpClient> {
    socket: S,
    config: AviEmbeddedConfig,
    scratch_buf: &'a mut [u8],
    is_connected: bool,
}

impl<'a, S: UdpClient> AviEmbedded<'a, S> {
    pub fn new(socket: S, config: AviEmbeddedConfig, buffer: &'a mut [u8]) -> Self {
        Self {
            socket,
            config,
            scratch_buf: buffer,
            is_connected: false,
        }
    }

    //TODO: Is connecting to nothing and returning true
    pub async fn connect(&mut self) -> Result<(), S::Error> {
        // 1. Send Hello
        let hello = UplinkMessage::Hello { device_id: self.config.device_id };
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

    pub async fn start_stream(&mut self, id: u8, target_peer: &str, reason: &str) -> Result<(), S::Error> {
        let msg = UplinkMessage::StreamStart {
            local_stream_id: id,
            target_peer_id: target_peer,
            reason
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
        let msg = UplinkMessage::StreamClose { local_stream_id: id };
        self.send_packet(&msg).await
    }

    pub async fn button_pressed(&mut self, button_id: u8, press_type: PressType) -> Result<(), S::Error> {
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

    async fn send_packet<T: Serialize>(&mut self, msg: &T) -> Result<(), S::Error> {
        if let Ok(used_slice) = postcard::to_slice(msg, self.scratch_buf) {
            self.socket.send(used_slice).await?;
        }
        Ok(())
    }
}