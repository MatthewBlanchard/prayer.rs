//! Agent, social, and consolidated chat projections.

use super::super::*;
use prayer_state::{ChatMessageData, FactionMemberData, FactionRoleData, FactionSnapshotData};

impl RuntimeService {
    pub async fn refresh_faction_knowledge(&self, id: Uuid) -> Result<(), SdkError> {
        let key = id.to_string();
        {
            let metadata = self.knowledge_metadata.read();
            if metadata
                .faction_fetched_at_by_session
                .get(&key)
                .is_some_and(|at| at.elapsed() < IDLE_SESSION_REFRESH_INTERVAL)
            {
                return Ok(());
            }
        }
        let response = self
            .spacemolt_account(id)
            .await?
            .commands()
            .spacemolt_faction()
            .info(Some(SpacemoltFactionInfoParams {
                limit: None,
                offset: None,
                id: None,
            }))
            .await
            .map_err(SdkError::from)?
            .into_typed()
            .map_err(SdkError::from)?;
        let faction = FactionSnapshotData {
            id: response.id,
            name: response.name,
            tag: response.tag,
            leader_id: response.leader_id,
            leader_username: response.leader_username,
            member_count: response.member_count,
            treasury: response.treasury,
            is_member: response.is_member,
            description: if response.description.is_empty() {
                response.charter
            } else {
                response.description
            },
            primary_color: response.primary_color,
            secondary_color: response.secondary_color,
            members: response
                .members
                .into_iter()
                .map(|member| FactionMemberData {
                    player_id: member.player_id,
                    username: member.username,
                    role: member.role,
                    online: member.is_online,
                })
                .collect(),
            roles: response
                .roles
                .into_iter()
                .map(|role| FactionRoleData {
                    name: role.name,
                    priority: role.priority,
                })
                .collect(),
        };
        let snapshot = {
            self.knowledge_metadata
                .write()
                .faction_fetched_at_by_session
                .insert(key.clone(), Instant::now());
            let mut knowledge = self.knowledge_state.write();
            if knowledge.faction_by_session.get(&key) == Some(&faction) {
                None
            } else {
                knowledge.faction_by_session.insert(key, faction);
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
                Some(knowledge.clone())
            }
        };
        if let Some(snapshot) = snapshot {
            self.knowledge_persistence
                .publish(snapshot, "after faction refresh");
        }
        Ok(())
    }

    /// Refresh the bounded public chat cache for one managed session.
    pub async fn refresh_chat_knowledge(&self, id: Uuid) -> Result<(), SdkError> {
        let key = id.to_string();
        {
            let metadata = self.knowledge_metadata.read();
            if metadata
                .chat_fetched_at_by_session
                .get(&key)
                .is_some_and(|at| at.elapsed() < IDLE_SESSION_REFRESH_INTERVAL)
            {
                return Ok(());
            }
        }

        let account = self.spacemolt_account(id).await?;
        let mut messages_by_id = HashMap::new();
        let mut any_succeeded = false;
        for channel in ["system", "local", "faction", "emergency"] {
            let params = SpacemoltSocialGetChatHistoryParams {
                after: None,
                before: None,
                limit: Some(50),
                target_id: None,
                target: channel.to_string(),
            };
            match account
                .commands()
                .spacemolt_social()
                .get_chat_history(params)
                .await
                .map_err(SdkError::from)
                .and_then(|result| result.into_typed().map_err(SdkError::from))
            {
                Ok(response) => {
                    any_succeeded = true;
                    for message in response.messages {
                        let message = ChatMessageData {
                            id: message.id,
                            channel: message.channel,
                            sender_id: message.sender_id,
                            sender: message.sender,
                            content: message.content,
                            timestamp_utc: message.timestamp_utc,
                            system_id: message.system_id,
                            poi_id: message.poi_id,
                            faction_id: message.faction_id,
                            target_id: message.target_id,
                            target_name: message.target_name,
                            empire_official: message.empire_official,
                        };
                        messages_by_id.entry(message.id.clone()).or_insert(message);
                    }
                }
                Err(err) => debug!(%id, channel, error = %err, "chat history refresh failed"),
            }
        }
        if !any_succeeded {
            return Ok(());
        }

        let mut messages: Vec<_> = messages_by_id.into_values().collect();
        messages.sort_by(|a, b| {
            b.timestamp_utc
                .cmp(&a.timestamp_utc)
                .then_with(|| b.id.cmp(&a.id))
        });
        let snapshot = {
            self.knowledge_metadata
                .write()
                .chat_fetched_at_by_session
                .insert(key.clone(), Instant::now());
            let mut knowledge = self.knowledge_state.write();
            if knowledge.chat_messages_by_session.get(&key) == Some(&messages) {
                None
            } else {
                knowledge.chat_messages_by_session.insert(key, messages);
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
                Some(knowledge.clone())
            }
        };
        if let Some(snapshot) = snapshot {
            self.knowledge_persistence
                .publish(snapshot, "after chat history refresh");
        }
        Ok(())
    }

    pub fn agent_sightings_snapshot(&self) -> Vec<AgentSightingData> {
        let knowledge = self.knowledge_state.read();
        let social = SocialLens::new(&knowledge);
        let mut sightings: Vec<AgentSightingData> = social.sightings().cloned().collect();
        sightings.sort_by(|a, b| {
            b.last_seen_unix
                .cmp(&a.last_seen_unix)
                .then_with(|| a.contact.username.cmp(&b.contact.username))
        });
        sightings
    }

    pub async fn refresh_managed_players_knowledge(&self) {
        let entries: Vec<Arc<Mutex<SessionHandle>>> =
            self.sessions.read().values().cloned().collect();
        let mut values = Vec::new();
        for session in entries {
            let session = session.lock().await;
            for value in [
                session.actor.observed.player.id.as_deref(),
                session.actor.observed.player.username.as_deref(),
                session.actor.observed.player.id.as_deref(),
                session.actor.observed.player.username.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                let trimmed = value.trim();
                if !trimmed.is_empty()
                    && !values
                        .iter()
                        .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
                {
                    values.push(trimmed.to_string());
                }
            }
        }
        values.sort_by_key(|value| value.to_ascii_lowercase());

        let snapshot = {
            let mut knowledge = self.knowledge_state.write();
            if knowledge.managed_players != values {
                knowledge.managed_players = values;
                knowledge.knowledge_version = knowledge.knowledge_version.saturating_add(1);
                Some(knowledge.clone())
            } else {
                None
            }
        };
        if let Some(snapshot) = snapshot {
            self.knowledge_persistence
                .publish(snapshot, "after managed player refresh");
        }
    }

    /// Social tab projection: remembered player sightings plus transient NPCs
    /// visible in current session location payloads.
    pub async fn social_snapshot_response(&self) -> SocialResponse {
        let mut social = map_social_bots(self.agent_sightings_snapshot());
        let entries: Vec<Arc<Mutex<SessionHandle>>> =
            self.sessions.read().values().cloned().collect();
        let now = Utc::now();
        let mut synthetic_keys = HashSet::new();
        for session in entries {
            let session = session.lock().await;
            if !session.has_state {
                continue;
            }
            for bot in
                synthetic_social_bots_from_state(&session.actor.observed, &session.label, now)
            {
                let key = format!(
                    "{}:{}:{}",
                    bot.actor_kind,
                    bot.player_id.as_deref().unwrap_or(&bot.username),
                    bot.last_seen_system
                );
                if synthetic_keys.insert(key) {
                    social.bots.push(bot);
                }
            }
        }
        social.bots.sort_by(|a, b| {
            b.last_seen_utc
                .cmp(&a.last_seen_utc)
                .then_with(|| a.username.cmp(&b.username))
        });
        social
    }

    pub async fn social_snapshot_response_for_session(
        &self,
        id: Uuid,
        chat_channels: &[String],
        chat_limit: i64,
    ) -> Result<SocialResponse, SdkError> {
        let mut social = self.social_snapshot_response().await;
        social.chat = Some(
            self.consolidated_chat_response(id, chat_channels, chat_limit, None)
                .await?,
        );
        Ok(social)
    }

    pub async fn consolidated_chat_response(
        &self,
        id: Uuid,
        channels: &[String],
        limit: i64,
        target_id: Option<String>,
    ) -> Result<GameChatResponse, SdkError> {
        let (default_system, default_poi) = {
            let session = self.get_session(id).await?;
            let session = session.lock().await;
            let current = &session.actor.observed;
            (
                current
                    .location
                    .system_id
                    .clone()
                    .filter(|value| !value.is_empty()),
                current
                    .location
                    .poi_id
                    .clone()
                    .filter(|value| !value.is_empty()),
            )
        };

        let mut messages_by_id: HashMap<String, GameChatMessageDto> = HashMap::new();
        let mut summaries = Vec::new();
        for channel in channels {
            let params = SpacemoltSocialGetChatHistoryParams {
                after: None,
                before: None,
                limit: Some(limit),
                target_id: (channel == "private")
                    .then(|| {
                        target_id
                            .as_ref()
                            .filter(|value| !value.trim().is_empty())
                            .cloned()
                    })
                    .flatten(),
                target: channel.clone(),
            };
            let response = match self.spacemolt_account(id).await {
                Ok(account) => account
                    .commands()
                    .spacemolt_social()
                    .get_chat_history(params)
                    .await
                    .map_err(SdkError::from)
                    .and_then(|result| result.into_typed().map_err(SdkError::from)),
                Err(err) => Err(err),
            };
            match response {
                Ok(response) => {
                    let message_count = response.messages.len();
                    for raw in response.messages {
                        let message = normalize_chat_message(
                            raw,
                            default_system.as_deref(),
                            default_poi.as_deref(),
                        );
                        messages_by_id.entry(message.id.clone()).or_insert(message);
                    }
                    summaries.push(GameChatChannelSummaryDto {
                        channel: response.channel,
                        message_count,
                        total_count: usize::try_from(response.total_count).ok(),
                        has_more: response.has_more,
                        error: None,
                    });
                }
                Err(err) => {
                    summaries.push(GameChatChannelSummaryDto {
                        channel: channel.clone(),
                        message_count: 0,
                        total_count: None,
                        has_more: false,
                        error: Some(err.to_string()),
                    });
                }
            }
        }

        let mut messages: Vec<_> = messages_by_id.into_values().collect();
        messages.sort_by(|a, b| {
            b.timestamp_utc
                .cmp(&a.timestamp_utc)
                .then_with(|| b.id.cmp(&a.id))
        });
        let has_more = summaries.iter().any(|summary| summary.has_more);
        let total_count = messages.len();
        Ok(GameChatResponse {
            messages,
            channels: summaries,
            total_count,
            has_more,
        })
    }
}
