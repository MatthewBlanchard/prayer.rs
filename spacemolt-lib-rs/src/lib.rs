//! Rust client foundation for SpaceMolt.
//!
//! This crate is a Rust port-in-progress of `@spacemolt/lib`: protocol
//! envelopes, generated command metadata, state caches, and eventually the
//! WebSocket v2 account/client implementation Prayer can use underneath its
//! higher-level planners.

#[allow(
    clippy::derivable_impls,
    clippy::result_large_err,
    clippy::type_complexity
)]
pub mod account;
pub mod actions;
pub mod auth;
#[allow(clippy::result_large_err, clippy::type_complexity)]
pub mod client;
#[allow(clippy::result_large_err)]
pub mod commands;
pub mod data;
pub mod errors;
pub mod events;
pub mod notifications;
pub mod protocol;
pub mod schema;
pub mod state;
pub mod transport;

pub use account::*;
pub use actions::*;
pub use client::*;
pub use data::*;
pub use errors::*;
pub use protocol::*;
