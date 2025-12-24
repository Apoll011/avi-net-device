use libp2p::request_response::{Codec}; // Removed ProtocolName
use async_trait::async_trait;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::io;
use crate::events::StreamId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioStreamMessage {
    RequestStream { stream_id: u64 },
    AcceptStream { stream_id: u64 },
    RejectStream { stream_id: u64, reason: String },
    AudioData { stream_id: u64, data: Vec<u8> },
    CloseStream { stream_id: u64 },
}

impl AudioStreamMessage {
    pub fn stream_id(&self) -> StreamId {
        match self {
            Self::RequestStream { stream_id } => StreamId(*stream_id),
            Self::AcceptStream { stream_id } => StreamId(*stream_id),
            Self::RejectStream { stream_id, .. } => StreamId(*stream_id),
            Self::AudioData { stream_id, .. } => StreamId(*stream_id),
            Self::CloseStream { stream_id } => StreamId(*stream_id),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AviAudioProtocol;

// In libp2p 0.53+, protocols are defined by types implementing AsRef<str>
impl AsRef<str> for AviAudioProtocol {
    fn as_ref(&self) -> &str {
        "/avi/audio/1.0.0"
    }
}

#[derive(Clone)]
pub struct AviAudioCodec;

#[async_trait]
impl Codec for AviAudioCodec {
    type Protocol = AviAudioProtocol;
    type Request = AudioStreamMessage;
    type Response = ();

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        // Read 4-byte length prefix
        let mut len_bytes = [0u8; 4];
        io.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        // Sanity check length (e.g. 10MB max)
        if len > 10 * 1024 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Message too large"));
        }

        let mut buffer = vec![0u8; len];
        io.read_exact(&mut buffer).await?;

        bincode::deserialize(&buffer)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        _io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        Ok(())
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let encoded = bincode::serialize(&req)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let len = (encoded.len() as u32).to_be_bytes();
        io.write_all(&len).await?;
        io.write_all(&encoded).await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        _io: &mut T,
        _res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        Ok(())
    }
}