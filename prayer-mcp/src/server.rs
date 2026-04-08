//! MCP server wiring: ServerHandler impl + transport startup.

use std::sync::Arc;

use rmcp::{
    model::{
        ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams,
        ReadResourceRequestParams, ReadResourceResult, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool_handler,
    transport::{
        io::stdio,
        streamable_http_server::{
            session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
        },
    },
    ErrorData as McpError, RoleServer, ServerHandler,
};

use crate::{
    client::PrayerApiClient, resources::ResourceHandler, session_store::SessionStore,
    tools::PrayerMcpServer,
};

// ── ServerHandler impl ────────────────────────────────────────────────────────
// `#[tool_handler]` appends call_tool / list_tools / get_tool that delegate to
// self.tool_router.  We add resource methods and get_info here.

#[tool_handler]
impl ServerHandler for PrayerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_instructions(
            "Prayer MCP server. Use the tools to manage sessions, author scripts, \
             and inspect EffectiveState through the virtual filesystem.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(self.resources.list_resources().await)
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: ResourceHandler::resource_templates(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        self.resources.read_resource(request).await
    }
}

// ── transport config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Stdio,
    /// HTTP/SSE transport via streamable HTTP endpoint.
    Sse,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub prayer_url: String,
    pub transport: Transport,
    pub bind: String,
    pub mcp_path: String,
    pub request_timeout_ms: u64,
    pub session_store: Option<String>,
    /// When set, each freshly-built VFS is mirrored to this directory and
    /// `{parent}/vfs-index.log` is regenerated.
    pub vfs_dump_dir: Option<std::path::PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            prayer_url: "http://127.0.0.1:7777".into(),
            transport: Transport::Stdio,
            bind: "127.0.0.1:5000".into(),
            mcp_path: "/mcp".into(),
            request_timeout_ms: 0,
            session_store: None,
            vfs_dump_dir: None,
        }
    }
}

pub async fn run(config: ServerConfig) -> anyhow::Result<()> {
    let client = Arc::new(PrayerApiClient::new(
        config.prayer_url,
        config.request_timeout_ms,
    ));
    let session_store = if let Some(path) = config.session_store.as_deref() {
        let store = Arc::new(SessionStore::load(path).await?);
        store.restore_startup(&client).await?;
        Some(store)
    } else {
        None
    };

    let vfs_dump_dir = config.vfs_dump_dir.clone();
    if let Some(dir) = &vfs_dump_dir {
        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::warn!(path = %dir.display(), error = %e, "could not create vfs dump dir");
        }
    }

    match config.transport {
        Transport::Stdio => {
            tracing::info!("transport: stdio");
            let server = PrayerMcpServer::new(client, session_store.clone(), vfs_dump_dir);
            let service = rmcp::serve_server(server, stdio()).await?;
            service.waiting().await?;
            Ok(())
        }
        Transport::Sse => {
            let mcp_path = normalize_mcp_path(&config.mcp_path);
            tracing::info!("sse: building StreamableHttpService");
            let service: StreamableHttpService<PrayerMcpServer, LocalSessionManager> =
                StreamableHttpService::new(
                    {
                        let client = Arc::clone(&client);
                        let session_store = session_store.clone();
                        move || {
                            Ok(PrayerMcpServer::new(
                                Arc::clone(&client),
                                session_store.clone(),
                                vfs_dump_dir.clone(),
                            ))
                        }
                    },
                    Default::default(),
                    StreamableHttpServerConfig::default(),
                );
            tracing::info!("sse: StreamableHttpService built, binding listener");
            let router = axum::Router::new().nest_service(&mcp_path, service);
            let listener = tokio::net::TcpListener::bind(&config.bind).await?;
            tracing::info!("sse: listener bound");
            let local = listener.local_addr()?;
            tracing::info!(
                bind = %local,
                mcp_path = %mcp_path,
                "transport: streamable-http"
            );
            axum::serve(listener, router).await?;
            Ok(())
        }
    }
}

fn normalize_mcp_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "/mcp".to_string();
    }
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_mcp_path;

    #[test]
    fn normalize_mcp_path_defaults_when_empty() {
        assert_eq!(normalize_mcp_path(""), "/mcp");
        assert_eq!(normalize_mcp_path("   "), "/mcp");
    }

    #[test]
    fn normalize_mcp_path_adds_leading_slash() {
        assert_eq!(normalize_mcp_path("mcp"), "/mcp");
        assert_eq!(normalize_mcp_path("api/mcp"), "/api/mcp");
    }

    #[test]
    fn normalize_mcp_path_preserves_leading_slash() {
        assert_eq!(normalize_mcp_path("/mcp"), "/mcp");
        assert_eq!(normalize_mcp_path("/custom"), "/custom");
    }
}
