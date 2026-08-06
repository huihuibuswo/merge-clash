use crate::{
    database::validate_http_url,
    error::{AppError, AppResult},
    models::ConnectionTestResult,
};
use futures_util::StreamExt;
use reqwest::Client;
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

const MAX_RESPONSE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct FetchedConfig {
    pub text: String,
    pub proxies: Vec<Value>,
    pub proxy_types: Vec<String>,
    pub elapsed_ms: u64,
    pub hash: String,
}

pub fn client() -> AppResult<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("MergeClash/0.1 Mihomo")
        .build()
        .map_err(|e| AppError::Network(e.to_string()))
}

pub async fn fetch_config(client: &Client, url: &str) -> AppResult<FetchedConfig> {
    validate_http_url(url)?;
    let started = Instant::now();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Network(safe_network_error(&e)))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "订阅服务器返回 HTTP {}",
            status.as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::Network("订阅响应超过 20 MiB 限制".into()));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Network(safe_network_error(&e)))?;
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::Network("订阅响应超过 20 MiB 限制".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    let text =
        String::from_utf8(bytes).map_err(|_| AppError::Config("订阅内容不是有效 UTF-8".into()))?;
    let root: Value = serde_yaml::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| AppError::Config(format!("YAML 解析失败: {e}")))?;
    let mapping = root
        .as_mapping()
        .ok_or_else(|| AppError::Config("YAML 顶层必须是对象".into()))?;
    let proxies = mapping
        .get(Value::String("proxies".into()))
        .and_then(Value::as_sequence)
        .cloned()
        .ok_or_else(|| AppError::Config("配置中缺少 proxies 列表".into()))?;
    let valid: Vec<Value> = proxies
        .into_iter()
        .filter(|value| proxy_name(value).is_some())
        .collect();
    if valid.is_empty() {
        return Err(AppError::Config("没有识别到有效节点".into()));
    }
    let mut proxy_types = valid.iter().filter_map(proxy_type).collect::<Vec<_>>();
    proxy_types.sort();
    proxy_types.dedup();
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    Ok(FetchedConfig {
        text,
        proxies: valid,
        proxy_types,
        elapsed_ms: started.elapsed().as_millis() as u64,
        hash,
    })
}

pub async fn test_url(client: &Client, url: &str) -> ConnectionTestResult {
    let started = Instant::now();
    if let Err(error) = validate_http_url(url) {
        return failed("url", started, error.to_string());
    }
    match fetch_config(client, url).await {
        Ok(config) => ConnectionTestResult {
            reachable: true,
            stage: "complete".into(),
            http_status: Some(200),
            elapsed_ms: config.elapsed_ms,
            response_bytes: Some(config.text.len() as u64),
            proxy_count: Some(config.proxies.len()),
            proxy_types: config.proxy_types,
            warnings: vec![],
            error: None,
        },
        Err(AppError::Network(message)) => failed("network", started, message),
        Err(AppError::Config(message)) => failed("yaml", started, message),
        Err(error) => failed("http", started, error.to_string()),
    }
}

fn failed(stage: &str, started: Instant, error: String) -> ConnectionTestResult {
    ConnectionTestResult {
        reachable: false,
        stage: stage.into(),
        http_status: None,
        elapsed_ms: started.elapsed().as_millis() as u64,
        response_bytes: None,
        proxy_count: None,
        proxy_types: vec![],
        warnings: vec![],
        error: Some(error),
    }
}

pub fn proxy_name(value: &Value) -> Option<String> {
    value
        .as_mapping()?
        .get(Value::String("name".into()))?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub fn proxy_type(value: &Value) -> Option<String> {
    value
        .as_mapping()?
        .get(Value::String("type".into()))?
        .as_str()
        .map(str::to_owned)
}

fn safe_network_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "请求超时，请检查网络或订阅服务".into()
    } else if error.is_connect() {
        "无法连接订阅服务器".into()
    } else if error.is_redirect() {
        "订阅重定向次数过多或目标无效".into()
    } else {
        "订阅请求失败".into()
    }
}
