//! Runtime service assembled from focused session, knowledge, persistence,
//! and execution-host responsibilities.

#[path = "service/runtime_service.rs"]
mod host;

pub use host::{RuntimeService, RuntimeServiceOptions};
