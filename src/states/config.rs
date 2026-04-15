//! Configuration State
//!
//! Manages the state of Redis configuration items and their loading status.

use crate::connection::{ConfigItem, ConfigLoadState, DetailItem, TopicAgentItem, TopicDetail};
use gpui::Context;
use std::collections::{BTreeMap, btree_map::Entry};
use std::sync::Arc;

/// Configuration state for managing Redis config items
pub struct ConfigState {
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
    /// IDs of all connected servers (supports multiple)
    connected_server_ids: Vec<String>,
    /// Currently selected TopicAgentId
    selected_agent_id: Option<String>,
}

impl ConfigState {
    const PREFERRED_AGENT_TOPIC_PREFIX: &str =
        "non-persistent://goldwind/iothub/prop_data-BZ-FAST-realdev-Guarantee";

    /// Create a new empty config state
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            topic_agents_merged: Vec::new(),
            load_state: ConfigLoadState::Idle,
            selected_config_id: None,
            selected_topic_index: None,
            connected_server_ids: Vec::new(),
            selected_agent_id: None,
        }
    }

    // ==================== Getters ====================

    /// Get all configuration items
    pub fn configs(&self) -> &[ConfigItem] {
        &self.configs
    }

    /// Get the current loading state
    pub fn load_state(&self) -> &ConfigLoadState {
        &self.load_state
    }

    /// Check if currently loading
    pub fn is_loading(&self) -> bool {
        self.load_state.is_loading()
    }

    /// Get the selected config group ID
    pub fn selected_config_id(&self) -> Option<i32> {
        self.selected_config_id
    }

    /// Get the selected topic index
    pub fn selected_topic_index(&self) -> Option<i32> {
        self.selected_topic_index
    }

    /// Get all connected server IDs
    pub fn connected_server_ids(&self) -> &[String] {
        &self.connected_server_ids
    }

    /// Get the currently selected config item
    pub fn selected_config(&self) -> Option<&ConfigItem> {
        self.selected_config_id
            .and_then(|id| self.configs.iter().find(|c| c.group_id == id))
    }

    /// Get the currently selected topic detail
    pub fn selected_topic(&self) -> Option<&DetailItem> {
        self.selected_config().and_then(|config| {
            self.selected_topic_index
                .and_then(|idx| config.details.iter().find(|d| d.index == idx))
        })
    }

    /// Get the selected agent ID
    pub fn selected_agent_id(&self) -> Option<&str> {
        self.selected_agent_id.as_deref()
    }

    /// Get all TopicAgentItems from the selected config
    pub fn topic_agents(&self) -> Vec<&TopicAgentItem> {
        self.topic_agents_merged.iter().collect()
    }

    /// Get the currently selected TopicAgentItem
    pub fn selected_agent(&self) -> Option<&TopicAgentItem> {
        self.selected_agent_id.as_ref().and_then(|aid| {
            self.topic_agents_merged
                .iter()
                .find(|ta| &ta.agent_id == aid)
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

    fn default_agent_selection(&self) -> Option<(String, Option<i32>)> {
        self.topic_agents_merged
            .iter()
            .find(|agent| agent.topics.iter().any(Self::is_preferred_agent_topic))
            .map(|agent| {
                (
                    agent.agent_id.clone(),
                    Self::default_topic_index_for_agent(agent),
                )
            })
            .or_else(|| {
                self.topic_agents_merged
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
                self.topic_agents_merged
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
                self.topic_agents_merged
                    .first()
                    .map(|agent| (agent.agent_id.clone(), None))
            })
    }

    fn selected_topic_path(&self) -> Option<&str> {
        let idx = self.selected_topic_index? as usize;
        self.selected_agent()
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

    fn restore_previous_selection(
        &mut self,
        previous_agent_id: Option<&str>,
        previous_topic_path: Option<&str>,
    ) -> bool {
        let Some(previous_agent_id) = previous_agent_id else {
            return false;
        };

        let Some(agent) = self
            .topic_agents_merged
            .iter()
            .find(|agent| agent.agent_id == previous_agent_id)
        else {
            return false;
        };

        if agent.topics.is_empty() {
            return false;
        }

        self.selected_agent_id = Some(agent.agent_id.clone());
        self.selected_topic_index = previous_topic_path
            .and_then(|topic_path| Self::topic_index_by_path(agent, topic_path))
            .or_else(|| Self::default_topic_index_for_agent(agent));

        true
    }

    fn rebuild_topic_agents_merged(&mut self) {
        #[derive(Clone, Debug)]
        struct MergedTopicMeta {
            index: i32,
            visibility: bool,
            topic_type: String,
        }

        let mut merged: BTreeMap<String, BTreeMap<String, MergedTopicMeta>> = BTreeMap::new();

        for config in &self.configs {
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

        self.topic_agents_merged = merged
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

    fn apply_configs(&mut self, configs: Vec<ConfigItem>) {
        let previous_agent_id = self.selected_agent_id.clone();
        let previous_topic_path = self.selected_topic_path().map(ToOwned::to_owned);

        self.configs = configs;
        self.rebuild_topic_agents_merged();
        self.selected_config_id = None;

        if self.restore_previous_selection(
            previous_agent_id.as_deref(),
            previous_topic_path.as_deref(),
        ) {
            self.load_state = ConfigLoadState::Loaded;
            return;
        }

        // Initialize selection to first agent/topic when available
        if let Some((agent_id, topic_index)) = self.default_agent_selection() {
            self.selected_agent_id = Some(agent_id);
            self.selected_topic_index = topic_index;
        } else {
            self.selected_agent_id = None;
            self.selected_topic_index = None;
        }

        self.load_state = ConfigLoadState::Loaded;
    }

    /// Set configuration items
    pub fn set_configs(&mut self, configs: Vec<ConfigItem>, cx: &mut Context<Self>) {
        self.apply_configs(configs);
        cx.notify();
    }

    /// Set loading state
    pub fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.load_state = ConfigLoadState::Loading;
        cx.notify();
    }

    /// Set error state
    pub fn set_error(&mut self, message: impl Into<Arc<str>>, cx: &mut Context<Self>) {
        self.load_state = ConfigLoadState::Error(message.into());
        cx.notify();
    }

    /// Select a configuration by group ID
    pub fn select_config(&mut self, group_id: Option<i32>, cx: &mut Context<Self>) {
        self.selected_config_id = group_id;
        // Reset topic selection when changing config
        if group_id.is_some() {
            self.selected_topic_index = Some(0);
        } else {
            self.selected_topic_index = None;
        }
        cx.notify();
    }

    /// Select a topic by index
    pub fn select_topic(&mut self, index: Option<i32>, cx: &mut Context<Self>) {
        self.selected_topic_index = index;
        cx.notify();
    }

    fn apply_agent_selection(&mut self, agent_id: Option<String>) -> bool {
        if self.selected_agent_id == agent_id {
            return false;
        }

        self.selected_agent_id = agent_id;
        self.selected_topic_index = self
            .selected_agent()
            .and_then(Self::default_topic_index_for_agent);

        true
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
                selected_topic_index = self.selected_topic_index,
                "Selected TopicAgentId"
            );
        } else {
            tracing::info!(agent_id = ?self.selected_agent_id.as_deref(), "Selected TopicAgentId (not found)");
        }
        cx.notify();
    }

    /// Add a connected server (no duplicates)
    pub fn add_connected_server(&mut self, server_id: String, cx: &mut Context<Self>) {
        if !self.connected_server_ids.iter().any(|id| id == &server_id) {
            self.connected_server_ids.push(server_id);
        }
        cx.notify();
    }

    /// Remove a connected server
    pub fn remove_connected_server(&mut self, server_id: &str, cx: &mut Context<Self>) {
        self.connected_server_ids.retain(|id| id != server_id);
        cx.notify();
    }

    /// Clear all state and reset to initial
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.configs.clear();
        self.topic_agents_merged.clear();
        self.load_state = ConfigLoadState::Idle;
        self.selected_config_id = None;
        self.selected_topic_index = None;
        self.connected_server_ids.clear();
        self.selected_agent_id = None;
        cx.notify();
    }

    /// Go back to config list (deselect config)
    pub fn back_to_list(&mut self, cx: &mut Context<Self>) {
        self.selected_config_id = None;
        self.selected_topic_index = None;
        self.selected_agent_id = None;
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

        state.configs = configs;
        state.rebuild_topic_agents_merged();

        let agents = state.topic_agents_merged.clone();
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

        state.configs = configs;
        state.rebuild_topic_agents_merged();

        let agents = state.topic_agents_merged.clone();
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
        state.selected_agent_id = Some("A".to_string());
        state.selected_topic_index = Some(1);

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
        state.selected_agent_id = Some("A".to_string());
        state.selected_topic_index = Some(0);

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
        state.selected_agent_id = Some("A".to_string());
        state.selected_topic_index = Some(0);

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
        state.selected_agent_id = Some("A".to_string());
        state.selected_topic_index = Some(1);

        let changed = state.apply_agent_selection(Some("A".to_string()));

        assert!(!changed);
        assert_eq!(state.selected_agent_id(), Some("A"));
        assert_eq!(state.selected_topic_index(), Some(1));
        assert_eq!(state.selected_topic_path(), Some("/a/event"));
    }
}
