use std::net::SocketAddr;
use std::sync::Arc;

use prayer_api::{build_router_with_sdk, start_process_telemetry};
use prayer_sdk::RuntimeServiceOptions;
use spacemolt_lib_rs::auth::MemoryCredentialStore;
use spacemolt_lib_rs::{SpacemoltClient, SpacemoltClientOptions};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEFAULT_BIND: &str = "127.0.0.1:7777";

#[cfg(feature = "heap-profile")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main]
async fn main() {
    // Keep this guard alive for the entire process. Dropping it during graceful
    // shutdown writes dhat-heap.json in the current working directory.
    #[cfg(feature = "heap-profile")]
    let _heap_profiler = dhat::Profiler::new_heap();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    start_process_telemetry("prayer-api");

    let spacemolt_origin = spacemolt_origin();
    let mut spacemolt_options = SpacemoltClientOptions::from_origin(spacemolt_origin.clone());
    spacemolt_options.clerk_api_key = std::env::var("SPACEMOLT_CLERK_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let spacemolt_client = Arc::new(SpacemoltClient::new(
        spacemolt_options,
        MemoryCredentialStore::default(),
    ));
    let sdk_options = prayer_sdk::with_runtime_options(
        prayer_sdk::options_from_client(spacemolt_client, spacemolt_origin),
        runtime_service_options(),
    );
    let sdk = Arc::new(prayer_sdk::sdk_from_options(sdk_options));
    let app = build_router_with_sdk(Arc::clone(&sdk));
    prayer_sdk::start_background_workers(&sdk);

    let addr: SocketAddr = std::env::var("PRAYER_RS_BIND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            DEFAULT_BIND
                .parse()
                .expect("DEFAULT_BIND must be a valid socket address")
        });
    if !addr.ip().is_loopback()
        && std::env::var("PRAYER_API_TOKEN")
            .ok()
            .is_none_or(|token| token.trim().is_empty())
    {
        panic!("non-loopback PRAYER_RS_BIND requires PRAYER_API_TOKEN");
    }

    let listener = TcpListener::bind(addr).await.expect("bind listener");
    tracing::info!(%addr, "prayer-api listening");

    let restore_sdk = Arc::clone(&sdk);
    tokio::spawn(async move {
        if let Err(error) = prayer_sdk::restore(&restore_sdk).await {
            tracing::warn!(%error, "Prayer SDK restore failed");
        }
    });

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve app");
    if let Err(error) = sdk.shutdown().await {
        tracing::warn!(%error, "Prayer SDK shutdown failed");
    }
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install Ctrl-C handler");
        std::future::pending::<()>().await;
    }
    tracing::info!("shutdown signal received");
}

fn spacemolt_origin() -> String {
    std::env::var("PRAYER_SPACEMOLT_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://game.spacemolt.com".to_string())
}

fn runtime_service_options() -> RuntimeServiceOptions {
    let mut options = RuntimeServiceOptions::default();
    if let Some(path) = env_path("PRAYER_KNOWLEDGE_STATE_PATH") {
        options.knowledge_state_path = path;
    }
    if let Some(path) = env_path("PRAYER_SESSION_STATE_PATH") {
        options.session_state_path = path;
    }
    options.local_auth_bypass = env_bool("PRAYER_LOCAL_AUTH_BYPASS");
    options.memory_size_breakdown = env_bool("PRAYER_MEMORY_SIZE_BREAKDOWN");
    options.tax_estimate_ttl = env_u64("PRAYER_TAX_ESTIMATE_TTL_SECS")
        .filter(|value| *value > 0)
        .map(std::time::Duration::from_secs)
        .unwrap_or(options.tax_estimate_ttl);
    options.script_wait_override =
        env_u64("PRAYER_SCRIPT_WAIT_MS").map(std::time::Duration::from_millis);
    options
}

fn env_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
}

fn env_bool(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.trim().parse().ok()
}
