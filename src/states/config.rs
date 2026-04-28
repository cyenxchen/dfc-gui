//! Configuration State
//!
//! Manages the state of Redis configuration items and their loading status.

use crate::connection::{ConfigItem, ConfigLoadState, DetailItem, TopicAgentItem, TopicDetail};
use gpui::{Action, Context};
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::{BTreeMap, btree_map::Entry};
use std::sync::Arc;

const DEFAULT_SESSION_ID: &str = "__default__";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, JsonSchema, Action)]
pub enum AgentQueryMode {
    All,
    Prefix,
    Exact,
}

impl Default for AgentQueryMode {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentSearchSession {
    pub query: String,
    pub query_mode: AgentQueryMode,
}

#[derive(Default)]
struct ServerConfigSession {
    /// List of configuration items
    configs: Vec<ConfigItem>,
    /// TopicAgentId items merged from all configs
    topic_agents_merged: Vec<TopicAgentItem>,
    /// Current loading state
    load_state: ConfigLoadState,
    /// Currently selected config group ID
    selected_config_id: Option<i32>,
    /// Currently selected topic index within the config
    selected_topic_index: Option<i32>,
    /// Currently selected TopicAgentId
    selected_agent_id: Option<String>,
    /// TopicAgentId search UI state scoped to this server session.
    agent_search: AgentSearchSession,
    /// Whether the current topic selection should auto-drive topic consumption.
    topic_sync_enabled: bool,
    /// In-flight reconnect request id for this server session, if any.
    pending_request_id: Option<u64>,
    /// Stable load state to restore if an in-flight reconnect becomes stale.
    resume_load_state: Option<ConfigLoadState>,
}

/// Configuration state for managing Redis config items
pub struct ConfigState {
    /// Cached config session for each connected server.
    sessions: BTreeMap<String, ServerConfigSession>,
    /// IDs of all connected servers (supports multiple)
    connected_server_ids: Vec<String>,
    /// Currently active config session's server ID
    active_server_id: Option<String>,
}

impl ConfigState {
    const PREFERRED_AGENT_TOPIC_PREFIX: &str =
        "non-persistent://goldwind/iothub/prop_data-BZ-FAST-realdev-Guarantee";

    /// Create a new empty config state
    pub fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            connected_server_ids: Vec::new(),
            active_server_id: None,
        }
    }

    fn current_session(&self) -> Option<&ServerConfigSession> {
        self.active_server_id
            .as_deref()
            .and_then(|server_id| self.sessions.get(server_id))
            .or_else(|| self.sessions.get(DEFAULT_SESSION_ID))
    }

    fn current_session_mut(&mut self) -> &mut ServerConfigSession {
        let session_id = self
            .active_server_id
            .clone()
            .unwrap_or_else(|| DEFAULT_SESSION_ID.to_string());
        self.sessions.entry(session_id).or_default()
    }

    fn session_for_server_mut(&mut self, server_id: &str) -> &mut ServerConfigSession {
        self.sessions.entry(server_id.to_string()).or_default()
    }

    fn fallback_restored_load_state(session: &ServerConfigSession) -> ConfigLoadState {
        session.resume_load_state.clone().unwrap_or_else(|| {
            if session.configs.is_empty() {
                ConfigLoadState::Idle
            } else {
                ConfigLoadState::Loaded
            }
        })
    }

    fn mark_session_loading(session: &mut ServerConfigSession, request_id: Option<u64>) {
        if !matches!(session.load_state, ConfigLoadState::Loading) {
            session.resume_load_state = Some(session.load_state.clone());
        } else if session.resume_load_state.is_none() {
            session.resume_load_state = Some(Self::fallback_restored_load_state(session));
        }

        session.pending_request_id = request_id;
        session.load_state = ConfigLoadState::Loading;
    }

    fn finalize_session_request(session: &mut ServerConfigSession) {
        session.pending_request_id = None;
        session.resume_load_state = None;
    }

    fn restore_stale_loading_for_session(
        session: &mut ServerConfigSession,
        request_id: u64,
    ) -> bool {
        if session.pending_request_id != Some(request_id) {
            return false;
        }

        session.load_state = Self::fallback_restored_load_state(session);
        Self::finalize_session_request(session);
        true
    }

    fn apply_agent_search_session(&mut self, session: AgentSearchSession) -> bool {
        let current = self.current_session_mut();
        if current.agent_search == session {
            return false;
        }

        current.agent_search = session;
        true
    }

    // ==================== Getters ====================

    /// Get all configuration items
    pub fn configs(&self) -> &[ConfigItem] {
        self.current_session()
            .map_or(&[], |session| &session.configs)
    }

    /// Get the current loading state
    pub fn load_state(&self) -> ConfigLoadState {
        self.current_session()
            .map(|session| session.load_state.clone())
            .unwrap_or_default()
    }

    /// Check if currently loading
    pub fn is_loading(&self) -> bool {
        self.load_state().is_loading()
    }

    /// Get the selected config group ID
    pub fn selected_config_id(&self) -> Option<i32> {
        self.current_session()
            .and_then(|session| session.selected_config_id)
    }

    /// Get the selected topic index
    pub fn selected_topic_index(&self) -> Option<i32> {
        self.current_session()
            .and_then(|session| session.selected_topic_index)
    }

    /// Get the topic index currently allowed to drive topic streaming.
    pub fn synced_selected_topic_index(&self) -> Option<i32> {
        self.current_session().and_then(|session| {
            session
                .topic_sync_enabled
                .then_some(session.selected_topic_index)
                .flatten()
        })
    }

    /// Get all connected server IDs
    pub fn connected_server_ids(&self) -> &[String] {
        &self.connected_server_ids
    }

    /// Get the currently active server ID
    pub fn active_server_id(&self) -> Option<&str> {
        self.active_server_id.as_deref()
    }

    /// Whether the active session may auto-drive topic streaming.
    pub fn topic_sync_enabled(&self) -> bool {
        self.current_session()
            .is_some_and(|session| session.topic_sync_enabled)
    }

    /// Get the currently selected config item
    pub fn selected_config(&self) -> Option<&ConfigItem> {
        let session = self.current_session()?;
        session
            .selected_config_id
            .and_then(|id| session.configs.iter().find(|config| config.group_id == id))
    }

    /// Get the currently selected topic detail
    pub fn selected_topic(&self) -> Option<&DetailItem> {
        let session = self.current_session()?;
        let selected_config = session
            .selected_config_id
            .and_then(|id| session.configs.iter().find(|config| config.group_id == id))?;

        session.selected_topic_index.and_then(|idx| {
            selected_config
                .details
                .iter()
                .find(|detail| detail.index == idx)
        })
    }

    /// Get the selected agent ID
    pub fn selected_agent_id(&self) -> Option<&str> {
        self.current_session()
            .and_then(|session| session.selected_agent_id.as_deref())
    }

    /// Get the active server's TopicAgentId search UI state.
    pub fn agent_search_session(&self) -> AgentSearchSession {
        self.current_session()
            .map(|session| session.agent_search.clone())
            .unwrap_or_default()
    }

    /// Get all TopicAgentItems from the selected config
    pub fn topic_agents(&self) -> Vec<&TopicAgentItem> {
        self.current_session()
            .map(|session| session.topic_agents_merged.iter().collect())
            .unwrap_or_default()
    }

    /// Get the currently selected TopicAgentItem
    pub fn selected_agent(&self) -> Option<&TopicAgentItem> {
        let session = self.current_session()?;
        session.selected_agent_id.as_ref().and_then(|agent_id| {
            session
                .topic_agents_merged
                .iter()
                .find(|topic_agent| &topic_agent.agent_id == agent_id)
        })
    }

    fn is_preferred_agent_topic(topic: &TopicDetail) -> bool {
        topic.path.starts_with(Self::PREFERRED_AGENT_TOPIC_PREFIX)
    }

    fn is_prop_agent_topic(topic: &TopicDetail) -> bool {
        topic.path.contains("prop_data-BZ-") || topic.topic_type == "prop"
    }

    fn default_topic_index_for_agent(agent: &TopicAgentItem) -> Option<i32> {
        agent
            .topics
            .iter()
            .position(Self::is_preferred_agent_topic)
            .or_else(|| agent.topics.iter().position(Self::is_prop_agent_topic))
            .or_else(|| (!agent.topics.is_empty()).then_some(0))
            .map(|idx| idx as i32)
    }

    fn default_agent_selection(session: &ServerConfigSession) -> Option<(String, Option<i32>)> {
        session
            .topic_agents_merged
            .iter()
            .find(|agent| agent.topics.iter().any(Self::is_preferred_agent_topic))
            .map(|agent| {
                (
                    agent.agent_id.clone(),
                    Self::default_topic_index_for_agent(agent),
                )
            })
            .or_else(|| {
                session
                    .topic_agents_merged
                    .iter()
                    .find(|agent| agent.topics.iter().any(Self::is_prop_agent_topic))
                    .map(|agent| {
                        (
                            agent.agent_id.clone(),
                            Self::default_topic_index_for_agent(agent),
                        )
                    })
            })
            .or_else(|| {
                session
                    .topic_agents_merged
                    .iter()
                    .find(|agent| !agent.topics.is_empty())
                    .map(|agent| {
                        (
                            agent.agent_id.clone(),
                            Self::default_topic_index_for_agent(agent),
                        )
                    })
            })
            .or_else(|| {
                session
                    .topic_agents_merged
                    .first()
                    .map(|agent| (agent.agent_id.clone(), None))
            })
    }

    fn session_selected_topic_path(session: &ServerConfigSession) -> Option<&str> {
        let idx = session.selected_topic_index? as usize;
        session
            .selected_agent_id
            .as_ref()
            .and_then(|agent_id| {
                session
                    .topic_agents_merged
                    .iter()
                    .find(|agent| &agent.agent_id == agent_id)
            })
            .and_then(|agent| agent.topics.get(idx))
            .map(|topic| topic.path.as_str())
    }

    fn topic_index_by_path(agent: &TopicAgentItem, topic_path: &str) -> Option<i32> {
        agent
            .topics
            .iter()
            .position(|topic| topic.path == topic_path)
            .map(|idx| idx as i32)
    }

    fn selected_topic_path(&self) -> Option<&str> {
        self.current_session()
            .and_then(Self::session_selected_topic_path)
    }

    fn restore_previous_selection_for_session(
        session: &mut ServerConfigSession,
        previous_agent_id: Option<&str>,
        previous_topic_path: Option<&str>,
    ) -> bool {
        let Some(previous_agent_id) = previous_agent_id else {
            return false;
        };

        let Some(agent) = session
            .topic_agents_merged
            .iter()
            .find(|agent| agent.agent_id == previous_agent_id)
        else {
            return false;
        };

        if agent.topics.is_empty() {
            return false;
        }

        session.selected_agent_id = Some(agent.agent_id.clone());
        session.selected_topic_index = previous_topic_path
            .and_then(|topic_path| Self::topic_index_by_path(agent, topic_path))
            .or_else(|| Self::default_topic_index_for_agent(agent));

        true
    }

    fn rebuild_topic_agents_merged_for_session(session: &mut ServerConfigSession) {
        #[derive(Clone, Debug)]
        struct MergedTopicMeta {
            index: i32,
            visibility: bool,
            topic_type: String,
        }

        let mut merged: BTreeMap<String, BTreeMap<String, MergedTopicMeta>> = BTreeMap::new();

        for config in &session.configs {
            for agent in &config.topic_agents {
                let topics_by_path = merged.entry(agent.agent_id.clone()).or_default();

                for topic in &agent.topics {
                    match topics_by_path.entry(topic.path.clone()) {
                        Entry::Vacant(vacant) => {
                            vacant.insert(MergedTopicMeta {
                                index: topic.index,
                                visibility: topic.visibility,
                                topic_type: topic.topic_type.clone(),
                            });
                        }
                        Entry::Occupied(mut occupied) => {
                            let existing = occupied.get_mut();

                            if existing.topic_type == "unknown" && topic.topic_type != "unknown" {
                                existing.topic_type = topic.topic_type.clone();
                            }

                            if !existing.visibility && topic.visibility {
                                existing.visibility = true;
                            }

                            existing.index = existing.index.min(topic.index);
                        }
                    }
                }
            }
        }

        session.topic_agents_merged = merged
            .into_iter()
            .map(|(agent_id, topics_by_path)| {
                let mut topics: Vec<TopicDetail> = topics_by_path
                    .into_iter()
                    .map(|(path, meta)| TopicDetail {
                        index: meta.index,
                        path,
                        visibility: meta.visibility,
                        topic_type: meta.topic_type,
                    })
                    .collect();

                topics.sort_by(|a, b| a.index.cmp(&b.index).then_with(|| a.path.cmp(&b.path)));

                TopicAgentItem {
                    agent_id,
                    group_id: 0,
                    topics,
                }
            })
            .collect();
    }

    // ==================== Setters ====================

    fn apply_configs_for_session(session: &mut ServerConfigSession, configs: Vec<ConfigItem>) {
        let previous_agent_id = session.selected_agent_id.clone();
        let previous_topic_path = Self::session_selected_topic_path(session).map(ToOwned::to_owned);

        session.configs = configs;
        Self::rebuild_topic_agents_merged_for_session(session);
        session.selected_config_id = None;

        if Self::restore_previous_selection_for_session(
            session,
            previous_agent_id.as_deref(),
            previous_topic_path.as_deref(),
        ) {
            Self::finalize_session_request(session);
            session.load_state = ConfigLoadState::Loaded;
            return;
        }

        // Initialize selection to first agent/topic when available
        if let Some((agent_id, topic_index)) = Self::default_agent_selection(session) {
            session.selected_agent_id = Some(agent_id);
            session.selected_topic_index = topic_index;
        } else {
            session.selected_agent_id = None;
            session.selected_topic_index = None;
        }

        Self::finalize_session_request(session);
        session.load_state = ConfigLoadState::Loaded;
        session.topic_sync_enabled = true;
    }

    fn rebuild_topic_agents_merged(&mut self) {
        Self::rebuild_topic_agents_merged_for_session(self.current_session_mut());
    }

    fn apply_configs(&mut self, configs: Vec<ConfigItem>) {
        Self::apply_configs_for_session(self.current_session_mut(), configs);
    }

    /// Set configuration items
    pub fn set_configs(&mut self, configs: Vec<ConfigItem>, cx: &mut Context<Self>) {
        self.apply_configs(configs);
        cx.notify();
    }

    /// Set configuration items for a specific server session.
    pub fn set_configs_for_server(
        &mut self,
        server_id: &str,
        configs: Vec<ConfigItem>,
        cx: &mut Context<Self>,
    ) {
        self.active_server_id = Some(server_id.to_string());
        Self::apply_configs_for_session(self.session_for_server_mut(server_id), configs);
        cx.notify();
    }

    /// Set loading state
    pub fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.current_session_mut().load_state = ConfigLoadState::Loading;
        cx.notify();
    }

    /// Set loading state for a specific server session.
    pub fn set_loading_for_server(&mut self, server_id: &str, cx: &mut Context<Self>) {
        self.active_server_id = Some(server_id.to_string());
        Self::mark_session_loading(self.session_for_server_mut(server_id), None);
        cx.notify();
    }

    /// Set loading state for a specific server session and bind it to a reconnect request id.
    pub fn set_loading_for_server_request(
        &mut self,
        server_id: &str,
        request_id: u64,
        cx: &mut Context<Self>,
    ) {
        self.active_server_id = Some(server_id.to_string());
        Self::mark_session_loading(self.session_for_server_mut(server_id), Some(request_id));
        cx.notify();
    }

    /// Set error state
    pub fn set_error(&mut self, message: impl Into<Arc<str>>, cx: &mut Context<Self>) {
        self.current_session_mut().load_state = ConfigLoadState::Error(message.into());
        cx.notify();
    }

    /// Set error state for a specific server session.
    pub fn set_error_for_server(
        &mut self,
        server_id: &str,
        message: impl Into<Arc<str>>,
        cx: &mut Context<Self>,
    ) {
        self.active_server_id = Some(server_id.to_string());
        let session = self.session_for_server_mut(server_id);
        Self::finalize_session_request(session);
        session.load_state = ConfigLoadState::Error(message.into());
        cx.notify();
    }

    /// Set error state for a specific server session and finalize a tracked reconnect request.
    pub fn set_error_for_server_request(
        &mut self,
        server_id: &str,
        request_id: u64,
        message: impl Into<Arc<str>>,
        cx: &mut Context<Self>,
    ) {
        self.active_server_id = Some(server_id.to_string());
        let session = self.session_for_server_mut(server_id);
        if session.pending_request_id == Some(request_id) {
            Self::finalize_session_request(session);
        }
        session.load_state = ConfigLoadState::Error(message.into());
        cx.notify();
    }

    /// Restore a server session if a tracked reconnect became stale before completion.
    pub fn restore_stale_loading_for_server_request(
        &mut self,
        server_id: &str,
        request_id: u64,
        cx: &mut Context<Self>,
    ) {
        if !Self::restore_stale_loading_for_session(
            self.session_for_server_mut(server_id),
            request_id,
        ) {
            return;
        }
        cx.notify();
    }

    /// Select a configuration by group ID
    pub fn select_config(&mut self, group_id: Option<i32>, cx: &mut Context<Self>) {
        let session = self.current_session_mut();
        session.selected_config_id = group_id;
        // Reset topic selection when changing config
        if group_id.is_some() {
            session.selected_topic_index = Some(0);
        } else {
            session.selected_topic_index = None;
        }
        cx.notify();
    }

    /// Select a topic by index
    pub fn select_topic(&mut self, index: Option<i32>, cx: &mut Context<Self>) {
        let session = self.current_session_mut();
        session.topic_sync_enabled = index.is_some();
        session.selected_topic_index = index;
        cx.notify();
    }

    fn apply_agent_selection_for_session(
        session: &mut ServerConfigSession,
        agent_id: Option<String>,
    ) -> bool {
        if session.selected_agent_id == agent_id {
            return false;
        }

        session.selected_agent_id = agent_id;
        session.topic_sync_enabled = session.selected_agent_id.is_some();
        session.selected_topic_index = session
            .selected_agent_id
            .as_ref()
            .and_then(|selected_agent_id| {
                session
                    .topic_agents_merged
                    .iter()
                    .find(|agent| &agent.agent_id == selected_agent_id)
            })
            .and_then(Self::default_topic_index_for_agent);

        true
    }

    fn apply_agent_selection(&mut self, agent_id: Option<String>) -> bool {
        Self::apply_agent_selection_for_session(self.current_session_mut(), agent_id)
    }

    /// Select a TopicAgentId
    pub fn select_agent(&mut self, agent_id: Option<String>, cx: &mut Context<Self>) {
        if !self.apply_agent_selection(agent_id) {
            return;
        }

        if let Some(agent) = self.selected_agent() {
            tracing::info!(
                agent_id = %agent.agent_id,
                topics = agent.topics.len(),
                selected_topic_index = self.selected_topic_index(),
                "Selected TopicAgentId"
            );
        } else {
            tracing::info!(agent_id = ?self.selected_agent_id(), "Selected TopicAgentId (not found)");
        }
        cx.notify();
    }

    /// Save the active server's TopicAgentId search UI state.
    pub fn set_agent_search_session(
        &mut self,
        session: AgentSearchSession,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.apply_agent_search_session(session);
        if changed {
            cx.notify();
        }
        changed
    }

    /// Add a connected server (no duplicates)
    pub fn add_connected_server(&mut self, server_id: String, cx: &mut Context<Self>) {
        if !self.connected_server_ids.iter().any(|id| id == &server_id) {
            self.connected_server_ids.push(server_id.clone());
        }
        self.active_server_id = Some(server_id.clone());
        self.session_for_server_mut(&server_id);
        tracing::info!(
            server_id,
            cached_sessions = self.sessions.len(),
            "Connected config session ready"
        );
        cx.notify();
    }

    fn activate_server_session(&mut self, server_id: impl Into<String>) -> bool {
        let server_id = server_id.into();
        let was_active = self.active_server_id.as_deref() == Some(server_id.as_str());

        self.active_server_id = Some(server_id.clone());
        let has_cached_session = self.sessions.contains_key(server_id.as_str());
        let session = self.session_for_server_mut(&server_id);
        if !was_active {
            session.topic_sync_enabled = false;
        }
        tracing::info!(
            server_id,
            has_cached_session,
            topic_sync_enabled = session.topic_sync_enabled,
            "Activated cached config session"
        );

        !was_active
    }

    /// Switch to an already cached server session without reloading configs.
    pub fn activate_server(&mut self, server_id: impl Into<String>, cx: &mut Context<Self>) {
        if self.activate_server_session(server_id) {
            cx.notify();
        }
    }

    /// Check whether a specific server has a cached config session.
    pub fn has_server_session(&self, server_id: &str) -> bool {
        self.sessions.contains_key(server_id)
    }

    /// Get loading state for a specific server session.
    pub fn load_state_for_server(&self, server_id: &str) -> Option<ConfigLoadState> {
        self.sessions
            .get(server_id)
            .map(|session| session.load_state.clone())
    }

    /// Whether the active session has any configs cached.
    pub fn has_configs(&self) -> bool {
        !self.configs().is_empty()
    }

    /// Whether a specific server session has any configs cached.
    pub fn has_configs_for_server(&self, server_id: &str) -> bool {
        self.sessions
            .get(server_id)
            .is_some_and(|session| !session.configs.is_empty())
    }

    /// Remove a connected server
    pub fn remove_connected_server(&mut self, server_id: &str, cx: &mut Context<Self>) {
        self.connected_server_ids.retain(|id| id != server_id);
        self.sessions.remove(server_id);
        if self.active_server_id.as_deref() == Some(server_id) {
            self.active_server_id = None;
        }
        cx.notify();
    }

    /// Clear all state and reset to initial
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.sessions.clear();
        self.connected_server_ids.clear();
        self.active_server_id = None;
        cx.notify();
    }

    /// Go back to config list (deselect config)
    pub fn back_to_list(&mut self, cx: &mut Context<Self>) {
        let session = self.current_session_mut();
        session.selected_config_id = None;
        session.selected_topic_index = None;
        session.selected_agent_id = None;
        cx.notify();
    }
}

impl Default for ConfigState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(
        agent_id: &str,
        topics: Vec<(i32, &str, bool, &str)>,
        group_id: i32,
    ) -> TopicAgentItem {
        TopicAgentItem {
            agent_id: agent_id.to_string(),
            group_id,
            topics: topics
                .into_iter()
                .map(|(index, path, visibility, topic_type)| TopicDetail {
                    index,
                    path: path.to_string(),
                    visibility,
                    topic_type: topic_type.to_string(),
                })
                .collect(),
        }
    }

    fn make_config(group_id: i32, topic_agents: Vec<TopicAgentItem>) -> ConfigItem {
        ConfigItem {
            group_id,
            service_url: String::new(),
            source: String::new(),
            details: Vec::new(),
            topic_agents,
        }
    }

    #[test]
    fn merge_topic_agents_dedup_and_sort() {
        let mut state = ConfigState::new();

        let configs = vec![
            make_config(
                1,
                vec![make_agent(
                    "A",
                    vec![(20, "/a/ccc", true, "prop"), (10, "/a/bbb", true, "event")],
                    1,
                )],
            ),
            make_config(
                2,
                vec![make_agent(
                    "A",
                    vec![(10, "/a/bbb", true, "event"), (30, "/a/aaa", true, "cmd")],
                    2,
                )],
            ),
        ];

        state.current_session_mut().configs = configs;
        state.rebuild_topic_agents_merged();

        let agents: Vec<_> = state.topic_agents().into_iter().cloned().collect();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].agent_id, "A");

        let paths: Vec<_> = agents[0].topics.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(paths, vec!["/a/bbb", "/a/ccc", "/a/aaa"]);
    }

    #[test]
    fn merge_topic_type_prefers_non_unknown() {
        let mut state = ConfigState::new();

        let configs = vec![
            make_config(
                1,
                vec![make_agent("A", vec![(2, "/a/x", false, "unknown")], 1)],
            ),
            make_config(
                2,
                vec![make_agent("A", vec![(1, "/a/x", true, "event")], 2)],
            ),
        ];

        state.current_session_mut().configs = configs;
        state.rebuild_topic_agents_merged();

        let agents: Vec<_> = state.topic_agents().into_iter().cloned().collect();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].topics.len(), 1);
        assert_eq!(agents[0].topics[0].topic_type, "event");
        assert_eq!(agents[0].topics[0].visibility, true);
        assert_eq!(agents[0].topics[0].index, 1);
    }

    #[test]
    fn set_configs_initializes_selection() {
        let mut state = ConfigState::new();

        let configs = vec![
            make_config(
                1,
                vec![make_agent("B", vec![(0, "/b/x", true, "event")], 1)],
            ),
            make_config(
                2,
                vec![make_agent("A", vec![(0, "/a/x", true, "event")], 2)],
            ),
        ];

        state.apply_configs(configs);

        assert!(matches!(state.load_state(), ConfigLoadState::Loaded));
        // Agents are sorted by agent_id, so "A" should be first and selected.
        assert_eq!(state.selected_agent_id(), Some("A"));
        assert_eq!(state.selected_topic_index(), Some(0));
    }

    #[test]
    fn set_configs_empty_clears_selection() {
        let mut state = ConfigState::new();

        state.apply_configs(Vec::new());

        assert!(state.topic_agents().is_empty());
        assert_eq!(state.selected_agent_id(), None);
        assert_eq!(state.selected_topic_index(), None);
    }

    #[test]
    fn default_topic_index_prefers_fast_guarantee_prop_topic() {
        let agent = make_agent(
            "A",
            vec![
                (
                    0,
                    "non-persistent://goldwind/iothub/prop_data-BZ-GRID-realdev-Guarantee-1",
                    true,
                    "prop",
                ),
                (
                    1,
                    "non-persistent://goldwind/iothub/prop_data-BZ-FAST-realdev-Guarantee-1",
                    true,
                    "prop",
                ),
                (
                    2,
                    "non-persistent://goldwind/iothub/thing_event-BZ-FAST-realdev-Guarantee-1",
                    true,
                    "event",
                ),
            ],
            1,
        );

        assert_eq!(ConfigState::default_topic_index_for_agent(&agent), Some(1));
    }

    #[test]
    fn default_topic_index_falls_back_to_first_prop_then_first_topic() {
        let prop_fallback = make_agent(
            "A",
            vec![
                (
                    0,
                    "non-persistent://goldwind/iothub/thing_event-BZ-FAST-realdev-Guarantee-1",
                    true,
                    "event",
                ),
                (
                    1,
                    "non-persistent://goldwind/iothub/prop_data-BZ-GRID-realdev-Guarantee-1",
                    true,
                    "prop",
                ),
            ],
            1,
        );
        assert_eq!(
            ConfigState::default_topic_index_for_agent(&prop_fallback),
            Some(1)
        );

        let first_topic_fallback = make_agent(
            "B",
            vec![
                (
                    0,
                    "non-persistent://goldwind/iothub/thing_event-BZ-FAST-realdev-Guarantee-1",
                    true,
                    "event",
                ),
                (
                    1,
                    "non-persistent://goldwind/iothub/service-BZ-FAST-realdev-Guarantee-1",
                    true,
                    "cmd",
                ),
            ],
            1,
        );
        assert_eq!(
            ConfigState::default_topic_index_for_agent(&first_topic_fallback),
            Some(0)
        );
    }

    #[test]
    fn default_topic_index_treats_prop_data_path_as_prop_even_if_type_unknown() {
        let agent = make_agent(
            "A",
            vec![
                (
                    0,
                    "non-persistent://goldwind/iothub/thing_event-BZ-FAST-realdev-Guarantee-1",
                    true,
                    "event",
                ),
                (
                    1,
                    "non-persistent://goldwind/iothub/prop_data-BZ-GRID-realdev-Guarantee-1",
                    true,
                    "unknown",
                ),
            ],
            1,
        );

        assert_eq!(ConfigState::default_topic_index_for_agent(&agent), Some(1));
    }

    #[test]
    fn apply_configs_prefers_agent_with_fast_topic_over_empty_first_agent() {
        let mut state = ConfigState::new();

        let configs = vec![make_config(
            1,
            vec![
                make_agent("A", vec![], 1),
                make_agent(
                    "B",
                    vec![(
                        0,
                        "non-persistent://goldwind/iothub/prop_data-BZ-FAST-realdev-Guarantee-1",
                        true,
                        "unknown",
                    )],
                    1,
                ),
            ],
        )];

        state.apply_configs(configs);

        assert_eq!(state.selected_agent_id(), Some("B"));
        assert_eq!(state.selected_topic_index(), Some(0));
    }

    #[test]
    fn apply_configs_restores_previous_agent_and_topic_by_path() {
        let mut state = ConfigState::new();

        state.apply_configs(vec![make_config(
            1,
            vec![make_agent(
                "A",
                vec![(0, "/a/prop", true, "prop"), (1, "/a/event", true, "event")],
                1,
            )],
        )]);
        let session = state.current_session_mut();
        session.selected_agent_id = Some("A".to_string());
        session.selected_topic_index = Some(1);

        state.apply_configs(vec![make_config(
            2,
            vec![make_agent(
                "A",
                vec![
                    (10, "/a/other", true, "prop"),
                    (20, "/a/event", true, "event"),
                ],
                2,
            )],
        )]);

        assert_eq!(state.selected_agent_id(), Some("A"));
        assert_eq!(state.selected_topic_index(), Some(1));
        assert_eq!(state.selected_topic_path(), Some("/a/event"));
    }

    #[test]
    fn apply_configs_restores_previous_agent_and_falls_back_when_topic_missing() {
        let mut state = ConfigState::new();

        state.apply_configs(vec![make_config(
            1,
            vec![make_agent(
                "A",
                vec![(0, "/a/event", true, "event"), (1, "/a/prop", true, "prop")],
                1,
            )],
        )]);
        let session = state.current_session_mut();
        session.selected_agent_id = Some("A".to_string());
        session.selected_topic_index = Some(0);

        state.apply_configs(vec![make_config(
            2,
            vec![make_agent("A", vec![(5, "/a/prop", true, "prop")], 2)],
        )]);

        assert_eq!(state.selected_agent_id(), Some("A"));
        assert_eq!(state.selected_topic_index(), Some(0));
        assert_eq!(state.selected_topic_path(), Some("/a/prop"));
    }

    #[test]
    fn apply_configs_skips_restoring_empty_previous_agent() {
        let mut state = ConfigState::new();

        state.apply_configs(vec![make_config(
            1,
            vec![
                make_agent("A", vec![(0, "/a/event", true, "event")], 1),
                make_agent("B", vec![(0, "/b/prop", true, "prop")], 1),
            ],
        )]);
        let session = state.current_session_mut();
        session.selected_agent_id = Some("A".to_string());
        session.selected_topic_index = Some(0);

        state.apply_configs(vec![make_config(
            2,
            vec![
                make_agent("A", vec![], 2),
                make_agent("B", vec![(0, "/b/prop", true, "prop")], 2),
            ],
        )]);

        assert_eq!(state.selected_agent_id(), Some("B"));
        assert_eq!(state.selected_topic_index(), Some(0));
        assert_eq!(state.selected_topic_path(), Some("/b/prop"));
    }

    #[test]
    fn apply_agent_selection_is_noop_for_same_agent() {
        let mut state = ConfigState::new();

        state.apply_configs(vec![make_config(
            1,
            vec![make_agent(
                "A",
                vec![(0, "/a/prop", true, "prop"), (1, "/a/event", true, "event")],
                1,
            )],
        )]);
        let session = state.current_session_mut();
        session.selected_agent_id = Some("A".to_string());
        session.selected_topic_index = Some(1);

        let changed = state.apply_agent_selection(Some("A".to_string()));

        assert!(!changed);
        assert_eq!(state.selected_agent_id(), Some("A"));
        assert_eq!(state.selected_topic_index(), Some(1));
        assert_eq!(state.selected_topic_path(), Some("/a/event"));
    }

    #[test]
    fn server_sessions_keep_cached_configs_isolated() {
        let mut state = ConfigState::new();

        state.active_server_id = Some("server-a".to_string());
        state.apply_configs(vec![make_config(
            1,
            vec![make_agent("A", vec![(0, "/a/prop", true, "prop")], 1)],
        )]);

        state.active_server_id = Some("server-b".to_string());
        state.apply_configs(vec![make_config(
            2,
            vec![make_agent("B", vec![(0, "/b/event", true, "event")], 2)],
        )]);

        assert_eq!(state.selected_agent_id(), Some("B"));
        assert_eq!(state.selected_topic_path(), Some("/b/event"));

        state.active_server_id = Some("server-a".to_string());
        assert_eq!(state.selected_agent_id(), Some("A"));
        assert_eq!(state.selected_topic_path(), Some("/a/prop"));

        state.active_server_id = Some("server-b".to_string());
        assert_eq!(state.selected_agent_id(), Some("B"));
        assert_eq!(state.selected_topic_path(), Some("/b/event"));
    }

    #[test]
    fn server_sessions_keep_agent_search_isolated() {
        let mut state = ConfigState::new();

        state.active_server_id = Some("server-a".to_string());
        assert!(state.apply_agent_search_session(AgentSearchSession {
            query: "626221".to_string(),
            query_mode: AgentQueryMode::Prefix,
        }));

        state.active_server_id = Some("server-b".to_string());
        assert!(state.apply_agent_search_session(AgentSearchSession {
            query: "650412".to_string(),
            query_mode: AgentQueryMode::Exact,
        }));

        assert_eq!(state.agent_search_session().query, "650412");
        assert_eq!(
            state.agent_search_session().query_mode,
            AgentQueryMode::Exact
        );

        state.active_server_id = Some("server-a".to_string());
        assert_eq!(state.agent_search_session().query, "626221");
        assert_eq!(
            state.agent_search_session().query_mode,
            AgentQueryMode::Prefix
        );

        state.active_server_id = Some("server-c".to_string());
        assert_eq!(state.agent_search_session(), AgentSearchSession::default());
    }

    #[test]
    fn removing_server_session_drops_agent_search_state() {
        let mut state = ConfigState::new();

        state.connected_server_ids = vec!["server-a".to_string(), "server-b".to_string()];
        state.active_server_id = Some("server-a".to_string());
        state.apply_agent_search_session(AgentSearchSession {
            query: "626221".to_string(),
            query_mode: AgentQueryMode::All,
        });
        state.active_server_id = Some("server-b".to_string());
        state.apply_agent_search_session(AgentSearchSession {
            query: "650412".to_string(),
            query_mode: AgentQueryMode::All,
        });

        state.connected_server_ids.retain(|id| id != "server-a");
        state.sessions.remove("server-a");
        state.active_server_id = Some("server-a".to_string());

        assert_eq!(state.agent_search_session(), AgentSearchSession::default());
    }

    #[test]
    fn activate_server_disables_auto_topic_sync_until_manual_selection() {
        let mut state = ConfigState::new();

        state.active_server_id = Some("server-a".to_string());
        state.apply_configs(vec![make_config(
            1,
            vec![make_agent("A", vec![(0, "/a/prop", true, "prop")], 1)],
        )]);
        state.active_server_id = Some("server-b".to_string());
        state.apply_configs(vec![make_config(
            2,
            vec![make_agent("B", vec![(0, "/b/event", true, "event")], 2)],
        )]);

        state.activate_server_session("server-a");

        assert_eq!(state.selected_agent_id(), Some("A"));
        assert_eq!(state.selected_topic_index(), Some(0));
        assert_eq!(state.synced_selected_topic_index(), None);
        assert_eq!(state.selected_topic_path(), Some("/a/prop"));
        assert!(!state.topic_sync_enabled());
    }

    #[test]
    fn restore_stale_loading_recovers_previous_loaded_state() {
        let mut state = ConfigState::new();
        state.active_server_id = Some("server-a".to_string());
        state.apply_configs(vec![make_config(
            1,
            vec![make_agent("A", vec![(0, "/a/prop", true, "prop")], 1)],
        )]);

        let session = state.session_for_server_mut("server-a");
        ConfigState::mark_session_loading(session, Some(7));
        assert!(matches!(session.load_state, ConfigLoadState::Loading));

        assert!(ConfigState::restore_stale_loading_for_session(
            state.session_for_server_mut("server-a"),
            7,
        ));

        let session = state.session_for_server_mut("server-a");
        assert!(matches!(session.load_state, ConfigLoadState::Loaded));
        assert_eq!(session.pending_request_id, None);
        assert!(session.resume_load_state.is_none());
    }

    #[test]
    fn restore_stale_loading_does_not_change_active_server_session() {
        let mut state = ConfigState::new();
        state.active_server_id = Some("server-a".to_string());
        state.apply_configs(vec![make_config(
            1,
            vec![make_agent("A", vec![(0, "/a/prop", true, "prop")], 1)],
        )]);
        state.active_server_id = Some("server-b".to_string());
        state.apply_configs(vec![make_config(
            2,
            vec![make_agent("B", vec![(0, "/b/event", true, "event")], 2)],
        )]);

        ConfigState::mark_session_loading(state.session_for_server_mut("server-a"), Some(11));
        state.active_server_id = Some("server-b".to_string());

        assert!(ConfigState::restore_stale_loading_for_session(
            state.session_for_server_mut("server-a"),
            11,
        ));

        assert_eq!(state.active_server_id(), Some("server-b"));
        assert_eq!(state.selected_agent_id(), Some("B"));
        assert_eq!(state.selected_topic_path(), Some("/b/event"));
    }
}
