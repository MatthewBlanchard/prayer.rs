//! Social chat command planning and validation.

use super::*;

impl CommandPlanner {
    pub(super) fn start_say(&mut self) -> Result<RuntimeOperation, OperationFailure> {
        let content = self
            .command
            .args
            .first()
            .map(ActionArg::as_text)
            .unwrap_or_default();
        let channel = self
            .command
            .args
            .get(1)
            .map(ActionArg::as_text)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let target = self.command.args.get(2).map(ActionArg::as_text);

        if content.trim().is_empty() {
            return Err(OperationFailure::InvalidIntent(
                "say requires a non-empty message".to_string(),
            ));
        }
        if content.chars().count() > 500 {
            return Err(OperationFailure::InvalidIntent(
                "say message must be 500 characters or less".to_string(),
            ));
        }
        if channel == "emergency" {
            return Err(OperationFailure::InvalidIntent(
                "say cannot send to emergency; emergency chat is read-only".to_string(),
            ));
        }
        if !matches!(channel.as_str(), "system" | "local" | "faction" | "private") {
            return Err(OperationFailure::InvalidIntent(format!(
                "say channel must be system, local, faction, or private; got '{channel}'"
            )));
        }
        if channel == "private" && target.as_deref().unwrap_or("").trim().is_empty() {
            return Err(OperationFailure::InvalidIntent(
                "say to private requires a target player".to_string(),
            ));
        }

        let mut payload = serde_json::json!({
            "target": channel,
            "content": content,
        });
        if let Some(target) = target.filter(|target| !target.trim().is_empty()) {
            payload["target_id"] = Value::String(target);
        }

        self.phase = Phase::AwaitFinalCall;
        Ok(RuntimeOperation::SpaceMoltAction {
            action: "spacemolt_social/chat".to_string(),
            payload: Some(payload),
        })
    }
}
