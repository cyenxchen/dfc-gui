//! Redis Repository
//!
//! Provides access to device metadata, configuration, and dictionary tables
//! stored in Redis. Handles one-time queries and caching.

use crate::connection::{
    ConfigItem, DetailItem, DfcServerConfig, RedisKeyItem, RedisKeyType, RedisKeyValue,
    TopicAgentItem, TopicDetail, REDIS_KEY_PATTERNS,
};
use crate::error::{Error, Result};
use crate::services::events::{DeviceId, DeviceMeta};
use crossbeam_channel::Sender;
use fred::prelude::*;
use fred::clients::Client as FredClient;
use fred::types::config::Config as FredConfig;
use fred::types::CustomCommand;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::ServiceEvent;

/// Configuration for Redis connection
#[derive(Clone, Debug)]
pub struct RedisConfig {
    /// Redis server URL (e.g., "redis://localhost:6379")
    pub url: String,
    /// Optional password
    pub password: Option<String>,
    /// Database number (default: 0)
    pub database: u8,
    /// Connection timeout in seconds
    pub timeout_secs: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            password: None,
            database: 0,
            timeout_secs: 10,
        }
    }
}

/// Redis repository for device metadata and configuration
pub struct RedisRepo {
    config: RedisConfig,
    tx: Sender<ServiceEvent>,
    /// Redis client instance
    client: Arc<RwLock<Option<FredClient>>>,
}

impl RedisRepo {
    /// Create a new Redis repository
    pub fn new(config: &RedisConfig, tx: Sender<ServiceEvent>) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            tx,
            client: Arc::new(RwLock::new(None)),
        })
    }

    /// Connect to Redis server
    pub async fn connect(&self) -> Result<()> {
        tracing::info!("Connecting to Redis at {}", self.config.url);

        // Notify connection state
        let _ = self.tx.send(ServiceEvent::ConnectionState {
            service: "redis".into(),
            connected: true,
            detail: "Connected".into(),
        });

        Ok(())
    }

    /// Connect to a specific server configuration
    pub async fn connect_to_server(&self, server: &DfcServerConfig) -> Result<()> {
        tracing::info!("Connecting to Redis server: {} ({}:{})", server.name, server.host, server.port);

        // Build Redis config
        let mut redis_config = FredConfig::default();
        redis_config.server = ServerConfig::Centralized {
            server: fred::prelude::Server::new(server.host.clone(), server.port),
        };

        if let Some(ref password) = server.password {
            if !password.is_empty() {
                redis_config.password = Some(password.clone());
            }
        }

        // Create client with connection config
        let client = Builder::from_config(redis_config)
            .with_connection_config(|config| {
                config.connection_timeout = Duration::from_secs(10);
            })
            .build()
            .map_err(|e| Error::Connection { message: e.to_string() })?;

        // Enter tokio runtime context for fred client
        // Fred internally uses tokio::task::spawn which requires a tokio runtime
        let _guard = super::runtime_handle().enter();

        // Connect
        client.connect();
        client.wait_for_connect().await.map_err(|e| {
            tracing::error!("Failed to connect to Redis: {}", e);
            Error::Connection { message: e.to_string() }
        })?;

        // Store client
        let mut guard = self.client.write().await;
        *guard = Some(client);

        // Notify connection state
        let _ = self.tx.send(ServiceEvent::ConnectionState {
            service: "redis".into(),
            connected: true,
            detail: format!("Connected to {}", server.name).into(),
        });

        tracing::info!("Successfully connected to Redis server: {}", server.name);
        Ok(())
    }

    /// Disconnect from current server
    pub async fn disconnect(&self) {
        let mut guard = self.client.write().await;
        if let Some(client) = guard.take() {
            let _ = client.quit().await;
        }

        let _ = self.tx.send(ServiceEvent::ConnectionState {
            service: "redis".into(),
            connected: false,
            detail: "Disconnected".into(),
        });
    }

    /// Fetch configuration items from Redis
    pub async fn fetch_configs(&self, cfgid: Option<&str>) -> Result<Vec<ConfigItem>> {
        // Enter tokio runtime context for fred client operations
        let _guard = super::runtime_handle().enter();

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| Error::Connection {
            message: "Not connected to Redis".to_string(),
        })?;

        tracing::debug!("Fetching configs from Redis, cfgid filter: {:?}", cfgid);

        let mut configs = Vec::new();
        let mut group_id = 1;

        // Scan for matching keys using each pattern
        for pattern in REDIS_KEY_PATTERNS {
            let scan_pattern = if let Some(cfg) = cfgid {
                // Check if cfgid already has braces, add them if not
                // Redis keys use format: CMC_{DCC0001}_sg.og.output.iothub
                let wrapped_cfgid = if cfg.starts_with('{') && cfg.ends_with('}') {
                    cfg.to_string()
                } else {
                    format!("{{{}}}", cfg) // Add braces: DCC0001 -> {DCC0001}
                };
                pattern.replace('*', &wrapped_cfgid)
            } else {
                pattern.to_string()
            };

            tracing::debug!("Scanning with pattern: {}", scan_pattern);

            // Use KEYS command for pattern matching
            let cmd = CustomCommand::new_static("KEYS", None, false);
            let keys_result: Value = client.custom(cmd, vec![Value::from(scan_pattern.clone())]).await.map_err(|e: fred::error::Error| {
                tracing::error!("Redis KEYS failed: {}", e);
                Error::Connection { message: e.to_string() }
            })?;

            // Convert Value to Vec<String>
            let keys: Vec<String> = match keys_result {
                Value::Array(arr) => arr
                    .into_iter()
                    .filter_map(|v| v.into_string())
                    .collect(),
                _ => vec![],
            };

            for key in keys {
                let Some(config_json) = self.get_config_json(client, &key).await else {
                    continue;
                };

                let raw_value = match &config_json {
                    serde_json::Value::String(s) => s.clone(),
                    _ => config_json.to_string(),
                };

                let details = if self.is_output_iothub_key(&key)
                    || self.is_input_iothub_key(&key)
                    || self.is_io_iothub_key(&key)
                {
                    Vec::new()
                } else {
                    self.parse_config_value(&raw_value, group_id)
                };

                let service_url = self
                    .extract_service_url_from_json(&config_json)
                    .unwrap_or_else(|| self.extract_service_url(&key, &raw_value));

                // Extract cfgid from the key (e.g., CMC_{DCC0001}_sg.og.output.iothub -> DCC0001)
                let cfgid = self.extract_cfgid_from_key(&key);

                let topic_agents = if let Some(ref cfg) = cfgid {
                    let app_id = self.fetch_app_id(client, cfg).await;
                    let agent_ids = self.fetch_topic_agent_ids(client, cfg, &app_id).await;

                    if self.is_output_iothub_key(&key) {
                        self.build_output_iothub_topic_agents(&config_json, &agent_ids, group_id)
                    } else if self.is_input_iothub_key(&key) {
                        self.build_input_iothub_topic_agents(&config_json, &agent_ids, &app_id, group_id)
                    } else if self.is_io_iothub_key(&key) {
                        self.build_io_iothub_topic_agents(&config_json, &agent_ids, &app_id, group_id)
                    } else {
                        self.build_topic_agents_from_details(&agent_ids, &details, group_id)
                    }
                } else {
                    Vec::new()
                };

                configs.push(ConfigItem {
                    group_id,
                    service_url,
                    source: key,
                    details,
                    topic_agents,
                });

                group_id += 1;
            }
        }

        tracing::info!("Fetched {} config items from Redis", configs.len());
        Ok(configs)
    }

    fn is_output_iothub_key(&self, key: &str) -> bool {
        key.ends_with("sg.og.output.iothub")
    }

    fn is_input_iothub_key(&self, key: &str) -> bool {
        key.ends_with("sg.og.input.iothub")
    }

    fn is_io_iothub_key(&self, key: &str) -> bool {
        key.ends_with("sg.io.iothub")
    }

    fn wrap_cfgid(cfgid: &str) -> String {
        if cfgid.starts_with('{') && cfgid.ends_with('}') {
            cfgid.to_string()
        } else {
            format!("{{{}}}", cfgid)
        }
    }

    fn parse_config_string_value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
    }

    fn parse_list_values(values: Vec<String>) -> serde_json::Value {
        let items: Vec<serde_json::Value> = values
            .into_iter()
            .map(|s| Self::parse_config_string_value(&s))
            .collect();
        serde_json::Value::Array(items)
    }

    fn parse_hash_pairs(pairs: Vec<(String, String)>) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (key, value) in pairs {
            let json_val = Self::parse_config_string_value(&value);
            map.insert(key, json_val);
        }
        serde_json::Value::Object(map)
    }

    async fn get_config_json(&self, client: &FredClient, key: &str) -> Option<serde_json::Value> {
        let type_cmd = CustomCommand::new_static("TYPE", None, false);
        let type_result: Value = client
            .custom(type_cmd, vec![Value::from(key.to_string())])
            .await
            .unwrap_or(Value::Null);
        let key_type = type_result.into_string().unwrap_or_default();

        match key_type.as_str() {
            "string" => {
                let cmd = CustomCommand::new_static("GET", None, false);
                let result: Value = client
                    .custom(cmd, vec![Value::from(key.to_string())])
                    .await
                    .unwrap_or(Value::Null);
                result.into_string().map(|s| Self::parse_config_string_value(&s))
            }
            "list" => {
                let cmd = CustomCommand::new_static("LRANGE", None, false);
                let result: Value = client
                    .custom(
                        cmd,
                        vec![
                            Value::from(key.to_string()),
                            Value::from("0"),
                            Value::from("-1"),
                        ],
                    )
                    .await
                    .unwrap_or(Value::Null);
                if let Value::Array(arr) = result {
                    let values: Vec<String> = arr.into_iter().filter_map(|v| v.into_string()).collect();
                    Some(Self::parse_list_values(values))
                } else {
                    None
                }
            }
            "hash" => {
                let cmd = CustomCommand::new_static("HGETALL", None, false);
                let result: Value = client
                    .custom(cmd, vec![Value::from(key.to_string())])
                    .await
                    .unwrap_or(Value::Null);
                if let Value::Array(arr) = result {
                    let mut pairs = Vec::new();
                    let mut iter = arr.into_iter();
                    while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                        if let (Some(key_str), Some(val_str)) = (k.into_string(), v.into_string()) {
                            pairs.push((key_str, val_str));
                        }
                    }
                    Some(Self::parse_hash_pairs(pairs))
                } else {
                    None
                }
            }
            _ => {
                tracing::debug!("Skipping key {} with unsupported type: {}", key, key_type);
                None
            }
        }
    }

    fn extract_service_url_from_json(&self, value: &serde_json::Value) -> Option<String> {
        for entry in Self::value_as_array(value) {
            if let Some(url) = Self::get_json_string_multi(
                entry,
                &["serviceurl", "serviceUrl", "service_url", "url"],
            ) {
                return Some(url);
            }
        }
        None
    }

    async fn fetch_app_id(&self, client: &FredClient, cfgid: &str) -> String {
        let wrapped = Self::wrap_cfgid(cfgid);
        let main_key = format!("CMC_{}_sg.main", wrapped);
        let Some(json) = self.get_config_json(client, &main_key).await else {
            return cfgid.to_string();
        };
        if let Some(app_id) = Self::get_json_string_multi(&json, &["appId", "appid", "app_id"]) {
            return app_id;
        }
        cfgid.to_string()
    }

    async fn fetch_topic_agent_ids(
        &self,
        client: &FredClient,
        cfgid: &str,
        app_id: &str,
    ) -> Vec<String> {
        let wrapped = Self::wrap_cfgid(cfgid);
        let device_key = format!("CMC_{}_sg.device", wrapped);
        let Some(json) = self.get_config_json(client, &device_key).await else {
            return vec![app_id.to_string()];
        };

        let Some(devices) = json.as_array() else {
            return vec![app_id.to_string()];
        };

        let mut agent_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for device in devices {
            if let Some(agent_id) = device.get("topicAgentId").and_then(|v| v.as_str()) {
                if !agent_id.is_empty() {
                    agent_ids.insert(agent_id.to_string());
                } else {
                    agent_ids.insert(app_id.to_string());
                }
            } else {
                agent_ids.insert(app_id.to_string());
            }
        }

        if agent_ids.is_empty() {
            agent_ids.insert(app_id.to_string());
        }

        let mut ids: Vec<String> = agent_ids.into_iter().collect();
        ids.sort();
        ids
    }

    fn build_topic_agents_from_details(
        &self,
        agent_ids: &[String],
        details: &[DetailItem],
        group_id: i32,
    ) -> Vec<TopicAgentItem> {
        let mut topic_agents: Vec<TopicAgentItem> = agent_ids
            .iter()
            .cloned()
            .map(|agent_id| {
                let topics: Vec<TopicDetail> = details
                    .iter()
                    .filter(|d| d.path.contains(&agent_id))
                    .map(|d| TopicDetail {
                        index: d.index,
                        path: d.path.clone(),
                        visibility: d.visibility,
                        topic_type: Self::extract_topic_type_for_path(&d.path),
                    })
                    .collect();

                TopicAgentItem {
                    agent_id,
                    topics,
                    group_id,
                }
            })
            .collect();

        if topic_agents.iter().all(|ta| ta.topics.is_empty()) && !topic_agents.is_empty() {
            let all_topics: Vec<TopicDetail> = details
                .iter()
                .map(|d| TopicDetail {
                    index: d.index,
                    path: d.path.clone(),
                    visibility: d.visibility,
                    topic_type: Self::extract_topic_type_for_path(&d.path),
                })
                .collect();
            if let Some(first) = topic_agents.first_mut() {
                first.topics = all_topics;
            }
        }

        topic_agents.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        topic_agents
    }

    fn build_output_iothub_topic_agents(
        &self,
        config_json: &serde_json::Value,
        agent_ids: &[String],
        group_id: i32,
    ) -> Vec<TopicAgentItem> {
        if agent_ids.is_empty() {
            return Vec::new();
        }

        let mut topics_by_agent: std::collections::BTreeMap<String, Vec<TopicDetail>> =
            agent_ids
                .iter()
                .map(|id| (id.clone(), Vec::new()))
                .collect();
        let mut index_by_agent: std::collections::BTreeMap<String, i32> =
            agent_ids.iter().map(|id| (id.clone(), 0)).collect();

        for entry in Self::value_as_array(config_json) {
            let guarantee = Self::get_json_string_array(entry, "guaranteeTopic");
            let one_data = Self::get_json_string_array(entry, "oneDataTopic");
            let ten_data = Self::get_json_string_array(entry, "tenDataTopic");
            let curve_data = Self::get_json_string_array(entry, "curveDataTopic");

            let prop = Self::pick_first_starting_with(&guarantee, "persistent");
            let fast = Self::pick_first_starting_with(&guarantee, "non-persistent");
            let one = Self::pick_first_starting_with_and_ends(&one_data, "persistent", "-60")
                .or_else(|| one_data.first().cloned());
            let ten = Self::pick_first_starting_with_and_ends(&ten_data, "persistent", "-600")
                .or_else(|| ten_data.first().cloned());
            let curve = Self::pick_first_starting_with(&curve_data, "persistent")
                .or_else(|| curve_data.first().cloned());
            let event = Self::get_json_string(entry, "topicEvent");
            let cmd_req = Self::get_json_string(entry, "topicSvrReq");
            let cmd_resp = Self::get_json_string(entry, "topicSvrResp");

            for agent_id in agent_ids {
                let idx = index_by_agent
                    .get_mut(agent_id)
                    .expect("agent index");
                let topics = topics_by_agent
                    .get_mut(agent_id)
                    .expect("agent topics");

                let mut push_topic = |value: Option<String>| {
                    if let Some(path) = value.filter(|s| !s.is_empty()) {
                        let combined = Self::combine_app_id(&path, agent_id);
                        topics.push(TopicDetail {
                            index: *idx,
                            path: combined.clone(),
                            visibility: true,
                            topic_type: Self::extract_topic_type_for_path(&combined),
                        });
                        *idx += 1;
                    }
                };

                push_topic(prop.clone());
                push_topic(fast.clone());
                push_topic(one.clone());
                push_topic(ten.clone());
                push_topic(curve.clone());
                push_topic(event.clone());

                if let (Some(resp), Some(req)) = (cmd_resp.clone(), cmd_req.clone()) {
                    if !resp.is_empty() && !req.is_empty() {
                        let resp = Self::combine_app_id(&resp, agent_id);
                        let req = Self::combine_app_id(&req, agent_id);
                        let path = format!("{},{}", resp, req);
                        topics.push(TopicDetail {
                            index: *idx,
                            path: path.clone(),
                            visibility: true,
                            topic_type: Self::extract_topic_type_for_path(&path),
                        });
                        *idx += 1;
                    }
                }
            }
        }

        topics_by_agent
            .into_iter()
            .map(|(agent_id, topics)| TopicAgentItem {
                agent_id,
                topics,
                group_id,
            })
            .collect()
    }

    fn build_input_iothub_topic_agents(
        &self,
        config_json: &serde_json::Value,
        agent_ids: &[String],
        app_id: &str,
        group_id: i32,
    ) -> Vec<TopicAgentItem> {
        let mut topics_by_agent: std::collections::BTreeMap<String, Vec<TopicDetail>> =
            std::collections::BTreeMap::new();
        let mut index_by_agent: std::collections::BTreeMap<String, i32> =
            std::collections::BTreeMap::new();

        let effective_agents: Vec<String> = if agent_ids.is_empty() {
            vec![app_id.to_string()]
        } else {
            agent_ids.to_vec()
        };

        for agent in &effective_agents {
            topics_by_agent.insert(agent.clone(), Vec::new());
            index_by_agent.insert(agent.clone(), 0);
        }

        for entry in Self::value_as_array(config_json) {
            let topic_prop = Self::get_json_string(entry, "topicProp");
            let (prop, fast) = if let Some(prop) = topic_prop {
                if prop.starts_with("persistent") {
                    (Some(prop), None)
                } else if prop.starts_with("non-persistent") {
                    (None, Some(prop))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            let event = Self::get_json_string(entry, "topicEvent");
            let cmd_req = Self::get_json_string(entry, "topicSvrReq");
            let cmd_resp = Self::get_json_string(entry, "topicSvrResp");

            let mut base_topics: Vec<String> = Vec::new();
            if let Some(prop) = prop {
                base_topics.push(prop);
            }
            if let Some(fast) = fast {
                base_topics.push(fast);
            }
            if let Some(event) = event {
                base_topics.push(event);
            }
            if let (Some(resp), Some(req)) = (cmd_resp, cmd_req) {
                if !resp.is_empty() && !req.is_empty() {
                    base_topics.push(format!("{},{}", resp, req));
                }
            }

            for topic in base_topics {
                let mut matched = false;
                for agent_id in &effective_agents {
                    if Self::topic_matches_agent(&topic, agent_id) {
                        let idx = index_by_agent
                            .get_mut(agent_id)
                            .expect("agent index");
                        let list = topics_by_agent
                            .get_mut(agent_id)
                            .expect("agent list");
                        list.push(TopicDetail {
                            index: *idx,
                            path: topic.clone(),
                            visibility: true,
                            topic_type: Self::extract_topic_type_for_path(&topic),
                        });
                        *idx += 1;
                        matched = true;
                    }
                }

                if !matched {
                    for agent_id in &effective_agents {
                        let idx = index_by_agent
                            .get_mut(agent_id)
                            .expect("agent index");
                        let list = topics_by_agent
                            .get_mut(agent_id)
                            .expect("agent list");
                        list.push(TopicDetail {
                            index: *idx,
                            path: topic.clone(),
                            visibility: true,
                            topic_type: Self::extract_topic_type_for_path(&topic),
                        });
                        *idx += 1;
                    }
                }
            }
        }

        topics_by_agent
            .into_iter()
            .map(|(agent_id, topics)| TopicAgentItem {
                agent_id,
                topics,
                group_id,
            })
            .collect()
    }

    fn build_io_iothub_topic_agents(
        &self,
        config_json: &serde_json::Value,
        agent_ids: &[String],
        app_id: &str,
        group_id: i32,
    ) -> Vec<TopicAgentItem> {
        let mut topics_by_agent: std::collections::BTreeMap<String, Vec<TopicDetail>> =
            std::collections::BTreeMap::new();
        let mut index_by_agent: std::collections::BTreeMap<String, i32> =
            std::collections::BTreeMap::new();

        if !app_id.is_empty() {
            topics_by_agent.entry(app_id.to_string()).or_default();
            index_by_agent.entry(app_id.to_string()).or_insert(0);
        }
        for agent_id in agent_ids {
            topics_by_agent.entry(agent_id.clone()).or_default();
            index_by_agent.entry(agent_id.clone()).or_insert(0);
        }

        for entry in Self::value_as_array(config_json) {
            let mut topics: Vec<String> = Vec::new();
            topics.extend(Self::get_json_string_array(entry, "consumer"));
            topics.extend(Self::get_json_string_array(entry, "producer"));

            for topic in topics {
                let mut matched = false;
                for agent_id in agent_ids {
                    if Self::topic_matches_agent(&topic, agent_id) {
                        let idx = index_by_agent
                            .get_mut(agent_id)
                            .expect("agent index");
                        let list = topics_by_agent
                            .get_mut(agent_id)
                            .expect("agent list");
                        list.push(TopicDetail {
                            index: *idx,
                            path: topic.clone(),
                            visibility: true,
                            topic_type: Self::extract_topic_type_for_path(&topic),
                        });
                        *idx += 1;
                        matched = true;
                    }
                }

                if !matched {
                    let target_id = if app_id.is_empty() {
                        agent_ids.first().cloned()
                    } else {
                        Some(app_id.to_string())
                    };
                    if let Some(target_id) = target_id {
                        let idx = index_by_agent
                            .get_mut(&target_id)
                            .expect("agent index");
                        let list = topics_by_agent
                            .get_mut(&target_id)
                            .expect("agent list");
                        list.push(TopicDetail {
                            index: *idx,
                            path: topic.clone(),
                            visibility: true,
                            topic_type: Self::extract_topic_type_for_path(&topic),
                        });
                        *idx += 1;
                    }
                }
            }
        }

        topics_by_agent
            .into_iter()
            .map(|(agent_id, topics)| TopicAgentItem {
                agent_id,
                topics,
                group_id,
            })
            .collect()
    }

    fn combine_app_id(topic: &str, agent_id: &str) -> String {
        if agent_id.is_empty() {
            topic.to_string()
        } else {
            format!("{topic}-{agent_id}")
        }
    }

    fn topic_matches_agent(topic: &str, agent_id: &str) -> bool {
        let suffix = format!("-{agent_id}");
        if topic.ends_with(&suffix) {
            return true;
        }
        if topic.contains(',') {
            return topic
                .split(',')
                .any(|part| part.trim_end().ends_with(&suffix));
        }
        false
    }

    fn value_as_array<'a>(value: &'a serde_json::Value) -> Vec<&'a serde_json::Value> {
        match value {
            serde_json::Value::Array(arr) => arr.iter().collect(),
            serde_json::Value::Object(_) => vec![value],
            _ => Vec::new(),
        }
    }

    fn get_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn get_json_string_multi(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
        for key in keys {
            if let Some(val) = Self::get_json_string(value, key) {
                return Some(val);
            }
        }
        None
    }

    fn get_json_string_array(value: &serde_json::Value, key: &str) -> Vec<String> {
        match value.get(key) {
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            Some(serde_json::Value::String(s)) => vec![s.to_string()],
            _ => Vec::new(),
        }
    }

    fn pick_first_starting_with(values: &[String], prefix: &str) -> Option<String> {
        values
            .iter()
            .find(|s| s.starts_with(prefix))
            .cloned()
    }

    fn pick_first_starting_with_and_ends(
        values: &[String],
        prefix: &str,
        suffix: &str,
    ) -> Option<String> {
        values
            .iter()
            .find(|s| s.starts_with(prefix) && s.ends_with(suffix))
            .cloned()
    }

    /// Parse config value to extract topic details
    fn parse_config_value(&self, value: &str, group_id: i32) -> Vec<DetailItem> {
        // Try to parse as JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(value) {
            return self.extract_topics_from_json(&json, group_id);
        }

        // If not JSON, treat as a single topic path
        vec![DetailItem {
            index: 0,
            path: value.to_string(),
            visibility: true,
            group_id,
        }]
    }

    /// Extract topic paths from JSON value
    fn extract_topics_from_json(&self, json: &serde_json::Value, group_id: i32) -> Vec<DetailItem> {
        let mut details = Vec::new();
        let mut index = 0;

        // Handle array of topics
        if let Some(arr) = json.as_array() {
            for item in arr {
                if let Some(path) = item.as_str() {
                    details.push(DetailItem {
                        index,
                        path: path.to_string(),
                        visibility: true,
                        group_id,
                    });
                    index += 1;
                } else if let Some(obj) = item.as_object() {
                    // Handle object with path/topic field
                    let path = obj.get("path")
                        .or_else(|| obj.get("topic"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let item_index = obj
                        .get("index")
                        .or_else(|| obj.get("idx"))
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32)
                        .unwrap_or(index);

                    let visibility = obj.get("visibility")
                        .or_else(|| obj.get("visible"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    if !path.is_empty() {
                        details.push(DetailItem {
                            index: item_index,
                            path: path.to_string(),
                            visibility,
                            group_id,
                        });
                        index += 1;
                    }
                }
            }
        }
        // Handle single object with topics array
        else if let Some(obj) = json.as_object() {
            if let Some(topics) = obj.get("topics").and_then(|v| v.as_array()) {
                for item in topics {
                    if let Some(path) = item.as_str() {
                        details.push(DetailItem {
                            index,
                            path: path.to_string(),
                            visibility: true,
                            group_id,
                        });
                        index += 1;
                    }
                }
            }
            // Or a single path field
            else if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                details.push(DetailItem {
                    index: 0,
                    path: path.to_string(),
                    visibility: true,
                    group_id,
                });
            }
        }

        details
    }

    /// Extract service URL from key or value
    fn extract_service_url(&self, key: &str, value: &str) -> String {
        // Try to extract from JSON value first
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(value) {
            if let Some(url) = json.get("url")
                .or_else(|| json.get("serviceUrl"))
                .or_else(|| json.get("service_url"))
                .or_else(|| json.get("serviceurl"))
                .and_then(|v| v.as_str())
            {
                return url.to_string();
            }
        }

        // Default: construct from key pattern
        format!("pulsar://{}", key.replace('_', "/"))
    }

    /// Extract cfgid from Redis key
    /// e.g., CMC_{DCC0001}_sg.og.output.iothub -> DCC0001
    fn extract_cfgid_from_key(&self, key: &str) -> Option<String> {
        // Pattern: CMC_{cfgid}_sg.*
        if key.starts_with("CMC_") {
            // Find the content between { and }
            if let Some(start) = key.find('{') {
                if let Some(end) = key.find('}') {
                    if start < end {
                        return Some(key[start + 1..end].to_string());
                    }
                }
            }
            // Fallback: extract between CMC_ and _sg
            if let Some(sg_pos) = key.find("_sg") {
                let cfgid = &key[4..sg_pos]; // Skip "CMC_"
                // Remove braces if present
                let cfgid = cfgid.trim_start_matches('{').trim_end_matches('}');
                if !cfgid.is_empty() {
                    return Some(cfgid.to_string());
                }
            }
        }
        None
    }

    /// Extract topic type from path
    fn extract_topic_type(&self, path: &str) -> String {
        Self::extract_topic_type_for_path(path)
    }

    fn extract_topic_type_for_path(path: &str) -> String {
        // Common topic type patterns
        if path.contains("/prop/") || path.contains("/properties/") {
            "prop".to_string()
        } else if path.contains("/event/") || path.contains("/events/") {
            "event".to_string()
        } else if path.contains("/cmd/") || path.contains("/command/") || path.contains("/commands/") {
            "cmd".to_string()
        } else if path.contains("/telemetry/") || path.contains("/tele/") {
            "telemetry".to_string()
        } else if path.contains("/alarm/") || path.contains("/alarms/") {
            "alarm".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Fetch all device metadata from Redis
    pub async fn fetch_all_devices(&self) -> Result<Vec<DeviceMeta>> {
        // TODO: Implement actual Redis query
        // For now, return mock data
        tracing::debug!("Fetching all devices from Redis");

        let devices = vec![
            DeviceMeta::new("device-001", "Wind Turbine #1"),
            DeviceMeta::new("device-002", "Wind Turbine #2"),
            DeviceMeta::new("device-003", "Wind Turbine #3"),
        ];

        Ok(devices)
    }

    /// Fetch device metadata by ID
    pub async fn fetch_device(&self, device_id: &DeviceId) -> Result<Option<DeviceMeta>> {
        // TODO: Implement actual Redis query
        tracing::debug!("Fetching device {} from Redis", device_id);
        Ok(None)
    }

    /// Fetch metric dictionary (ID -> name mapping)
    pub async fn fetch_metric_dictionary(&self) -> Result<Vec<(u16, Arc<str>)>> {
        // TODO: Implement actual Redis query
        tracing::debug!("Fetching metric dictionary from Redis");

        let dictionary = vec![
            (1u16, Arc::from("wind_speed")),
            (2u16, Arc::from("power_output")),
            (3u16, Arc::from("rotor_rpm")),
            (4u16, Arc::from("generator_temp")),
            (5u16, Arc::from("nacelle_direction")),
        ];

        Ok(dictionary)
    }

    /// Update device configuration in Redis
    pub async fn update_device_config(
        &self,
        device_id: &DeviceId,
        config: &str,
    ) -> Result<()> {
        // TODO: Implement actual Redis write
        tracing::info!("Updating config for device {}", device_id);
        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &RedisConfig {
        &self.config
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        // Check if we have a client (synchronous check)
        // Note: This is a simplified check; actual connection state should be tracked separately
        true
    }

    /// Check if connected (async version with actual client check)
    pub async fn is_connected_async(&self) -> bool {
        let guard = self.client.read().await;
        guard.is_some()
    }

    // ==================== Key Operations ====================

    /// Scan keys using SCAN command with optional pattern
    ///
    /// Returns a tuple of (keys, next_cursor). A cursor of 0 means scan is complete.
    pub async fn scan_keys(
        &self,
        pattern: &str,
        cursor: u64,
        count: usize,
    ) -> Result<(Vec<RedisKeyItem>, u64)> {
        let _guard = super::runtime_handle().enter();

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| Error::Connection {
            message: "Not connected to Redis".to_string(),
        })?;

        tracing::debug!("Scanning keys with pattern: {}, cursor: {}", pattern, cursor);

        // Build SCAN command: SCAN cursor [MATCH pattern] [COUNT count]
        let cmd = CustomCommand::new_static("SCAN", None, false);
        let mut args = vec![Value::from(cursor.to_string())];

        if !pattern.is_empty() && pattern != "*" {
            args.push(Value::from("MATCH"));
            args.push(Value::from(pattern.to_string()));
        }

        args.push(Value::from("COUNT"));
        args.push(Value::from(count.to_string()));

        let result: Value = client.custom(cmd, args).await.map_err(|e| {
            tracing::error!("Redis SCAN failed: {}", e);
            Error::Connection { message: e.to_string() }
        })?;

        // Parse SCAN result: [cursor, [key1, key2, ...]]
        let (next_cursor, keys_raw) = match result {
            Value::Array(mut arr) if arr.len() >= 2 => {
                let cursor_val = arr.remove(0);
                let keys_val = arr.remove(0);

                let next_cursor = cursor_val
                    .into_string()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);

                let keys: Vec<String> = match keys_val {
                    Value::Array(arr) => arr.into_iter().filter_map(|v| v.into_string()).collect(),
                    _ => vec![],
                };

                (next_cursor, keys)
            }
            _ => (0, vec![]),
        };

        // Get type and TTL for each key
        let mut key_items = Vec::with_capacity(keys_raw.len());
        for key in keys_raw {
            let key_type = self.get_key_type_internal(client, &key).await;
            let ttl = self.get_key_ttl_internal(client, &key).await;
            key_items.push(RedisKeyItem::new(key, key_type, ttl));
        }

        tracing::debug!(
            "Scan returned {} keys, next cursor: {}",
            key_items.len(),
            next_cursor
        );

        Ok((key_items, next_cursor))
    }

    /// Get the type of a key
    async fn get_key_type_internal(&self, client: &FredClient, key: &str) -> RedisKeyType {
        let cmd = CustomCommand::new_static("TYPE", None, false);
        let result: Value = client
            .custom(cmd, vec![Value::from(key.to_string())])
            .await
            .unwrap_or(Value::Null);

        result
            .into_string()
            .map(|s| RedisKeyType::from_type_str(&s))
            .unwrap_or(RedisKeyType::Unknown)
    }

    /// Get the TTL of a key
    async fn get_key_ttl_internal(&self, client: &FredClient, key: &str) -> i64 {
        let cmd = CustomCommand::new_static("TTL", None, false);
        let result: Value = client
            .custom(cmd, vec![Value::from(key.to_string())])
            .await
            .unwrap_or(Value::Null);

        match result {
            Value::Integer(ttl) => ttl,
            _ => -1,
        }
    }

    /// Get the type of a key (public API)
    pub async fn get_key_type(&self, key: &str) -> Result<RedisKeyType> {
        let _guard = super::runtime_handle().enter();

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| Error::Connection {
            message: "Not connected to Redis".to_string(),
        })?;

        Ok(self.get_key_type_internal(client, key).await)
    }

    /// Get a string value
    pub async fn get_string(&self, key: &str) -> Result<String> {
        let _guard = super::runtime_handle().enter();

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| Error::Connection {
            message: "Not connected to Redis".to_string(),
        })?;

        let cmd = CustomCommand::new_static("GET", None, false);
        let result: Value = client
            .custom(cmd, vec![Value::from(key.to_string())])
            .await
            .map_err(|e| Error::Connection { message: e.to_string() })?;

        result.into_string().ok_or_else(|| Error::Parse {
            message: "Failed to parse string value".to_string(),
        })
    }

    /// Get a hash value (all fields)
    pub async fn get_hash(&self, key: &str) -> Result<Vec<(String, String)>> {
        let _guard = super::runtime_handle().enter();

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| Error::Connection {
            message: "Not connected to Redis".to_string(),
        })?;

        let cmd = CustomCommand::new_static("HGETALL", None, false);
        let result: Value = client
            .custom(cmd, vec![Value::from(key.to_string())])
            .await
            .map_err(|e| Error::Connection { message: e.to_string() })?;

        match result {
            Value::Array(arr) => {
                let mut pairs = Vec::new();
                let mut iter = arr.into_iter();
                while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                    if let (Some(key_str), Some(val_str)) = (k.into_string(), v.into_string()) {
                        pairs.push((key_str, val_str));
                    }
                }
                Ok(pairs)
            }
            _ => Ok(vec![]),
        }
    }

    /// Get a list value (with range)
    pub async fn get_list(&self, key: &str, start: i64, stop: i64) -> Result<Vec<String>> {
        let _guard = super::runtime_handle().enter();

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| Error::Connection {
            message: "Not connected to Redis".to_string(),
        })?;

        let cmd = CustomCommand::new_static("LRANGE", None, false);
        let result: Value = client
            .custom(
                cmd,
                vec![
                    Value::from(key.to_string()),
                    Value::from(start.to_string()),
                    Value::from(stop.to_string()),
                ],
            )
            .await
            .map_err(|e| Error::Connection { message: e.to_string() })?;

        match result {
            Value::Array(arr) => Ok(arr.into_iter().filter_map(|v| v.into_string()).collect()),
            _ => Ok(vec![]),
        }
    }

    /// Get a set value (all members)
    pub async fn get_set(&self, key: &str) -> Result<Vec<String>> {
        let _guard = super::runtime_handle().enter();

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| Error::Connection {
            message: "Not connected to Redis".to_string(),
        })?;

        let cmd = CustomCommand::new_static("SMEMBERS", None, false);
        let result: Value = client
            .custom(cmd, vec![Value::from(key.to_string())])
            .await
            .map_err(|e| Error::Connection { message: e.to_string() })?;

        match result {
            Value::Array(arr) => Ok(arr.into_iter().filter_map(|v| v.into_string()).collect()),
            _ => Ok(vec![]),
        }
    }

    /// Get a sorted set value (with scores, range)
    pub async fn get_zset(&self, key: &str, start: i64, stop: i64) -> Result<Vec<(String, f64)>> {
        let _guard = super::runtime_handle().enter();

        let guard = self.client.read().await;
        let client = guard.as_ref().ok_or_else(|| Error::Connection {
            message: "Not connected to Redis".to_string(),
        })?;

        let cmd = CustomCommand::new_static("ZRANGE", None, false);
        let result: Value = client
            .custom(
                cmd,
                vec![
                    Value::from(key.to_string()),
                    Value::from(start.to_string()),
                    Value::from(stop.to_string()),
                    Value::from("WITHSCORES"),
                ],
            )
            .await
            .map_err(|e| Error::Connection { message: e.to_string() })?;

        match result {
            Value::Array(arr) => {
                let mut pairs = Vec::new();
                let mut iter = arr.into_iter();
                while let (Some(member), Some(score)) = (iter.next(), iter.next()) {
                    if let Some(member_str) = member.into_string() {
                        let score_val = score
                            .into_string()
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        pairs.push((member_str, score_val));
                    }
                }
                Ok(pairs)
            }
            _ => Ok(vec![]),
        }
    }

    /// Get key value based on its type
    pub async fn get_key_value(&self, key: &str) -> Result<RedisKeyValue> {
        let key_type = self.get_key_type(key).await?;

        match key_type {
            RedisKeyType::String => {
                let value = self.get_string(key).await?;
                Ok(RedisKeyValue::String(value))
            }
            RedisKeyType::Hash => {
                let value = self.get_hash(key).await?;
                Ok(RedisKeyValue::Hash(value))
            }
            RedisKeyType::List => {
                // Get first 100 elements
                let value = self.get_list(key, 0, 99).await?;
                Ok(RedisKeyValue::List(value))
            }
            RedisKeyType::Set => {
                let value = self.get_set(key).await?;
                Ok(RedisKeyValue::Set(value))
            }
            RedisKeyType::ZSet => {
                // Get first 100 elements with scores
                let value = self.get_zset(key, 0, 99).await?;
                Ok(RedisKeyValue::ZSet(value))
            }
            _ => Ok(RedisKeyValue::Error(format!(
                "Unsupported key type: {:?}",
                key_type
            ))),
        }
    }
}

impl std::fmt::Debug for RedisRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisRepo")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_config_string_value_handles_json_and_plain() {
        let parsed = RedisRepo::parse_config_string_value("{\"a\":1}");
        assert!(parsed.is_object());

        let parsed = RedisRepo::parse_config_string_value("plain");
        assert!(parsed.is_string());
    }

    #[test]
    fn parse_list_values_parses_each_item() {
        let value = RedisRepo::parse_list_values(vec![
            "{\"a\":1}".to_string(),
            "plain".to_string(),
        ]);
        let arr = value.as_array().expect("array");
        assert!(arr[0].is_object());
        assert!(arr[1].is_string());
    }

    #[test]
    fn parse_hash_pairs_parses_values() {
        let value = RedisRepo::parse_hash_pairs(vec![
            ("a".to_string(), "{\"x\":1}".to_string()),
            ("b".to_string(), "plain".to_string()),
        ]);
        let obj = value.as_object().expect("object");
        assert!(obj.get("a").unwrap().is_object());
        assert!(obj.get("b").unwrap().is_string());
    }

    #[test]
    fn build_output_iothub_topics_combines_agent_id() {
        let (tx, _rx) = crossbeam_channel::unbounded();
        let repo = RedisRepo::new(&RedisConfig::default(), tx).expect("repo");

        let config_json = json!([
            {
                "guaranteeTopic": [
                    "persistent://goldwind/iothub/prop_data-BZ-GRID-realdev-Guarantee",
                    "non-persistent://goldwind/iothub/prop_data-BZ-FAST-realdev-Guarantee"
                ],
                "oneDataTopic": [
                    "persistent://goldwind/iothub/prop_data-BZ-GRID_SECTION-realdev-60"
                ],
                "tenDataTopic": [
                    "persistent://goldwind/iothub/prop_data-BZ-GRID_SECTION-realdev-600"
                ],
                "curveDataTopic": [
                    "persistent://goldwind/iothub/prop_data-BZ-GRID_SECTION-realdev-WindPower"
                ],
                "topicEvent": "persistent://goldwind/iothub/thing_event-BZ",
                "topicSvrReq": "persistent://goldwind/iothub/thing_service-BZ-REQUEST",
                "topicSvrResp": "persistent://goldwind/iothub/thing_service-BZ-RESPONSE"
            }
        ]);

        let agents = vec!["622".to_string()];
        let topic_agents = repo.build_output_iothub_topic_agents(&config_json, &agents, 1);
        assert_eq!(topic_agents.len(), 1);
        assert!(topic_agents[0].topics.len() >= 6);
        for topic in &topic_agents[0].topics {
            assert!(topic.path.contains("-622"));
        }
    }
}
