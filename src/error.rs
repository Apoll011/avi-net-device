use thiserror::Error;
use crate::events::{PeerId, StreamId};

#[derive(Debug, Clone, Error)]
pub enum AviP2pError {
    #[error("Node has not been started")]
    NotStarted,

    #[error("Peer not found: {0:?}")]
    PeerNotFound(PeerId),

    #[error("Stream not found: {0:?}")]
    StreamNotFound(StreamId),

    #[error("Invalid topic: {0}")]
    InvalidTopic(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Already subscribed to topic: {0}")]
    AlreadySubscribed(String),

    #[error("Not subscribed to topic: {0}")]
    NotSubscribed(String),

    #[error("Internal channel closed")]
    ChannelClosed,

    #[error("IO Error: {0}")]
    Io(String),

    #[error("Serialization Error: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone)]
pub enum StreamCloseReason {
    LocalClose,
    RemoteClose,
    Error(String),
}