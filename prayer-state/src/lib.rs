//! Canonical passive state models and pure state queries.

pub mod graph;
mod model;
mod state;

pub use graph::RouteTable;
pub use model::*;
pub use state::*;

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn shared_state_has_no_runtime_capability_dependencies() {
        let manifest = include_str!("../Cargo.toml");
        for forbidden in ["prayer-runtime", "prayer-sdk", "prayer-scheduler", "tokio"] {
            assert!(
                !manifest.contains(forbidden),
                "forbidden dependency: {forbidden}"
            );
        }

        let sources = [include_str!("state.rs"), include_str!("model.rs")].concat();
        for forbidden in [
            "std::time::Instant",
            "RwLock",
            "Mutex",
            "tokio::",
            "prayer_runtime",
            "prayer_scheduler",
        ] {
            assert!(
                !sources.contains(forbidden),
                "runtime capability leaked into shared state: {forbidden}"
            );
        }
    }
}
