use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub country: String,
    pub country_short: String,
    pub host_name: String,
    pub ip: String,
    pub remote_host: String,
    pub remote_port: u16,
    pub proto: String,
    pub score: i64,
    pub ping: i64,
    pub speed: i64,
    pub sessions: i64,
    pub config_text: String,
    pub latency_ms: Option<u128>,
    pub status: NodeStatus,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    New,
    Available,
    Failed,
    Active,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub active_node_id: Option<String>,
    pub connecting: bool,
    pub relay_addr: String,
    pub control_addr: String,
    pub last_message: String,
    pub last_refresh_at: Option<i64>,
    pub node_count: usize,
    pub auth_enabled: bool,
}

impl RuntimeStatus {
    pub fn new(relay_addr: String, control_addr: String, auth_enabled: bool) -> Self {
        Self {
            active_node_id: None,
            connecting: false,
            relay_addr,
            control_addr,
            last_message: "starting".to_string(),
            last_refresh_at: None,
            node_count: 0,
            auth_enabled,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatewaySettings {
    pub connection_enabled: bool,
    pub route_mode: RouteMode,
    pub country: String,
    pub fixed_node_id: String,
    pub favorite_node_ids: BTreeSet<String>,
    pub fallback_to_any: bool,
}

impl Default for GatewaySettings {
    fn default() -> Self {
        Self {
            connection_enabled: true,
            route_mode: RouteMode::Auto,
            country: String::new(),
            fixed_node_id: String::new(),
            favorite_node_ids: BTreeSet::new(),
            fallback_to_any: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    Auto,
    FixedCountry,
    FixedNode,
    Favorites,
}

#[derive(Debug, Deserialize)]
pub struct FavoriteRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: i64,
    pub level: String,
    pub module: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeTestResult {
    pub id: String,
    pub ok: bool,
    pub latency_ms: Option<u128>,
    pub message: String,
}
