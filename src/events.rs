use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PeerId(pub(crate) String);

impl PeerId {
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Implement conversion from libp2p::PeerId to our PeerId wrapper
impl From<libp2p::PeerId> for PeerId {
    fn from(id: libp2p::PeerId) -> Self {
        PeerId(id.to_base58())
    }
}

// Implement conversion from our PeerId wrapper to libp2p::PeerId
impl TryFrom<PeerId> for libp2p::PeerId {
    type Error = crate::error::AviP2pError;

    fn try_from(id: PeerId) -> Result<Self, Self::Error> {
        libp2p::PeerId::from_str(&id.0)
            .map_err(|e| crate::error::AviP2pError::NetworkError(e.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct StreamId(pub u64);

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

use crate::error::StreamCloseReason;

#[derive(Debug, Clone)]
pub enum AviEvent {
    // Network lifecycle
    Started {
        local_peer_id: PeerId,
        listen_addresses: Vec<String>,
    },

    // Peer events
    PeerDiscovered {
        peer_id: PeerId,
    },

    PeerConnected {
        peer_id: PeerId,
        address: String,
    },

    PeerDisconnected {
        peer_id: PeerId,
    },

    // PubSub
    Message {
        from: PeerId,
        topic: String,
        data: Vec<u8>,
    },

    //  streaming
    StreamRequested {
        from: PeerId,
        stream_id: StreamId,
    },

    StreamAccepted {
        peer_id: PeerId,
        stream_id: StreamId,
    },

    StreamData {
        from: PeerId,
        stream_id: StreamId,
        data: Vec<u8>,
    },

    StreamClosed {
        peer_id: PeerId,
        stream_id: StreamId,
        reason: StreamCloseReason,
    },

    ContextUpdated {
        peer_id: PeerId,
        context: serde_json::Value,
    },
}