use std::fmt;

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

    // Audio streaming
    AudioStreamRequested {
        from: PeerId,
        stream_id: StreamId,
    },

    AudioStreamAccepted {
        peer_id: PeerId,
        stream_id: StreamId,
    },

    AudioData {
        from: PeerId,
        stream_id: StreamId,
        data: Vec<u8>,
    },

    AudioStreamClosed {
        peer_id: PeerId,
        stream_id: StreamId,
        reason: StreamCloseReason,
    },
}