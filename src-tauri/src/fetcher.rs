use crate::{
    database::validate_http_url,
    error::{AppError, AppResult},
    models::{AvailableProxyNode, ConnectionTestResult},
};
use futures_util::{stream, StreamExt};
use reqwest::Client;
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};
use tauri::Manager;
use tokio::{process::Command, time::sleep};
use url::Url;
use uuid::Uuid;

const MAX_RESPONSE_BYTES: usize = 20 * 1024 * 1024;
const MAX_TESTED_NODES: usize = 300;
const NODE_TEST_CONCURRENCY: usize = 16;
const NODE_TEST_TIMEOUT_MS: u64 = 5_000;
const MIHOMO_START_TIMEOUT: Duration = Duration::from_secs(10);
const NODE_TEST_URL: &str = "https://www.gstatic.com/generate_204";

#[derive(Debug, Clone)]
pub struct FetchedConfig {
    pub text: String,
    pub proxies: Vec<Value>,
    pub proxy_types: Vec<String>,
    pub elapsed_ms: u64,
    pub hash: String,
}

pub struct TestedConfig {
    pub fetched: FetchedConfig,
    pub total_proxy_count: usize,
    pub available_nodes: Vec<AvailableProxyNode>,
    pub warnings: Vec<String>,
}

type TestedProxy = (usize, Value, AvailableProxyNode);

#[derive(Debug)]
struct MihomoCandidate {
    index: usize,
    proxy: Value,
    test_proxy: Value,
    original_name: String,
    proxy_type: String,
    internal_name: String,
}

pub fn mihomo_path(app: &tauri::App) -> AppResult<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("MERGE_CLASH_MIHOMO_PATH").filter(|path| !path.is_empty()) {
        candidates.push(PathBuf::from(path));
    }
    if cfg!(debug_assertions) {
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("bin")
                .join("mihomo-x86_64-pc-windows-msvc.exe"),
        );
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("mihomo.exe"));
        }
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("mihomo.exe"));
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            AppError::Operation("缺少 Mihomo 核心，无法执行真实节点测试；请重新安装完整版本".into())
        })
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

pub async fn test_url(client: &Client, mihomo_path: &Path, url: &str) -> ConnectionTestResult {
    let started = Instant::now();
    if let Err(error) = validate_http_url(url) {
        return failed("url", started, error.to_string());
    }
    match fetch_tested_config(client, mihomo_path, url).await {
        Ok(config) => {
            let available_proxy_count = config.available_nodes.len();
            ConnectionTestResult {
                reachable: available_proxy_count > 0,
                stage: if available_proxy_count > 0 {
                    "complete".into()
                } else {
                    "nodes".into()
                },
                http_status: Some(200),
                elapsed_ms: started.elapsed().as_millis() as u64,
                response_bytes: Some(config.fetched.text.len() as u64),
                proxy_count: Some(config.total_proxy_count),
                available_proxy_count,
                available_nodes: config.available_nodes,
                proxy_types: config.fetched.proxy_types,
                warnings: config.warnings,
                error: if available_proxy_count > 0 {
                    None
                } else {
                    Some(format!(
                        "识别到 {} 个节点，但没有节点通过真实代理请求测试",
                        config.total_proxy_count
                    ))
                },
            }
        }
        Err(AppError::Network(message)) => failed("network", started, message),
        Err(AppError::Config(message)) => failed("yaml", started, message),
        Err(error) => failed("nodes", started, error.to_string()),
    }
}

pub async fn fetch_tested_config(
    client: &Client,
    mihomo_path: &Path,
    url: &str,
) -> AppResult<TestedConfig> {
    let mut fetched = fetch_config(client, url).await?;
    let total_proxy_count = fetched.proxies.len();
    let tested_count = total_proxy_count.min(MAX_TESTED_NODES);
    let candidates = fetched
        .proxies
        .iter()
        .take(tested_count)
        .cloned()
        .enumerate()
        .collect::<Vec<_>>();
    let node_test_started = Instant::now();
    let mut results = test_proxy_candidates(mihomo_path, candidates).await?;
    let node_test_elapsed_ms = node_test_started.elapsed().as_millis() as u64;
    results.sort_by_key(|(index, _, _)| *index);

    let mut warnings = Vec::new();
    if total_proxy_count > tested_count {
        warnings.push(format!(
            "节点数量超过测试上限，仅测试前 {MAX_TESTED_NODES} 个节点"
        ));
    }
    let available_nodes = results
        .iter()
        .map(|(_, _, node)| node.clone())
        .collect::<Vec<_>>();
    let unavailable_count = tested_count.saturating_sub(available_nodes.len());
    if unavailable_count > 0 {
        warnings.push(format!("{unavailable_count} 个节点未通过真实代理请求测试"));
    }
    fetched.proxies = results.into_iter().map(|(_, proxy, _)| proxy).collect();
    fetched.proxy_types = fetched
        .proxies
        .iter()
        .filter_map(proxy_type)
        .collect::<Vec<_>>();
    fetched.proxy_types.sort();
    fetched.proxy_types.dedup();
    fetched.elapsed_ms = fetched.elapsed_ms.saturating_add(node_test_elapsed_ms);

    Ok(TestedConfig {
        fetched,
        total_proxy_count,
        available_nodes,
        warnings,
    })
}

async fn test_proxy_candidates(
    mihomo_path: &Path,
    candidates: Vec<(usize, Value)>,
) -> AppResult<Vec<TestedProxy>> {
    let test_id = Uuid::new_v4().simple().to_string();
    let test_dir = env::temp_dir().join(format!("merge-clash-node-test-{test_id}"));
    fs::create_dir_all(&test_dir)
        .map_err(|e| AppError::Operation(format!("无法创建节点测试目录: {e}")))?;

    let result = test_candidate_batches(mihomo_path, &test_dir, candidates).await;
    let _ = fs::remove_dir_all(&test_dir);
    result
}

async fn test_candidate_batches(
    mihomo_path: &Path,
    test_dir: &Path,
    candidates: Vec<(usize, Value)>,
) -> AppResult<Vec<TestedProxy>> {
    let mut pending = vec![candidates];
    let mut tested = Vec::new();
    while let Some(mut batch) = pending.pop() {
        match run_mihomo_test(mihomo_path, test_dir, batch.clone()).await {
            Ok(mut results) => tested.append(&mut results),
            Err(AppError::Config(_)) if batch.len() > 1 => {
                let right = batch.split_off(batch.len() / 2);
                pending.push(right);
                pending.push(batch);
            }
            Err(AppError::Config(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(tested)
}

async fn run_mihomo_test(
    mihomo_path: &Path,
    test_dir: &Path,
    candidates: Vec<(usize, Value)>,
) -> AppResult<Vec<TestedProxy>> {
    let controller_port = reserve_local_port()?;
    let secret = Uuid::new_v4().simple().to_string();
    let prepared = prepare_candidates(candidates);
    let config_path = test_dir.join("config.yaml");
    let log_path = test_dir.join("mihomo.log");
    fs::write(
        &config_path,
        mihomo_config(controller_port, &secret, &prepared)?,
    )
    .map_err(|e| AppError::Operation(format!("无法写入节点测试配置: {e}")))?;

    let log = fs::File::create(&log_path)
        .map_err(|e| AppError::Operation(format!("无法创建 Mihomo 日志: {e}")))?;
    let stdout = log
        .try_clone()
        .map_err(|e| AppError::Operation(format!("无法创建 Mihomo 日志句柄: {e}")))?;
    let mut command = Command::new(mihomo_path);
    command
        .arg("-d")
        .arg(test_dir)
        .arg("-f")
        .arg(&config_path)
        .current_dir(test_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(log));
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .spawn()
        .map_err(|e| AppError::Operation(format!("Mihomo 核心启动失败: {e}")))?;

    let controller = format!("http://127.0.0.1:{controller_port}");
    let controller_client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| AppError::Operation(format!("节点测试客户端创建失败: {e}")))?;
    let test_result = async {
        wait_for_mihomo(&controller_client, &controller, &secret, &mut child).await?;
        Ok::<_, AppError>(
            stream::iter(prepared)
                .map(|candidate| {
                    test_mihomo_candidate(
                        controller_client.clone(),
                        controller.clone(),
                        secret.clone(),
                        candidate,
                    )
                })
                .buffer_unordered(NODE_TEST_CONCURRENCY)
                .filter_map(|result| async move { result })
                .collect::<Vec<_>>()
                .await,
        )
    }
    .await;

    let _ = child.kill().await;
    let _ = child.wait().await;
    test_result
}

fn prepare_candidates(candidates: Vec<(usize, Value)>) -> Vec<MihomoCandidate> {
    let mut prepared = candidates
        .into_iter()
        .filter_map(|(index, proxy)| {
            Some(MihomoCandidate {
                index,
                original_name: proxy_name(&proxy)?,
                proxy_type: proxy_type(&proxy).unwrap_or_else(|| "unknown".into()),
                internal_name: format!("__merge_clash_test_{index:04}"),
                test_proxy: proxy.clone(),
                proxy,
            })
        })
        .collect::<Vec<_>>();
    let name_map = prepared
        .iter()
        .map(|candidate| {
            (
                candidate.original_name.clone(),
                candidate.internal_name.clone(),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    for candidate in &mut prepared {
        if let Some(mapping) = candidate.test_proxy.as_mapping_mut() {
            mapping.insert(
                Value::String("name".into()),
                Value::String(candidate.internal_name.clone()),
            );
            let dialer_key = Value::String("dialer-proxy".into());
            if let Some(original) = mapping.get(&dialer_key).and_then(Value::as_str) {
                if let Some(internal) = name_map.get(original) {
                    mapping.insert(dialer_key, Value::String(internal.clone()));
                }
            }
        }
    }
    prepared
}

fn mihomo_config(port: u16, secret: &str, candidates: &[MihomoCandidate]) -> AppResult<String> {
    let mut root = Mapping::new();
    root.insert(
        Value::String("log-level".into()),
        Value::String("silent".into()),
    );
    root.insert(Value::String("mode".into()), Value::String("global".into()));
    root.insert(
        Value::String("external-controller".into()),
        Value::String(format!("127.0.0.1:{port}")),
    );
    root.insert(
        Value::String("secret".into()),
        Value::String(secret.to_owned()),
    );
    root.insert(
        Value::String("proxies".into()),
        Value::Sequence(
            candidates
                .iter()
                .map(|candidate| candidate.test_proxy.clone())
                .collect(),
        ),
    );
    serde_yaml::to_string(&Value::Mapping(root))
        .map_err(|e| AppError::Config(format!("节点测试配置生成失败: {e}")))
}

fn reserve_local_port() -> AppResult<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| AppError::Operation(format!("无法分配 Mihomo 控制端口: {e}")))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|e| AppError::Operation(format!("无法读取 Mihomo 控制端口: {e}")))
}

async fn wait_for_mihomo(
    client: &Client,
    controller: &str,
    secret: &str,
    child: &mut tokio::process::Child,
) -> AppResult<()> {
    let started = Instant::now();
    while started.elapsed() < MIHOMO_START_TIMEOUT {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| AppError::Operation(format!("无法读取 Mihomo 运行状态: {e}")))?
        {
            return Err(AppError::Config(format!(
                "Mihomo 无法加载节点配置（退出码 {}）",
                status
                    .code()
                    .map_or_else(|| "未知".into(), |code| code.to_string())
            )));
        }
        if client
            .get(format!("{controller}/version"))
            .bearer_auth(secret)
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(());
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err(AppError::Operation("Mihomo 核心启动超时".into()))
}

async fn test_mihomo_candidate(
    client: Client,
    controller: String,
    secret: String,
    candidate: MihomoCandidate,
) -> Option<TestedProxy> {
    let mut url = Url::parse(&controller).ok()?;
    url.path_segments_mut()
        .ok()?
        .clear()
        .push("proxies")
        .push(&candidate.internal_name)
        .push("delay");
    url.query_pairs_mut()
        .append_pair("url", NODE_TEST_URL)
        .append_pair("timeout", &NODE_TEST_TIMEOUT_MS.to_string());
    let response = client.get(url).bearer_auth(secret).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.text().await.ok()?;
    let delay = serde_json::from_str::<serde_json::Value>(&body)
        .ok()?
        .get("delay")?
        .as_u64()?;
    (delay > 0).then(|| {
        (
            candidate.index,
            candidate.proxy,
            AvailableProxyNode {
                index: candidate.index,
                name: candidate.original_name,
                proxy_type: candidate.proxy_type,
                elapsed_ms: delay,
            },
        )
    })
}

fn failed(stage: &str, started: Instant, error: String) -> ConnectionTestResult {
    ConnectionTestResult {
        reachable: false,
        stage: stage.into(),
        http_status: None,
        elapsed_ms: started.elapsed().as_millis() as u64,
        response_bytes: None,
        proxy_count: None,
        available_proxy_count: 0,
        available_nodes: vec![],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_unique_names_and_rewrites_dialer_proxy() {
        let candidates = vec![
            (0, serde_yaml::from_str("name: base\ntype: direct").unwrap()),
            (
                1,
                serde_yaml::from_str("name: child\ntype: direct\ndialer-proxy: base").unwrap(),
            ),
        ];
        let prepared = prepare_candidates(candidates);

        assert_eq!(
            proxy_name(&prepared[0].test_proxy).as_deref(),
            Some("__merge_clash_test_0000")
        );
        assert_eq!(proxy_name(&prepared[0].proxy).as_deref(), Some("base"));
        assert_eq!(
            prepared[1]
                .test_proxy
                .as_mapping()
                .unwrap()
                .get(Value::String("dialer-proxy".into()))
                .and_then(Value::as_str),
            Some("__merge_clash_test_0000")
        );
    }

    #[test]
    fn test_config_uses_controller_and_internal_proxy_names() {
        let candidates = prepare_candidates(vec![(
            3,
            serde_yaml::from_str("name: node\ntype: direct").unwrap(),
        )]);
        let yaml = mihomo_config(19090, "secret", &candidates).unwrap();
        let root: Value = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(
            root.get("external-controller").and_then(Value::as_str),
            Some("127.0.0.1:19090")
        );
        assert!(yaml.contains("__merge_clash_test_0003"));
    }
}
