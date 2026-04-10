//! MCP tool definitions and the `PrayerMcpServer` struct.

use std::sync::Arc;

use prayer_runtime::dsl::AstProgram;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    client::PrayerApiClient,
    resources::ResourceHandler,
    session_handles::{
        resolve_session_id, sanitize_session_entry, sanitize_sessions, strip_session_id_fields,
    },
    session_store::SessionStore,
};

// ── input schemas ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateSessionInput {
    /// SpaceMolt username.
    pub username: String,
    /// SpaceMolt password.
    pub password: String,
    /// Optional human-readable label.
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RegisterSessionInput {
    /// SpaceMolt username.
    pub username: String,
    /// SpaceMolt empire.
    pub empire: String,
    /// SpaceMolt registration code.
    pub registration_code: String,
    /// Optional human-readable label.
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunScriptInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
    /// PrayerLang script text.
    pub script: String,
    /// Optional maximum steps to execute.
    pub max_steps: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HaltSessionInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
    /// Optional halt reason.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveSessionInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PassthroughInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
    /// SpaceMolt command name.
    pub command: String,
    /// Optional command payload.
    pub payload: Option<Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FsReadInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
    /// Virtual path (e.g. `/missions/active.json`).
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FsQueryInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
    /// Pipeline string.
    pub pipeline: String,
    /// Maximum result rows (default 200, hard-cap 1000).
    pub max_results: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FsLsInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
    /// Directory path to list. Defaults to "/" (root).
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSkillsLibraryInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSkillsLibraryInput {
    /// Target session handle (playerName from list_sessions).
    pub session_handle: String,
    /// Skill/override library text.
    pub text: String,
}

// ── server struct ─────────────────────────────────────────────────────────────

/// The MCP server: holds the API client, resource handler, and tool router.
#[derive(Clone)]
pub struct PrayerMcpServer {
    pub(crate) client: Arc<PrayerApiClient>,
    pub(crate) resources: Arc<ResourceHandler>,
    pub(crate) session_store: Option<Arc<SessionStore>>,
    pub(crate) tool_router: ToolRouter<Self>,
}

impl PrayerMcpServer {
    pub fn new(
        client: Arc<PrayerApiClient>,
        session_store: Option<Arc<SessionStore>>,
        vfs_dump_dir: Option<std::path::PathBuf>,
    ) -> Self {
        let resources = Arc::new(ResourceHandler::new(Arc::clone(&client), vfs_dump_dir));
        Self {
            client,
            resources,
            session_store,
            tool_router: Self::tool_router(),
        }
    }

    async fn resolve_session_id_or_error(&self, session_handle: &str) -> Result<String, String> {
        let sessions = self
            .client
            .list_sessions()
            .await
            .map_err(|e| format!("error: {e}"))?;
        resolve_session_id(&sessions, session_handle)
    }
}

// ── tool implementations ──────────────────────────────────────────────────────

#[tool_router]
impl PrayerMcpServer {
    #[tool(description = "List all active Prayer sessions")]
    async fn list_sessions(&self) -> Result<String, String> {
        match self.client.list_sessions().await {
            Ok(v) => Ok(serde_json::to_string_pretty(&sanitize_sessions(&v)).unwrap_or_default()),
            Err(e) => Err(format!("error: {e}")),
        }
    }

    #[tool(description = "Create a new Prayer session and authenticate with SpaceMolt")]
    async fn create_session(
        &self,
        Parameters(CreateSessionInput {
            username,
            password,
            label,
        }): Parameters<CreateSessionInput>,
    ) -> Result<String, String> {
        let effective_label = label
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| username.clone());

        match self
            .client
            .create_session(&username, &password, Some(effective_label.as_str()))
            .await
        {
            Ok(v) => {
                if let (Some(store), Some(session_id)) = (
                    self.session_store.as_ref(),
                    v.get("id").and_then(serde_json::Value::as_str),
                ) {
                    store
                        .remember_created(
                            session_id,
                            &username,
                            &password,
                            Some(effective_label.as_str()),
                        )
                        .await;
                }
                Ok(serde_json::to_string_pretty(&sanitize_session_entry(&v)).unwrap_or_default())
            }
            Err(e) => Err(format!("error: {e}")),
        }
    }

    #[tool(description = "Register a new SpaceMolt account and create a Prayer session")]
    async fn register_session(
        &self,
        Parameters(RegisterSessionInput {
            username,
            empire,
            registration_code,
            label,
        }): Parameters<RegisterSessionInput>,
    ) -> Result<String, String> {
        let effective_label = label
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| username.clone());

        match self
            .client
            .register_session(
                &username,
                &empire,
                &registration_code,
                Some(effective_label.as_str()),
            )
            .await
        {
            Ok(v) => {
                let session_id = v
                    .get("sessionId")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .or_else(|| {
                        v.get("session_id")
                            .and_then(serde_json::Value::as_str)
                            .map(ToString::to_string)
                    });
                let generated_password = v
                    .get("password")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string);

                if let (Some(store), Some(session_id), Some(generated_password)) = (
                    self.session_store.as_ref(),
                    session_id.as_deref(),
                    generated_password.as_deref(),
                ) {
                    store
                        .remember_created(
                            session_id,
                            &username,
                            generated_password,
                            Some(effective_label.as_str()),
                        )
                        .await;
                }

                let payload = serde_json::json!({
                    "playerName": effective_label,
                    "username": username,
                    "empire": empire,
                    "password": generated_password,
                });
                Ok(serde_json::to_string_pretty(&payload).unwrap_or_default())
            }
            Err(e) => Err(format!("error: {e}")),
        }
    }

    #[tool(
        description = "Load a PrayerLang script into a session (selected by session_handle) and execute it. \
        Returns session, normalized script, status (completed/halted/step limit reached/error), and steps executed."
    )]
    async fn run_script(
        &self,
        Parameters(RunScriptInput {
            session_handle,
            script,
            max_steps,
        }): Parameters<RunScriptInput>,
    ) -> Result<String, String> {
        let session_id = self.resolve_session_id_or_error(&session_handle).await?;
        self.client
            .load_script(&session_id, &script)
            .await
            .map_err(|e| {
                let msg = format!("error: {e}");
                tracing::error!(tool = "run_script", stage = "load_script", error = %msg);
                msg
            })?;

        if let Some(store) = &self.session_store {
            store.remember_script(&session_id, &script).await;
        }

        let pretty_script = AstProgram::parse(&script)
            .map(|program| program.normalize())
            .unwrap_or_else(|_| script.clone());

        let exec_result = self
            .client
            .execute_script(&session_id, max_steps)
            .await
            .map_err(|e| {
                let msg = format!("error: {e}");
                tracing::error!(tool = "run_script", stage = "execute_script", error = %msg);
                msg
            })?;

        let status = if exec_result["error"].is_string() {
            format!("error: {}", exec_result["error"].as_str().unwrap_or("unknown"))
        } else if exec_result["completed"].as_bool().unwrap_or(false) {
            "completed".to_string()
        } else if exec_result["halted"].as_bool().unwrap_or(false) {
            "halted".to_string()
        } else {
            "step limit reached".to_string()
        };
        let steps = exec_result["steps_executed"].as_u64().unwrap_or(0);
        let payload = serde_json::json!({
            "session": session_handle,
            "script": pretty_script.trim(),
            "status": status,
            "steps": steps,
        });

        Ok(serde_json::to_string_pretty(&payload).unwrap_or_default())
    }

    #[tool(description = "Halt execution in a session selected by session_handle")]
    async fn halt_session(
        &self,
        Parameters(HaltSessionInput {
            session_handle,
            reason,
        }): Parameters<HaltSessionInput>,
    ) -> Result<String, String> {
        let session_id = self.resolve_session_id_or_error(&session_handle).await?;
        match self
            .client
            .halt_session(&session_id, reason.as_deref())
            .await
        {
            Ok(v) => {
                if let Some(store) = &self.session_store {
                    store.remember_halted(&session_id, true).await;
                }
                let cleaned = strip_session_id_fields(&v);
                let command = cleaned
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "halt".to_string());
                let message = cleaned
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "halted".to_string());
                let payload = serde_json::json!({
                    "session_handle": session_handle,
                    "command": command,
                    "message": message,
                    "halted": true,
                });
                Ok(serde_json::to_string_pretty(&payload).unwrap_or_default())
            }
            Err(e) => Err(format!("error: {e}")),
        }
    }

    #[tool(description = "Remove a Prayer session selected by session_handle")]
    async fn remove_session(
        &self,
        Parameters(RemoveSessionInput { session_handle }): Parameters<RemoveSessionInput>,
    ) -> Result<String, String> {
        let session_id = self.resolve_session_id_or_error(&session_handle).await?;
        match self.client.delete_session(&session_id).await {
            Ok(()) => {
                if let Some(store) = &self.session_store {
                    store.forget_session(&session_id).await;
                }
                let payload = serde_json::json!({
                    "session_handle": session_handle,
                    "removed": true,
                });
                Ok(serde_json::to_string_pretty(&payload).unwrap_or_default())
            }
            Err(e) => Err(format!("error: {e}")),
        }
    }

    #[tool(
        description = "Send a SpaceMolt passthrough command through the transport for session_handle"
    )]
    async fn passthrough(
        &self,
        Parameters(PassthroughInput {
            session_handle,
            command,
            payload,
        }): Parameters<PassthroughInput>,
    ) -> Result<String, String> {
        let session_id = self.resolve_session_id_or_error(&session_handle).await?;
        match self
            .client
            .passthrough(&session_id, &command, payload)
            .await
        {
            Ok(v) => {
                let cleaned = strip_session_id_fields(&v);
                let payload = serde_json::json!({
                    "session_handle": session_handle,
                    "command": command,
                    "response": cleaned,
                });
                Ok(serde_json::to_string_pretty(&payload).unwrap_or_default())
            }
            Err(e) => Err(format!("error: {e}")),
        }
    }

    #[tool(
        description = "Read a single virtual file from the projected EffectiveState filesystem for session_handle"
    )]
    async fn fs_read(
        &self,
        Parameters(FsReadInput {
            session_handle,
            path,
        }): Parameters<FsReadInput>,
    ) -> String {
        let session_id = match self.resolve_session_id_or_error(&session_handle).await {
            Ok(v) => v,
            Err(e) => return e,
        };
        self.resources.fs_read(&session_id, &path).await
    }

    #[tool(
        description = "Run a composable pipeline query over the virtual EffectiveState filesystem for session_handle. \
        Stages: find <glob> | grep <regex> | read | project <fields> | sort <field> [asc|desc] | unique <field> | limit <n>. \
        Example: find **/missions/*.json | grep turn in | limit 20"
    )]
    async fn fs_query(
        &self,
        Parameters(FsQueryInput {
            session_handle,
            pipeline,
            max_results,
        }): Parameters<FsQueryInput>,
    ) -> String {
        let session_id = match self.resolve_session_id_or_error(&session_handle).await {
            Ok(v) => v,
            Err(e) => return e,
        };
        self.resources
            .fs_query(&session_id, &pipeline, max_results)
            .await
    }

    #[tool(
        description = "List the contents of a directory in the virtual EffectiveState filesystem for session_handle. \
        Defaults to the root. Use fs_read to read a specific file."
    )]
    async fn fs_ls(
        &self,
        Parameters(FsLsInput {
            session_handle,
            path,
        }): Parameters<FsLsInput>,
    ) -> String {
        let session_id = match self.resolve_session_id_or_error(&session_handle).await {
            Ok(v) => v,
            Err(e) => return e,
        };
        self.resources
            .fs_ls(&session_id, path.as_deref().unwrap_or("/"))
            .await
    }

    #[tool(description = "Get the canonicalized skill/override library text for session_handle")]
    async fn get_skills_library(
        &self,
        Parameters(GetSkillsLibraryInput { session_handle }): Parameters<GetSkillsLibraryInput>,
    ) -> Result<String, String> {
        let session_id = self.resolve_session_id_or_error(&session_handle).await?;
        let v = self
            .client
            .get_skills(&session_id)
            .await
            .map_err(|e| format!("error: {e}"))?;
        let payload = serde_json::json!({
            "session_handle": session_handle,
            "library": v,
        });
        Ok(serde_json::to_string_pretty(&payload).unwrap_or_default())
    }

    #[tool(
        description = "Set the skill/override library text for session_handle and return canonicalized text"
    )]
    async fn set_skills_library(
        &self,
        Parameters(SetSkillsLibraryInput {
            session_handle,
            text,
        }): Parameters<SetSkillsLibraryInput>,
    ) -> Result<String, String> {
        let session_id = self.resolve_session_id_or_error(&session_handle).await?;
        let v = self
            .client
            .set_skills(&session_id, &text)
            .await
            .map_err(|e| format!("error: {e}"))?;
        let payload = serde_json::json!({
            "session_handle": session_handle,
            "library": v,
        });
        Ok(serde_json::to_string_pretty(&payload).unwrap_or_default())
    }
}
