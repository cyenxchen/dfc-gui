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
    /// ID of the connected server
    connected_server_id: Option<String>,
    /// Currently selected TopicAgentId
    selected_agent_id: Option<String>,
}

impl ConfigState {
    /// Create a new empty config state
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            topic_agents_merged: Vec::new(),
            load_state: ConfigLoadState::Idle,
            selected_config_id: None,
            selected_topic_index: None,
            connected_server_id: None,
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

    /// Get the connected server ID
    pub fn connected_server_id(&self) -> Option<&str> {
        self.connected_server_id.as_deref()
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
        self.selected_agent_id
            .as_ref()
            .and_then(|aid| self.topic_agents_merged.iter().find(|ta| &ta.agent_id == aid))
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
        self.configs = configs;
        self.rebuild_topic_agents_merged();
        self.selected_config_id = None;

        // Initialize selection to first agent/topic when available
        if let Some(first_agent) = self.topic_agents_merged.first() {
            self.selected_agent_id = Some(first_agent.agent_id.clone());
            self.selected_topic_index = None;
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

    /// Select a TopicAgentId
    pub fn select_agent(&mut self, agent_id: Option<String>, cx: &mut Context<Self>) {
        self.selected_agent_id = agent_id;
        // No topic selected by default
        self.selected_topic_index = None;
        cx.notify();
    }

    /// Set the connected server ID
    pub fn set_connected_server(&mut self, server_id: Option<String>, cx: &mut Context<Self>) {
        self.connected_server_id = server_id;
        cx.notify();
    }

    /// Clear all state and reset to initial
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.configs.clear();
        self.topic_agents_merged.clear();
        self.load_state = ConfigLoadState::Idle;
        self.selected_config_id = None;
        self.selected_topic_index = None;
        self.connected_server_id = None;
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
            make_config(1, vec![make_agent(
                "A",
                vec![(20, "/a/ccc", true, "prop"), (10, "/a/bbb", true, "event")],
                1,
            )]),
            make_config(2, vec![make_agent(
                "A",
                vec![(10, "/a/bbb", true, "event"), (30, "/a/aaa", true, "cmd")],
                2,
            )]),
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
            make_config(1, vec![make_agent("A", vec![(2, "/a/x", false, "unknown")], 1)]),
            make_config(2, vec![make_agent("A", vec![(1, "/a/x", true, "event")], 2)]),
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
            make_config(1, vec![make_agent("B", vec![(0, "/b/x", true, "event")], 1)]),
            make_config(2, vec![make_agent("A", vec![(0, "/a/x", true, "event")], 2)]),
        ];

        state.apply_configs(configs);

        assert!(matches!(state.load_state(), ConfigLoadState::Loaded));
        // Agents are sorted by agent_id, so "A" should be first and selected.
        assert_eq!(state.selected_agent_id(), Some("A"));
        assert_eq!(state.selected_topic_index(), None);
    }

    #[test]
    fn set_configs_empty_clears_selection() {
        let mut state = ConfigState::new();

        state.apply_configs(Vec::new());

        assert!(state.topic_agents().is_empty());
        assert_eq!(state.selected_agent_id(), None);
        assert_eq!(state.selected_topic_index(), None);
    }
}
