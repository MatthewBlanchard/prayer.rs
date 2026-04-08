use std::net::SocketAddr;
use std::sync::Arc;

use prayer_api::{build_router, RuntimeService};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEFAULT_BIND: &str = "127.0.0.1:7777";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let service = Arc::new(RuntimeService::new());
    let app = build_router(service);

    let addr: SocketAddr = std::env::var("PRAYER_RS_BIND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            DEFAULT_BIND
                .parse()
                .expect("DEFAULT_BIND must be a valid socket address")
        });

    let listener = TcpListener::bind(addr).await.expect("bind listener");
    tracing::info!(%addr, "prayer-api listening");

    axum::serve(listener, app).await.expect("serve app");
}
