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

    // Audio
    RequestAudioStream {
        peer_id: PeerId,
        respond_to: oneshot::Sender<Result<StreamId, AviP2pError>>
    },
    AcceptAudioStream {
        stream_id: StreamId,
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },
    SendAudio {
        stream_id: StreamId,
        data: Vec<u8>,
        respond_to: oneshot::Sender<Result<(), AviP2pError>>
    },
    CloseAudioStream {
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
}