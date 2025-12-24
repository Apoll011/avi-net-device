use serde_json::Value;
use tokio::sync::oneshot;
use crate::events::{PeerId, StreamId};
use crate::error::AviP2pError;

#[derive(Debug)]
pub enum Command {
    // PubSub
    Subscribe {
        topic: String,
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },
    Unsubscribe {
        topic: String,
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },
    Publish {
        topic: String,
        data: Vec<u8>,
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },

    RequestStream {
        peer_id: PeerId,
        respond_to: oneshot::Sender<Result<StreamId, AviP2pError>>
    },
    AcceptStream {
        stream_id: StreamId,
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },
    SendStreamData {
        stream_id: StreamId,
        data: Vec<u8>,
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },
    CloseStream {
        stream_id: StreamId,
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },

    // Queries
    GetConnectedPeers {
        respond_to: oneshot::Sender<Result<Vec<PeerId>, AviP2pError>>
    },
    DiscoverPeers {
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },

    // Lifecycle
    Shutdown {
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },

    UpdateSelfContext {
        patch: Value, // JSON partial update
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },

    GetPeerContext {
        peer_id: Option<PeerId>, // None = Get All / Self
        respond_to: oneshot::Sender<Result<Value, AviP2pError>>
    },
}