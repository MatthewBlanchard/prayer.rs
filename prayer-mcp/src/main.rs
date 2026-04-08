//! prayer-mcp — MCP server that proxies LLM requests through prayer-api.
//!
//! # Usage
//!
//! ```text
//! prayer-mcp [--prayer-url URL] [--transport stdio|sse] [--bind ADDR] [--mcp-path PATH]
//! ```

mod client;
mod dsl_ref;
mod resources;
mod server;
mod session_handles;
mod session_store;
mod tools;
mod vfs;

use clap::{Parser, ValueEnum};
use server::{ServerConfig, Transport};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TransportArg {
    Stdio,
    Sse,
}

/// Prayer MCP server.
#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// prayer-api base URL.
    #[arg(long, env = "PRAYER_URL", default_value = "http://127.0.0.1:7777")]
    prayer_url: String,

    /// Transport mode.
    #[arg(long, env = "PRAYER_MCP_TRANSPORT", default_value = "stdio")]
    transport: TransportArg,

    /// Bind address (SSE mode only).
    #[arg(long, env = "PRAYER_MCP_BIND", default_value = "127.0.0.1:5000")]
    bind: String,

    /// MCP HTTP path (SSE mode only).
    #[arg(long, env = "PRAYER_MCP_PATH", default_value = "/mcp")]
    mcp_path: String,

    /// HTTP request timeout in milliseconds.
    /// Set to 0 to disable timeout (default).
    #[arg(long, env = "PRAYER_MCP_REQUEST_TIMEOUT_MS", default_value_t = 0)]
    request_timeout_ms: u64,

    /// Optional JSON file path for saving/restoring MCP-created sessions across restarts.
    #[arg(long, env = "PRAYER_MCP_SESSION_STORE")]
    session_store: Option<String>,

    /// Disable local session/credential persistence and startup auto-restore.
    #[arg(long, default_value_t = false)]
    no_session_store: bool,

    /// Directory to mirror VFS files into on each state refresh.
    /// vfs-index.log is written to the parent directory.
    /// Defaults to logs/vfs relative to the current working directory.
    #[arg(long, env = "PRAYER_MCP_VFS_DUMP_DIR", default_value = "logs/vfs")]
    vfs_dump_dir: String,

    /// Disable VFS disk mirroring entirely.
    #[arg(long, default_value_t = false)]
    no_vfs_dump: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    let transport = match args.transport {
        TransportArg::Stdio => Transport::Stdio,
        TransportArg::Sse => Transport::Sse,
    };

    tracing::info!(
        prayer_url = %args.prayer_url,
        transport = ?transport,
        "prayer-mcp starting"
    );

    let session_store = if args.no_session_store {
        None
    } else {
        args.session_store.or_else(default_session_store_path)
    };

    if let Some(path) = &session_store {
        tracing::info!(
            session_store = %path,
            "session/credential persistence enabled"
        );
    } else {
        tracing::info!("session/credential persistence disabled");
    }

    let vfs_dump_dir = if args.no_vfs_dump {
        None
    } else {
        Some(std::path::PathBuf::from(&args.vfs_dump_dir))
    };

    if let Some(dir) = &vfs_dump_dir {
        tracing::info!(vfs_dump_dir = %dir.display(), "vfs disk mirroring enabled");
    }

    let config = ServerConfig {
        prayer_url: args.prayer_url,
        transport,
        bind: args.bind,
        mcp_path: args.mcp_path,
        request_timeout_ms: args.request_timeout_ms,
        session_store,
        vfs_dump_dir,
    };

    server::run(config).await
}

fn default_session_store_path() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    if home.trim().is_empty() {
        return None;
    }
    Some(format!("{home}/.config/prayer-rs/mcp-session-store.json"))
}
