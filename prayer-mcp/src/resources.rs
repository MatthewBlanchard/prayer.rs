//! MCP resource handlers and VFS cache.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::Mutex;
use rmcp::model::{
    ListResourcesResult, RawResource, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents, ResourceTemplate,
};

use crate::{
    client::PrayerApiClient,
    dsl_ref::{dsl_reference_json, dsl_reference_text},
    vfs::{
        run_pipeline, VfsCache, MAX_QUERY_RETURN_BYTES, MAX_QUERY_ROWS_DEFAULT,
        MAX_QUERY_ROWS_HARD, MAX_READ_RETURN_BYTES,
    },
};

type SessionId = String;

struct VfsCacheStore {
    entries: HashMap<SessionId, VfsCache>,
}

impl VfsCacheStore {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn get_if_current(&self, session_id: &str, version: u64) -> Option<VfsCache> {
        self.entries
            .get(session_id)
            .filter(|c| c.state_version == version)
            .map(clone_vfs)
    }

    fn insert(&mut self, session_id: String, cache: VfsCache) {
        self.entries.insert(session_id, cache);
    }
}

/// Manages MCP resource listing/reading and the per-session VFS cache.
pub struct ResourceHandler {
    client: Arc<PrayerApiClient>,
    vfs_cache: Mutex<VfsCacheStore>,
    /// When set, each newly-built VFS is written to `{vfs_dump_dir}/{session_id}/…`
    /// and `{vfs_dump_dir}/../vfs-index.log` is regenerated.
    vfs_dump_dir: Option<PathBuf>,
}

impl ResourceHandler {
    pub fn new(client: Arc<PrayerApiClient>, vfs_dump_dir: Option<PathBuf>) -> Self {
        Self {
            client,
            vfs_cache: Mutex::new(VfsCacheStore::new()),
            vfs_dump_dir,
        }
    }

    // ── static definitions ────────────────────────────────────────────────────

    pub fn static_resources() -> Vec<Resource> {
        vec![Resource::new(
            RawResource::new("prayer://dsl/reference", "PrayerLang DSL reference")
                .with_description("Commands, predicates, and syntax reference")
                .with_mime_type("text/plain"),
            None,
        )]
    }

    pub fn resource_templates() -> Vec<ResourceTemplate> {
        vec![]
    }

    // ── MCP dispatch ──────────────────────────────────────────────────────────

    pub async fn list_resources(&self) -> ListResourcesResult {
        ListResourcesResult {
            resources: Self::static_resources(),
            next_cursor: None,
            meta: None,
        }
    }

    pub async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let uri = params.uri.as_str();

        if uri == "prayer://dsl/reference" {
            let body = format!(
                "{}\n\n---\n\n{}",
                dsl_reference_text(),
                serde_json::to_string_pretty(&dsl_reference_json()).unwrap_or_default()
            );
            return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                body, uri,
            )
            .with_mime_type("text/plain")]));
        }

        Err(rmcp::ErrorData::resource_not_found(
            format!("unknown resource uri: {uri}"),
            None,
        ))
    }

    // ── VFS cache ─────────────────────────────────────────────────────────────

    async fn get_or_build_vfs(&self, session_id: &str) -> Result<VfsCache, rmcp::ErrorData> {
        let sw = self
            .client
            .get_state(session_id, None, None)
            .await
            .map_err(client_to_mcp)?;
        let version = sw.version;

        if let Some(cached) = self.vfs_cache.lock().get_if_current(session_id, version) {
            return Ok(cached);
        }

        let new_cache = VfsCache::build(version, &sw.body);
        let result = clone_vfs(&new_cache);
        self.vfs_cache
            .lock()
            .insert(session_id.to_string(), new_cache);

        if let Some(dump_dir) = &self.vfs_dump_dir {
            let dump_dir = dump_dir.clone();
            let sid = session_id.to_string();
            let snapshot = clone_vfs(&result);
            tokio::spawn(async move {
                dump_vfs_to_disk(&snapshot, &dump_dir, &sid).await;
            });
        }

        Ok(result)
    }

    // ── tool helpers ──────────────────────────────────────────────────────────

    pub async fn fs_read(&self, session_id: &str, path: &str) -> String {
        let path = match normalize_path(path) {
            Ok(p) => p,
            Err(e) => return format!("error: {e}"),
        };
        match self.get_or_build_vfs(session_id).await {
            Err(e) => format!("error: {e}"),
            Ok(vfs) => {
                if let Some(children) = vfs.list_dir(&path) {
                    return serde_json::to_string_pretty(
                        &serde_json::json!({ "path": path, "children": children }),
                    )
                    .unwrap_or_default();
                }
                match vfs.read_file(&path) {
                    None => format!("error: path not found: {path}"),
                    Some((_mime, text)) => {
                        if text.len() > MAX_READ_RETURN_BYTES {
                            tracing::error!(
                                session_id,
                                path,
                                bytes = text.len(),
                                limit = MAX_READ_RETURN_BYTES,
                                operation = "fs_read",
                                "result_too_large"
                            );
                            serde_json::json!({
                                "error": "result_too_large",
                                "path": path,
                                "bytes": text.len(),
                                "limit": MAX_READ_RETURN_BYTES,
                            })
                            .to_string()
                        } else {
                            text.to_string()
                        }
                    }
                }
            }
        }
    }

    pub async fn fs_query(
        &self,
        session_id: &str,
        pipeline: &str,
        max_results: Option<usize>,
    ) -> String {
        let limit = max_results
            .unwrap_or(MAX_QUERY_ROWS_DEFAULT)
            .min(MAX_QUERY_ROWS_HARD);

        match self.get_or_build_vfs(session_id).await {
            Err(e) => format!("error: {e}"),
            Ok(vfs) => match run_pipeline(&vfs, pipeline, limit) {
                Err(e) => format!("error: {e}"),
                Ok((rows, truncated, result_count, state_version)) => {
                    let result = serde_json::json!({
                        "rows": rows,
                        "truncated": truncated,
                        "result_count": result_count,
                        "state_version": state_version,
                    });
                    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
                    if text.len() > MAX_QUERY_RETURN_BYTES {
                        tracing::error!(
                            session_id,
                            pipeline,
                            bytes = text.len(),
                            limit = MAX_QUERY_RETURN_BYTES,
                            operation = "fs_query",
                            "result_too_large"
                        );
                        serde_json::json!({
                            "error": "result_too_large",
                            "bytes": text.len(),
                            "limit": MAX_QUERY_RETURN_BYTES,
                            "suggestion": "Add `limit` or `project` stages to reduce result size."
                        })
                        .to_string()
                    } else {
                        text
                    }
                }
            },
        }
    }

    pub async fn fs_ls(&self, session_id: &str, path: &str) -> String {
        let path = match normalize_path(path) {
            Ok(p) => p,
            Err(e) => return format!("error: {e}"),
        };
        match self.get_or_build_vfs(session_id).await {
            Err(e) => format!("error: {e}"),
            Ok(vfs) => {
                if let Some(children) = vfs.list_dir(&path) {
                    serde_json::to_string_pretty(
                        &serde_json::json!({ "path": path, "children": children }),
                    )
                    .unwrap_or_default()
                } else if vfs.read_file(&path).is_some() {
                    serde_json::to_string_pretty(
                        &serde_json::json!({ "path": path, "kind": "file" }),
                    )
                    .unwrap_or_default()
                } else {
                    format!("error: path not found: {path}")
                }
            }
        }
    }
}

// ── VFS disk dump ─────────────────────────────────────────────────────────────

async fn dump_vfs_to_disk(vfs: &VfsCache, dump_dir: &Path, session_id: &str) {
    let session_dir = dump_dir.join(session_id);

    // Collect everything we need before moving into spawn_blocking.
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut all_paths: Vec<String> = vfs.all_file_paths().map(str::to_owned).collect();
    all_paths.sort_unstable();
    for path in &all_paths {
        let Some((_mime, text)) = vfs.read_file(path) else {
            continue;
        };
        let rel = path.trim_start_matches('/');
        files.push((session_dir.join(rel), text.to_owned()));
    }

    let tree = render_tree_lines(vfs, "/");
    let index_content = format!(
        "# session: {session_id}\n# state_version: {}\n\n{}\n",
        vfs.state_version,
        tree.join("\n")
    );
    let index_path = dump_dir
        .parent()
        .map(|p| p.join("vfs-index.log"))
        .unwrap_or_else(|| dump_dir.join("vfs-index.log"));
    let file_count = files.len();
    let dump_dir_display = dump_dir.to_owned();

    let _ = tokio::task::spawn_blocking(move || {
        for (file_path, text) in &files {
            if let Some(parent) = file_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(file_path, text);
        }
        let _ = std::fs::write(&index_path, &index_content);
    })
    .await;

    tracing::debug!(
        session_id,
        files = file_count,
        dump_dir = %dump_dir_display.display(),
        "vfs dumped to disk"
    );
}

// ── utilities ─────────────────────────────────────────────────────────────────

fn client_to_mcp(e: crate::client::ClientError) -> rmcp::ErrorData {
    match &e {
        crate::client::ClientError::Api {
            status,
            endpoint,
            request_id,
            body,
        } => {
            let data = Some(serde_json::json!({
                "status_code": status,
                "endpoint": endpoint,
                "request_id": request_id,
            }));
            match *status {
                400 => rmcp::ErrorData::invalid_params(body.clone(), data),
                404 => rmcp::ErrorData::resource_not_found(body.clone(), data),
                409 | 429 => rmcp::ErrorData::invalid_request(body.clone(), data),
                _ => rmcp::ErrorData::internal_error(e.to_string(), data),
            }
        }
        crate::client::ClientError::Http(_) => rmcp::ErrorData::internal_error(e.to_string(), None),
    }
}

fn normalize_path(path: &str) -> Result<String, rmcp::ErrorData> {
    if path.chars().any(|c| c.is_control()) {
        return Err(rmcp::ErrorData::invalid_params(
            "path contains control characters",
            None,
        ));
    }
    if path.contains("..") || path.contains("./") {
        return Err(rmcp::ErrorData::invalid_params(
            "path traversal or dot-segments are not allowed",
            None,
        ));
    }
    let mut out = path.to_lowercase();
    if !out.starts_with('/') {
        out.insert(0, '/');
    }
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    Ok(out)
}

fn clone_vfs(c: &VfsCache) -> VfsCache {
    VfsCache {
        state_version: c.state_version,
        nodes: c.nodes.clone(),
    }
}

fn render_tree_lines(vfs: &VfsCache, root: &str) -> Vec<String> {
    fn walk(vfs: &VfsCache, path: &str, prefix: &str, out: &mut Vec<String>) {
        let Some(children) = vfs.list_dir(path) else {
            return;
        };
        for (idx, child) in children.iter().enumerate() {
            let is_last = idx + 1 == children.len();
            let connector = if is_last { "└── " } else { "├── " };
            out.push(format!("{prefix}{connector}{child}"));
            let next_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}│   ")
            };
            let child_path = if path == "/" {
                format!("/{child}")
            } else {
                format!("{path}/{child}")
            };
            walk(vfs, &child_path, &next_prefix, out);
        }
    }

    let mut out = Vec::new();
    out.push("/".to_string());
    walk(vfs, root, "", &mut out);
    out
}
