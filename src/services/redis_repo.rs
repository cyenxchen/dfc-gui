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
                // First check the key type
                let type_cmd = CustomCommand::new_static("TYPE", None, false);
                let type_result: Value = client.custom(type_cmd, vec![Value::from(key.clone())]).await.unwrap_or(Value::Null);
                let key_type = type_result.into_string().unwrap_or_default();

                let value: Option<String> = match key_type.as_str() {
                    "string" => {
                        // Get string value
                        let cmd = CustomCommand::new_static("GET", None, false);
                        let result: Value = client.custom(cmd, vec![Value::from(key.clone())]).await.unwrap_or(Value::Null);
                        result.into_string()
                    }
                    "hash" => {
                        // Get hash value and convert to JSON
                        let cmd = CustomCommand::new_static("HGETALL", None, false);
                        let result: Value = client.custom(cmd, vec![Value::from(key.clone())]).await.unwrap_or(Value::Null);
                        if let Value::Array(arr) = result {
                            // Convert pairs to JSON object
                            let mut map = serde_json::Map::new();
                            let mut iter = arr.into_iter();
                            while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                                if let (Some(key_str), Some(val_str)) = (k.into_string(), v.into_string()) {
                                    // Try to parse value as JSON, otherwise use as string
                                    let json_val = serde_json::from_str(&val_str)
                                        .unwrap_or(serde_json::Value::String(val_str));
                                    map.insert(key_str, json_val);
                                }
                            }
                            Some(serde_json::to_string(&map).unwrap_or_default())
                        } else {
                            None
                        }
                    }
                    "list" => {
                        // Get list value and convert to JSON array
                        let cmd = CustomCommand::new_static("LRANGE", None, false);
                        let result: Value = client.custom(cmd, vec![
                            Value::from(key.clone()),
                            Value::from("0"),
                            Value::from("-1"),  // Get all elements
                        ]).await.unwrap_or(Value::Null);
                        if let Value::Array(arr) = result {
                            let items: Vec<serde_json::Value> = arr
                                .into_iter()
                                .filter_map(|v| v.into_string())
                                .map(|s| {
                                    // Try to parse each item as JSON, otherwise use as string
                                    serde_json::from_str(&s)
                                        .unwrap_or(serde_json::Value::String(s))
                                })
                                .collect();
                            Some(serde_json::to_string(&items).unwrap_or_default())
                        } else {
                            None
                        }
                    }
                    _ => {
                        tracing::debug!("Skipping key {} with unsupported type: {}", key, key_type);
                        None
                    }
                };

                if let Some(val) = value {
                    // Parse the value as JSON to extract topic details
                    let details = self.parse_config_value(&val, group_id);

                    // Extract service URL from the key or value
                    let service_url = self.extract_service_url(&key, &val);

                    // Extract cfgid from the key (e.g., CMC_{DCC0001}_sg.og.output.iothub -> DCC0001)
                    let cfgid = self.extract_cfgid_from_key(&key);

                    // Fetch TopicAgentIds for this config
                    let topic_agents = if let Some(ref cfg) = cfgid {
                        self.fetch_topic_agent_ids_internal(client, cfg, group_id, &details).await
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
        }

        tracing::info!("Fetched {} config items from Redis", configs.len());
        Ok(configs)
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

                    let visibility = obj.get("visibility")
                        .or_else(|| obj.get("visible"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    if !path.is_empty() {
                        details.push(DetailItem {
                            index,
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

    /// Fetch TopicAgentIds from CMC_{cfgid}_sg.device key
    async fn fetch_topic_agent_ids_internal(
        &self,
        client: &FredClient,
        cfgid: &str,
        group_id: i32,
        details: &[DetailItem],
    ) -> Vec<TopicAgentItem> {
        // Build the device key: CMC_{cfgid}_sg.device
        let device_key = format!("CMC_{{{}}}_sg.device", cfgid);
        tracing::debug!("Fetching TopicAgentIds from key: {}", device_key);

        // First check the key type
        let type_cmd = CustomCommand::new_static("TYPE", None, false);
        let type_result: Value = client
            .custom(type_cmd, vec![Value::from(device_key.clone())])
            .await
            .unwrap_or(Value::Null);
        let key_type = type_result.into_string().unwrap_or_default();

        let value: Option<String> = match key_type.as_str() {
            "string" => {
                let cmd = CustomCommand::new_static("GET", None, false);
                let result: Value = client
                    .custom(cmd, vec![Value::from(device_key.clone())])
                    .await
                    .unwrap_or(Value::Null);
                result.into_string()
            }
            "list" => {
                // Handle list type
                let cmd = CustomCommand::new_static("LRANGE", None, false);
                let result: Value = client
                    .custom(cmd, vec![
                        Value::from(device_key.clone()),
                        Value::from("0"),
                        Value::from("-1"),
                    ])
                    .await
                    .unwrap_or(Value::Null);
                if let Value::Array(arr) = result {
                    let items: Vec<serde_json::Value> = arr
                        .into_iter()
                        .filter_map(|v| v.into_string())
                        .filter_map(|s| serde_json::from_str(&s).ok())
                        .collect();
                    Some(serde_json::to_string(&items).unwrap_or_default())
                } else {
                    None
                }
            }
            _ => {
                tracing::debug!("Device key {} has unsupported type: {}", device_key, key_type);
                None
            }
        };

        // Parse the JSON array to extract topicAgentIds
        let Some(value) = value else {
            tracing::debug!("No value found for device key: {}", device_key);
            return Vec::new();
        };

        let Ok(json) = serde_json::from_str::<serde_json::Value>(&value) else {
            tracing::debug!("Failed to parse device JSON: {}", value);
            return Vec::new();
        };

        let Some(devices_arr) = json.as_array() else {
            tracing::debug!("Device value is not an array");
            return Vec::new();
        };

        // Collect unique TopicAgentIds
        let mut agent_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for device in devices_arr {
            if let Some(agent_id) = device.get("topicAgentId").and_then(|v| v.as_str()) {
                if !agent_id.is_empty() {
                    agent_ids.insert(agent_id.to_string());
                }
            }
        }

        tracing::debug!("Found {} unique TopicAgentIds", agent_ids.len());

        // Create TopicAgentItem for each unique agent_id
        // Associate topics based on the agent_id pattern in the topic path
        let mut topic_agents: Vec<TopicAgentItem> = agent_ids
            .into_iter()
            .map(|agent_id| {
                // Filter topics that contain this agent_id in their path
                let topics: Vec<TopicDetail> = details
                    .iter()
                    .filter(|d| d.path.contains(&agent_id))
                    .map(|d| TopicDetail {
                        path: d.path.clone(),
                        topic_type: self.extract_topic_type(&d.path),
                    })
                    .collect();

                TopicAgentItem {
                    agent_id,
                    topics,
                    group_id,
                }
            })
            .collect();

        // If no topics matched by agent_id, create one TopicAgentItem with all topics
        // This handles the case where topics don't contain agent_id in their path
        if topic_agents.iter().all(|ta| ta.topics.is_empty()) && !topic_agents.is_empty() {
            // Assign all topics to the first agent
            let all_topics: Vec<TopicDetail> = details
                .iter()
                .map(|d| TopicDetail {
                    path: d.path.clone(),
                    topic_type: self.extract_topic_type(&d.path),
                })
                .collect();

            if let Some(first) = topic_agents.first_mut() {
                first.topics = all_topics;
            }
        }

        // Sort by agent_id for consistent ordering
        topic_agents.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));

        topic_agents
    }

    /// Extract topic type from path
    fn extract_topic_type(&self, path: &str) -> String {
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
