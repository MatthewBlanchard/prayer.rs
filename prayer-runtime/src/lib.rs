//! Prayer runtime crate (DSL, engine, and pure operation planning).

pub mod action_resolution;
pub mod analysis;
pub mod economy;
pub mod engine;
pub mod execution;
pub mod knowledge;
pub mod navigation;
pub mod operation_failure;
pub mod orchestration;
pub mod read_context;
pub mod snapshot;
mod state;

pub use action_resolution::*;
pub use analysis::*;
pub use engine::*;
pub use execution::*;
pub use operation_failure::*;
pub(crate) use state::*;

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn runtime_dependency_direction_is_downward_only() {
        let manifest = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
        )
        .expect("runtime manifest");
        assert!(!manifest.contains("prayer-api"));
        assert!(!manifest.contains("prayer-sdk"));
        assert!(!manifest.contains("axum"));

        let mut pending = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(path) = pending.pop() {
            for entry in std::fs::read_dir(path).expect("read runtime source directory") {
                let path = entry.expect("read runtime source entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    let source = std::fs::read_to_string(path).expect("read runtime source");
                    for forbidden in [
                        ["std", "::env::"].concat(),
                        ["SPACEMOLT", "_CLERK_API_KEY"].concat(),
                        ["Status", "Code"].concat(),
                    ] {
                        assert!(!source.contains(&forbidden));
                    }
                }
            }
        }
    }

    #[test]
    fn prayer_crates_do_not_restore_the_mixed_state_model() {
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repository root");
        let mut pending = vec![
            repository.join("prayer-runtime/src"),
            repository.join("prayer-sdk/src"),
        ];
        let forbidden = [
            ["pub struct ", "GameState"].concat(),
            ["&", "GameState"].concat(),
            ["GameState", " {"].concat(),
            ["GameState", "::"].concat(),
            ["from_", "game_state"].concat(),
            ["to_", "game_state"].concat(),
        ];
        while let Some(path) = pending.pop() {
            for entry in std::fs::read_dir(path).expect("read orchestration sources") {
                let entry = entry.expect("read source entry");
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    if path.ends_with("prayer-runtime/src/lib.rs")
                        || path.ends_with("prayer-sdk/src/lib.rs")
                    {
                        continue;
                    }
                    let source = std::fs::read_to_string(&path).expect("read source");
                    for token in &forbidden {
                        assert!(
                            !source.contains(token),
                            "mixed state token {token:?} in {}",
                            path.display()
                        );
                    }
                }
            }
        }
    }
}
