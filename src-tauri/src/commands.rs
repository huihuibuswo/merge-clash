use crate::{
    database::Database,
    error::{AppError, AppResult},
    fetcher,
    merge::{self, SourceConfig},
    models::*,
    publisher::{self, PublishedContent, ServerHandle},
    templates,
};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub http: reqwest::Client,
    pub mihomo_path: std::path::PathBuf,
    pub publisher: Arc<Mutex<Option<ServerHandle>>>,
}

#[tauri::command]
pub fn list_templates() -> Vec<TemplateSummary> {
    templates::list()
}

#[tauri::command]
pub async fn get_project_settings(state: State<'_, AppState>) -> AppResult<ProjectSettings> {
    state.db.settings().await
}

#[tauri::command]
pub async fn select_project_template(
    state: State<'_, AppState>,
    template_id: String,
    template_version: i64,
    merge_mode: String,
) -> AppResult<ProjectSettings> {
    let template = templates::list()
        .into_iter()
        .find(|item| item.id == template_id)
        .ok_or_else(|| AppError::InvalidInput("模板不存在".into()))?;
    if template.version != template_version || !template.supported_modes.contains(&merge_mode) {
        return Err(AppError::InvalidInput("模板版本或合并模式不受支持".into()));
    }
    state
        .db
        .select_template(&template_id, template_version, &merge_mode)
        .await
}

#[tauri::command]
pub async fn save_theme(state: State<'_, AppState>, theme: String) -> AppResult<ProjectSettings> {
    if !["system", "light", "dark"].contains(&theme.as_str()) {
        return Err(AppError::InvalidInput("主题值无效".into()));
    }
    state.db.save_theme(&theme).await
}

#[tauri::command]
pub async fn list_subscriptions(state: State<'_, AppState>) -> AppResult<Vec<Subscription>> {
    state.db.list_subscriptions().await
}

#[tauri::command]
pub async fn save_subscription(
    state: State<'_, AppState>,
    input: SubscriptionInput,
) -> AppResult<Subscription> {
    let test_result = input.test_result.clone();
    let saved = state.db.save_subscription(input).await?;
    if let Some(result) = test_result {
        state
            .db
            .mark_fetch(
                &saved.id,
                if result.reachable { "success" } else { "error" },
                result.error.as_deref(),
                result.available_proxy_count as i64,
                result.elapsed_ms as i64,
                result.reachable,
            )
            .await?;
        return state
            .db
            .list_subscriptions()
            .await?
            .into_iter()
            .find(|item| item.id == saved.id)
            .ok_or_else(|| AppError::Operation("保存测试结果后无法读取订阅".into()));
    }
    Ok(saved)
}

#[tauri::command]
pub async fn delete_subscription(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.delete_subscription(&id).await
}

#[tauri::command]
pub async fn test_subscription_url(
    state: State<'_, AppState>,
    url: Option<String>,
    id: Option<String>,
) -> AppResult<ConnectionTestResult> {
    let candidate = url.unwrap_or_default();
    if !candidate.trim().is_empty() {
        return Ok(fetcher::test_url(&state.http, &state.mihomo_path, candidate.trim()).await);
    }
    let id = id.ok_or_else(|| AppError::InvalidInput("请输入订阅地址".into()))?;
    let subscription = state
        .db
        .list_internal_subscriptions()
        .await?
        .into_iter()
        .find(|item| item.safe.id == id)
        .ok_or_else(|| AppError::InvalidInput("订阅不存在或已删除".into()))?;
    Ok(fetcher::test_url(&state.http, &state.mihomo_path, &subscription.url).await)
}

#[tauri::command]
pub async fn test_subscription(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<ConnectionTestResult> {
    let subscription = state
        .db
        .list_internal_subscriptions()
        .await?
        .into_iter()
        .find(|item| item.safe.id == id)
        .ok_or_else(|| AppError::InvalidInput("订阅不存在或已删除".into()))?;
    let result = fetcher::test_url(&state.http, &state.mihomo_path, &subscription.url).await;
    state
        .db
        .mark_fetch(
            &subscription.safe.id,
            if result.reachable { "success" } else { "error" },
            result.error.as_deref(),
            result.available_proxy_count as i64,
            result.elapsed_ms as i64,
            result.reachable,
        )
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn refresh_subscriptions(state: State<'_, AppState>) -> AppResult<RefreshResult> {
    let all = state.db.list_internal_subscriptions().await?;
    let enabled: Vec<_> = all.into_iter().filter(|item| item.safe.enabled).collect();
    if enabled.is_empty() {
        return Err(AppError::InvalidInput("没有启用的订阅".into()));
    }
    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for subscription in enabled {
        match fetcher::fetch_tested_config(&state.http, &state.mihomo_path, &subscription.url).await
        {
            Ok(tested) if !tested.available_nodes.is_empty() => {
                let fetched = tested.fetched;
                state
                    .db
                    .mark_fetch(
                        &subscription.safe.id,
                        "success",
                        None,
                        fetched.proxies.len() as i64,
                        fetched.elapsed_ms as i64,
                        true,
                    )
                    .await?;
                state
                    .db
                    .save_snapshot(
                        &subscription.safe.id,
                        &fetched.text,
                        &fetched.hash,
                        fetched.proxies.len() as i64,
                    )
                    .await?;
                successes.push(SourceConfig {
                    subscription,
                    fetched,
                });
            }
            Ok(tested) => {
                let safe_error = format!(
                    "识别到 {} 个节点，但没有节点通过真实代理请求测试",
                    tested.total_proxy_count
                );
                state
                    .db
                    .mark_fetch(
                        &subscription.safe.id,
                        "error",
                        Some(&safe_error),
                        0,
                        tested.fetched.elapsed_ms as i64,
                        false,
                    )
                    .await?;
                failures.push(format!("{}：{}", subscription.safe.name, safe_error));
            }
            Err(error) => {
                let safe_error = error.to_string();
                state
                    .db
                    .mark_fetch(
                        &subscription.safe.id,
                        "error",
                        Some(&safe_error),
                        0,
                        0,
                        false,
                    )
                    .await?;
                failures.push(format!("{}：{}", subscription.safe.name, safe_error));
            }
        }
    }
    if successes.is_empty() {
        return Err(AppError::Operation(
            "所有订阅刷新失败，现有草稿和发布版本未改变".into(),
        ));
    }
    let settings = state.db.settings().await?;
    let previous = state.db.stored_draft().await.ok();
    let stored = merge::build(&settings, &successes, previous.as_ref(), failures.clone())?;
    state.db.save_stored_draft(&stored).await?;
    state.db.save_draft_history(&stored, "刷新订阅").await?;
    Ok(RefreshResult {
        draft: stored.draft,
        successful: successes.len(),
        failed: failures.len(),
    })
}

#[tauri::command]
pub async fn get_draft(state: State<'_, AppState>) -> AppResult<Draft> {
    Ok(state.db.stored_draft().await?.draft)
}

#[tauri::command]
pub async fn save_proxy_groups(
    state: State<'_, AppState>,
    revision: i64,
    groups: Vec<ProxyGroup>,
) -> AppResult<Draft> {
    let mut stored = state.db.stored_draft().await?;
    if stored.draft.revision != revision {
        return Err(AppError::Operation(
            "草稿已被其他操作更新，请刷新页面后重试".into(),
        ));
    }
    stored.draft.groups = groups;
    let settings = state.db.settings().await?;
    let subscriptions = state
        .db
        .list_internal_subscriptions()
        .await?
        .into_iter()
        .filter(|item| item.safe.enabled)
        .collect::<Vec<_>>();
    merge::rerender(&settings, &mut stored, &subscriptions)?;
    state.db.save_stored_draft(&stored).await?;
    state.db.save_draft_history(&stored, "保存分组").await?;
    Ok(stored.draft)
}

#[tauri::command]
pub async fn save_draft_yaml(
    state: State<'_, AppState>,
    revision: i64,
    yaml: String,
) -> AppResult<Draft> {
    let mut stored = state.db.stored_draft().await?;
    if stored.draft.revision != revision {
        return Err(AppError::Operation(
            "草稿已被其他操作更新，请刷新页面后重试".into(),
        ));
    }
    if stored.draft.template_id == "clash-mihomo" {
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml)
            .map_err(|error| AppError::InvalidInput(format!("YAML 语法错误: {error}")))?;
        if !parsed.is_mapping() {
            return Err(AppError::InvalidInput("YAML 顶层必须是对象".into()));
        }
    } else {
        if yaml.trim().is_empty() {
            return Err(AppError::InvalidInput("订阅内容不能为空".into()));
        }
        base64::engine::general_purpose::STANDARD
            .decode(yaml.trim())
            .map_err(|_| AppError::InvalidInput("订阅内容必须是有效的 Base64 文本".into()))?;
    }
    stored.draft.yaml = yaml;
    stored.draft.revision += 1;
    stored.draft.updated_at = crate::database::now_ms();
    state.db.save_stored_draft(&stored).await?;
    state.db.save_draft_history(&stored, "保存配置内容").await?;
    Ok(stored.draft)
}

#[tauri::command]
pub async fn list_draft_history(state: State<'_, AppState>) -> AppResult<Vec<DraftHistory>> {
    state.db.list_draft_history().await
}

#[tauri::command]
pub async fn restore_draft_history(state: State<'_, AppState>, id: i64) -> AppResult<Draft> {
    let current = state.db.stored_draft().await?;
    let mut restored = state.db.draft_history_snapshot(id).await?;
    let settings = state.db.settings().await?;
    if restored.draft.template_id != settings.template_id
        || restored.draft.template_version != settings.template_version
        || restored.draft.merge_mode != settings.merge_mode
    {
        return Err(AppError::Operation(
            "该历史记录属于其他模板或合并模式，无法恢复".into(),
        ));
    }
    restored.draft.revision = current.draft.revision + 1;
    restored.draft.updated_at = crate::database::now_ms();
    restored.draft.published_at = current.draft.published_at;
    state.db.save_stored_draft(&restored).await?;
    state.db.save_draft_history(&restored, "恢复历史").await?;
    Ok(restored.draft)
}

#[tauri::command]
pub async fn delete_draft_history(
    state: State<'_, AppState>,
    id: i64,
) -> AppResult<Vec<DraftHistory>> {
    let current = state.db.stored_draft().await?;
    state
        .db
        .delete_draft_history(id, current.draft.revision)
        .await?;
    state.db.list_draft_history().await
}

#[tauri::command]
pub async fn delete_other_draft_history(
    state: State<'_, AppState>,
) -> AppResult<Vec<DraftHistory>> {
    let current = state.db.stored_draft().await?;
    state
        .db
        .delete_other_draft_history(current.draft.revision)
        .await?;
    state.db.list_draft_history().await
}

#[tauri::command]
pub async fn publish_draft(state: State<'_, AppState>) -> AppResult<PublishStatus> {
    let mut stored = state.db.stored_draft().await?;
    let settings = state.db.settings().await?;
    if merge::sanitize_provider_group_members(&mut stored.draft.groups, &settings.merge_mode) {
        let subscriptions = state
            .db
            .list_internal_subscriptions()
            .await?
            .into_iter()
            .filter(|item| item.safe.enabled)
            .collect::<Vec<_>>();
        merge::rerender(&settings, &mut stored, &subscriptions)?;
        state.db.save_stored_draft(&stored).await?;
    }
    if stored
        .draft
        .issues
        .iter()
        .any(|issue| issue.severity == "blocker")
    {
        return Err(AppError::Operation("草稿存在阻断问题，不能发布".into()));
    }
    let hash = format!("{:x}", Sha256::digest(stored.draft.yaml.as_bytes()));
    state.db.publish(&stored, &hash).await?;
    let (_port, token, _, template_id, body, _, _) = state.db.publish_record().await?;
    if let (Some(body), Some(handle)) = (body, state.publisher.lock().await.as_ref()) {
        let template_id = template_id.unwrap_or_else(|| "clash-mihomo".into());
        *handle.content.write().await = published_content(token, body, hash, &template_id);
    }
    publish_status(&state).await
}

#[tauri::command]
pub async fn list_published_versions(
    state: State<'_, AppState>,
) -> AppResult<Vec<PublishedVersion>> {
    state.db.list_published_versions().await
}

#[tauri::command]
pub async fn activate_published_version(
    state: State<'_, AppState>,
    version_no: i64,
) -> AppResult<PublishStatus> {
    if let Some(handle) = state.publisher.lock().await.take() {
        let _ = handle.shutdown.send(());
    }
    state.db.activate_published_version(version_no).await?;
    publish_status(&state).await
}

#[tauri::command]
pub async fn delete_published_version(
    state: State<'_, AppState>,
    version_no: i64,
) -> AppResult<PublishStatus> {
    state.db.delete_published_version(version_no).await?;
    sync_publisher_content(&state).await?;
    publish_status(&state).await
}

#[tauri::command]
pub async fn delete_other_published_versions(
    state: State<'_, AppState>,
) -> AppResult<Vec<PublishedVersion>> {
    state.db.delete_other_published_versions().await?;
    state.db.list_published_versions().await
}

#[tauri::command]
pub async fn get_publish_status(state: State<'_, AppState>) -> AppResult<PublishStatus> {
    publish_status(&state).await
}

#[tauri::command]
pub async fn start_publish_server(state: State<'_, AppState>) -> AppResult<PublishStatus> {
    if state.publisher.lock().await.is_some() {
        return publish_status(&state).await;
    }
    let network = publisher::network_environment();
    if network.lan_addresses.is_empty() {
        return Err(AppError::Operation(
            "未检测到可用的真实局域网 IPv4；如果已开启代理或 VPN，请确认 Wi-Fi/以太网已连接".into(),
        ));
    }
    let (port, token, _, template_id, body, hash, _) = state.db.publish_record().await?;
    let body = body.ok_or_else(|| AppError::Operation("尚未发布有效草稿".into()))?;
    let template_id = template_id.unwrap_or_else(|| "clash-mihomo".into());
    let handle = publisher::start(
        port,
        published_content(token, body, hash.unwrap_or_default(), &template_id),
    )
    .await?;
    *state.publisher.lock().await = Some(handle);
    publish_status(&state).await
}

#[tauri::command]
pub async fn stop_publish_server(state: State<'_, AppState>) -> AppResult<PublishStatus> {
    if let Some(handle) = state.publisher.lock().await.take() {
        let _ = handle.shutdown.send(());
    }
    publish_status(&state).await
}

#[tauri::command]
pub async fn save_publish_settings(
    state: State<'_, AppState>,
    port: u16,
) -> AppResult<PublishStatus> {
    if port < 1024 {
        return Err(AppError::InvalidInput("端口必须在 1024-65535 之间".into()));
    }
    if state.publisher.lock().await.is_some() {
        return Err(AppError::Operation("请先停止局域网服务再修改端口".into()));
    }
    state.db.set_port(port).await?;
    publish_status(&state).await
}

#[tauri::command]
pub async fn rotate_publish_token(state: State<'_, AppState>) -> AppResult<PublishStatus> {
    let token = publisher::new_token();
    state.db.rotate_token(&token).await?;
    if let Some(handle) = state.publisher.lock().await.as_ref() {
        handle.content.write().await.token = token;
    }
    publish_status(&state).await
}

async fn publish_status(state: &AppState) -> AppResult<PublishStatus> {
    let (port, token, version, _template_id, _body, hash, published_at) =
        state.db.publish_record().await?;
    let running = state.publisher.lock().await.is_some();
    let network = publisher::network_environment();
    let addresses = network.lan_addresses;
    let subscription_url = if running {
        addresses
            .first()
            .map(|ip| format!("http://{ip}:{port}/subscription/{token}/config"))
    } else {
        None
    };
    Ok(PublishStatus {
        running,
        port,
        bind_address: "0.0.0.0".into(),
        lan_addresses: addresses,
        proxy_detected: network.proxy_detected,
        subscription_url,
        last_published_at: published_at,
        version_no: version,
        content_hash: hash,
        last_error: None,
    })
}

async fn sync_publisher_content(state: &AppState) -> AppResult<()> {
    let (_port, token, _version, template_id, body, hash, _published_at) =
        state.db.publish_record().await?;
    if let Some(body) = body {
        if let Some(handle) = state.publisher.lock().await.as_ref() {
            let template_id = template_id.unwrap_or_else(|| "clash-mihomo".into());
            *handle.content.write().await =
                published_content(token, body, hash.unwrap_or_default(), &template_id);
        }
    } else if let Some(handle) = state.publisher.lock().await.take() {
        let _ = handle.shutdown.send(());
    }
    Ok(())
}

fn published_content(
    token: String,
    body: String,
    hash: String,
    template_id: &str,
) -> PublishedContent {
    PublishedContent {
        token,
        body,
        hash,
        content_type: templates::content_type(template_id).into(),
        file_name: templates::file_name(template_id).into(),
    }
}
