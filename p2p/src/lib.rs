//! # AVI P2P
//!
//! A production-ready, opinionated abstraction over libp2p for the AVI Core platform.
//!
//! ## Features
//! - Pub/Sub messaging
//! - Streaming (logical streams over request-response)
//! - Kademlia Mesh Networking
//! - Zero libp2p type exposure

pub mod config;
pub mod events;
mod behaviour;
mod command;
mod node;
mod protocols;
mod runtime;
mod error;
pub mod bridge;

pub use config::AviP2pConfig;
pub use error::{AviP2pError, StreamCloseReason};
pub use events::{AviEvent, PeerId};
pub use protocols::stream::{StreamId, StreamStatus, StreamState, generate_stream_id, StreamDirection};
pub use node::{AviP2p, AviP2pHandle};
pub use protocols::context::{AviContext, VectorClock};
pub use protocols::context::{set_nested_value};
pub use bridge::{BridgeConfig, EmbeddedBridge};