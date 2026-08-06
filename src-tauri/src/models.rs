use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSummary {
    pub id: String,
    pub version: i64,
    pub name: String,
    pub description: String,
    pub core: String,
    pub output_format: String,
    pub file_name: String,
    pub supported_modes: Vec<String>,
    pub default_mode: String,
    pub groups: Vec<String>,
    pub external_dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    pub template_id: String,
    pub template_version: i64,
    pub merge_mode: String,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url_masked: String,
    pub enabled: bool,
    pub priority: i64,
    pub last_status: String,
    pub last_error: Option<String>,
    pub last_fetched_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_tested_at: Option<i64>,
    pub proxy_count: i64,
    pub elapsed_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct InternalSubscription {
    pub safe: Subscription,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionInput {
    pub id: Option<String>,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub priority: Option<i64>,
    pub test_result: Option<ConnectionTestResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub reachable: bool,
    pub stage: String,
    pub http_status: Option<u16>,
    pub elapsed_ms: u64,
    pub response_bytes: Option<u64>,
    pub proxy_count: Option<usize>,
    pub proxy_types: Vec<String>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub source_id: String,
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyGroup {
    pub name: String,
    pub group_type: String,
    pub members: Vec<String>,
    pub filter: Option<String>,
    pub exclude_filter: Option<String>,
    pub url: Option<String>,
    pub interval: Option<i64>,
    pub tolerance: Option<i64>,
    pub lazy: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub revision: i64,
    pub template_id: String,
    pub template_version: i64,
    pub merge_mode: String,
    pub proxies: Vec<ProxyNode>,
    pub groups: Vec<ProxyGroup>,
    pub yaml: String,
    pub issues: Vec<ValidationIssue>,
    pub source_failures: Vec<String>,
    pub updated_at: i64,
    pub published_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProxy {
    pub meta: ProxyNode,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDraft {
    pub draft: Draft,
    pub stored_proxies: Vec<StoredProxy>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResult {
    pub draft: Draft,
    pub successful: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishStatus {
    pub running: bool,
    pub port: u16,
    pub bind_address: String,
    pub lan_addresses: Vec<String>,
    pub proxy_detected: bool,
    pub subscription_url: Option<String>,
    pub last_published_at: Option<i64>,
    pub version_no: Option<i64>,
    pub content_hash: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftHistory {
    pub id: i64,
    pub revision: i64,
    pub action: String,
    pub node_count: i64,
    pub group_count: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedVersion {
    pub version_no: i64,
    pub template_id: String,
    pub template_version: i64,
    pub merge_mode: String,
    pub content_hash: String,
    pub created_at: i64,
    pub active: bool,
}
