use crate::{
    error::{AppError, AppResult},
    models::*,
    templates,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::path::Path;
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::Operation(e.to_string()))?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        let db = Self { pool };
        db.migrate().await?;
        db.seed().await?;
        Ok(db)
    }

    async fn migrate(&self) -> AppResult<()> {
        let statements = [
            "PRAGMA journal_mode = WAL",
            "CREATE TABLE IF NOT EXISTS app_settings (id INTEGER PRIMARY KEY CHECK(id=1), template_id TEXT NOT NULL, template_version INTEGER NOT NULL, merge_mode TEXT NOT NULL, theme TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS subscriptions (id TEXT PRIMARY KEY, name TEXT NOT NULL, url TEXT NOT NULL, enabled INTEGER NOT NULL, priority INTEGER NOT NULL, last_status TEXT NOT NULL DEFAULT 'never', last_error TEXT, last_fetched_at INTEGER, last_success_at INTEGER, last_tested_at INTEGER, proxy_count INTEGER NOT NULL DEFAULT 0, elapsed_ms INTEGER)",
            "CREATE TABLE IF NOT EXISTS subscription_snapshots (subscription_id TEXT PRIMARY KEY REFERENCES subscriptions(id) ON DELETE CASCADE, yaml_content TEXT NOT NULL, content_hash TEXT NOT NULL, proxy_count INTEGER NOT NULL, fetched_at INTEGER NOT NULL)",
            "CREATE TABLE IF NOT EXISTS drafts (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, template_id TEXT NOT NULL, template_version INTEGER NOT NULL, merge_mode TEXT NOT NULL, payload_json TEXT NOT NULL, yaml_content TEXT NOT NULL, updated_at INTEGER NOT NULL, published_at INTEGER)",
            "CREATE TABLE IF NOT EXISTS draft_history (id INTEGER PRIMARY KEY AUTOINCREMENT, revision INTEGER NOT NULL, action TEXT NOT NULL, payload_json TEXT NOT NULL, node_count INTEGER NOT NULL, group_count INTEGER NOT NULL, created_at INTEGER NOT NULL)",
            "CREATE TABLE IF NOT EXISTS published_versions (version_no INTEGER PRIMARY KEY AUTOINCREMENT, template_id TEXT NOT NULL, template_version INTEGER NOT NULL, merge_mode TEXT NOT NULL, yaml_content TEXT NOT NULL, content_hash TEXT NOT NULL, validation_json TEXT NOT NULL, created_at INTEGER NOT NULL)",
            "CREATE TABLE IF NOT EXISTS publish_settings (id INTEGER PRIMARY KEY CHECK(id=1), port INTEGER NOT NULL, access_token TEXT NOT NULL, active_version_no INTEGER, active_yaml TEXT, content_hash TEXT, last_published_at INTEGER)",
        ];
        for statement in statements {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        Ok(())
    }

    async fn seed(&self) -> AppResult<()> {
        sqlx::query("INSERT OR IGNORE INTO app_settings(id,template_id,template_version,merge_mode,theme) VALUES(1,'clash-mihomo',1,'proxy-providers','system')").execute(&self.pool).await?;
        sqlx::query("UPDATE app_settings SET template_id='clash-mihomo',template_version=1 WHERE template_id IN ('clash-verge-rev','flclash','mihomo-generic')").execute(&self.pool).await?;
        let token = crate::publisher::new_token();
        sqlx::query(
            "INSERT OR IGNORE INTO publish_settings(id,port,access_token) VALUES(1,17890,?)",
        )
        .bind(token)
        .execute(&self.pool)
        .await?;
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM drafts WHERE id=1")
            .fetch_one(&self.pool)
            .await?;
        if exists == 0 {
            let settings = self.settings().await?;
            let draft = Draft {
                revision: 1,
                template_id: settings.template_id.clone(),
                template_version: settings.template_version,
                merge_mode: settings.merge_mode.clone(),
                proxies: vec![],
                groups: templates::groups(&settings.template_id),
                yaml: "# 添加订阅并刷新后生成配置\n".into(),
                issues: vec![ValidationIssue {
                    severity: "warning".into(),
                    code: "no-subscriptions".into(),
                    message: "尚未添加可用订阅".into(),
                    target: None,
                }],
                source_failures: vec![],
                updated_at: now_ms(),
                published_at: None,
            };
            self.save_stored_draft(&StoredDraft {
                draft,
                stored_proxies: vec![],
            })
            .await?;
        } else {
            let template_id: String =
                sqlx::query_scalar("SELECT template_id FROM drafts WHERE id=1")
                    .fetch_one(&self.pool)
                    .await?;
            if ["clash-verge-rev", "flclash", "mihomo-generic"].contains(&template_id.as_str()) {
                self.select_template("clash-mihomo", 1, "proxy-providers")
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn settings(&self) -> AppResult<ProjectSettings> {
        let row = sqlx::query(
            "SELECT template_id,template_version,merge_mode,theme FROM app_settings WHERE id=1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(ProjectSettings {
            template_id: row.get(0),
            template_version: row.get(1),
            merge_mode: row.get(2),
            theme: row.get(3),
        })
    }

    pub async fn select_template(
        &self,
        template_id: &str,
        version: i64,
        merge_mode: &str,
    ) -> AppResult<ProjectSettings> {
        sqlx::query(
            "UPDATE app_settings SET template_id=?,template_version=?,merge_mode=? WHERE id=1",
        )
        .bind(template_id)
        .bind(version)
        .bind(merge_mode)
        .execute(&self.pool)
        .await?;
        let old = self.stored_draft().await?;
        let draft = Draft {
            revision: old.draft.revision + 1,
            template_id: template_id.into(),
            template_version: version,
            merge_mode: merge_mode.into(),
            proxies: vec![],
            groups: templates::groups(template_id),
            yaml: if template_id == "clash-mihomo" {
                "# 刷新订阅后生成新模板配置\n".into()
            } else {
                String::new()
            },
            issues: vec![ValidationIssue {
                severity: "warning".into(),
                code: "draft-rebuild-required".into(),
                message: "模板已切换，请刷新订阅重建草稿".into(),
                target: None,
            }],
            source_failures: vec![],
            updated_at: now_ms(),
            published_at: old.draft.published_at,
        };
        self.save_stored_draft(&StoredDraft {
            draft,
            stored_proxies: vec![],
        })
        .await?;
        self.settings().await
    }

    pub async fn save_theme(&self, theme: &str) -> AppResult<ProjectSettings> {
        sqlx::query("UPDATE app_settings SET theme=? WHERE id=1")
            .bind(theme)
            .execute(&self.pool)
            .await?;
        self.settings().await
    }

    pub async fn list_subscriptions(&self) -> AppResult<Vec<Subscription>> {
        Ok(self
            .list_internal_subscriptions()
            .await?
            .into_iter()
            .map(|item| item.safe)
            .collect())
    }

    pub async fn list_internal_subscriptions(&self) -> AppResult<Vec<InternalSubscription>> {
        let rows = sqlx::query("SELECT id,name,url,enabled,priority,last_status,last_error,last_fetched_at,last_success_at,last_tested_at,proxy_count,elapsed_ms FROM subscriptions ORDER BY priority,id").fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let url: String = row.get("url");
                InternalSubscription {
                    safe: Subscription {
                        id: row.get("id"),
                        name: row.get("name"),
                        url_masked: mask_url(&url),
                        enabled: row.get::<i64, _>("enabled") != 0,
                        priority: row.get("priority"),
                        last_status: row.get("last_status"),
                        last_error: row.get("last_error"),
                        last_fetched_at: row.get("last_fetched_at"),
                        last_success_at: row.get("last_success_at"),
                        last_tested_at: row.get("last_tested_at"),
                        proxy_count: row.get("proxy_count"),
                        elapsed_ms: row.get("elapsed_ms"),
                    },
                    url,
                }
            })
            .collect())
    }

    pub async fn save_subscription(&self, input: SubscriptionInput) -> AppResult<Subscription> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(AppError::InvalidInput("订阅名称不能为空".into()));
        }
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let existing = sqlx::query("SELECT url,priority FROM subscriptions WHERE id=?")
            .bind(&id)
            .fetch_optional(&self.pool)
            .await?;
        let (url, priority) = if let Some(row) = existing {
            let old_url: String = row.get("url");
            (
                if input.url.trim().is_empty() {
                    old_url
                } else {
                    input.url.trim().into()
                },
                input.priority.unwrap_or_else(|| row.get("priority")),
            )
        } else {
            if input.url.trim().is_empty() {
                return Err(AppError::InvalidInput("订阅地址不能为空".into()));
            }
            let next: i64 =
                sqlx::query_scalar("SELECT COALESCE(MAX(priority),-1)+1 FROM subscriptions")
                    .fetch_one(&self.pool)
                    .await?;
            (input.url.trim().into(), input.priority.unwrap_or(next))
        };
        validate_http_url(&url)?;
        sqlx::query("INSERT INTO subscriptions(id,name,url,enabled,priority) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,url=excluded.url,enabled=excluded.enabled,priority=excluded.priority")
            .bind(&id).bind(name).bind(&url).bind(input.enabled as i64).bind(priority).execute(&self.pool).await?;
        self.list_subscriptions()
            .await?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::Operation("保存订阅后无法读取".into()))
    }

    pub async fn delete_subscription(&self, id: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM subscriptions WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_fetch(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
        proxy_count: i64,
        elapsed_ms: i64,
        success: bool,
    ) -> AppResult<()> {
        let now = now_ms();
        sqlx::query("UPDATE subscriptions SET last_status=?,last_error=?,last_fetched_at=?,last_tested_at=?,proxy_count=?,elapsed_ms=?,last_success_at=CASE WHEN ? THEN ? ELSE last_success_at END WHERE id=?")
            .bind(status).bind(error).bind(now).bind(now).bind(proxy_count).bind(elapsed_ms).bind(success).bind(now).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn save_snapshot(
        &self,
        subscription_id: &str,
        yaml: &str,
        hash: &str,
        proxy_count: i64,
    ) -> AppResult<()> {
        sqlx::query("INSERT INTO subscription_snapshots(subscription_id,yaml_content,content_hash,proxy_count,fetched_at) VALUES(?,?,?,?,?) ON CONFLICT(subscription_id) DO UPDATE SET yaml_content=excluded.yaml_content,content_hash=excluded.content_hash,proxy_count=excluded.proxy_count,fetched_at=excluded.fetched_at")
            .bind(subscription_id).bind(yaml).bind(hash).bind(proxy_count).bind(now_ms()).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn stored_draft(&self) -> AppResult<StoredDraft> {
        let row =
            sqlx::query("SELECT payload_json,yaml_content,published_at FROM drafts WHERE id=1")
                .fetch_one(&self.pool)
                .await?;
        let mut stored: StoredDraft =
            serde_json::from_str(row.get::<String, _>("payload_json").as_str())
                .map_err(|e| AppError::Config(e.to_string()))?;
        stored.draft.yaml = row.get("yaml_content");
        stored.draft.published_at = row.get("published_at");
        Ok(stored)
    }

    pub async fn save_stored_draft(&self, stored: &StoredDraft) -> AppResult<()> {
        let payload = serde_json::to_string(stored).map_err(|e| AppError::Config(e.to_string()))?;
        sqlx::query("INSERT INTO drafts(id,revision,template_id,template_version,merge_mode,payload_json,yaml_content,updated_at,published_at) VALUES(1,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,template_id=excluded.template_id,template_version=excluded.template_version,merge_mode=excluded.merge_mode,payload_json=excluded.payload_json,yaml_content=excluded.yaml_content,updated_at=excluded.updated_at,published_at=excluded.published_at")
            .bind(stored.draft.revision).bind(&stored.draft.template_id).bind(stored.draft.template_version).bind(&stored.draft.merge_mode).bind(payload).bind(&stored.draft.yaml).bind(stored.draft.updated_at).bind(stored.draft.published_at).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn save_draft_history(&self, stored: &StoredDraft, action: &str) -> AppResult<()> {
        let payload = serde_json::to_string(stored).map_err(|e| AppError::Config(e.to_string()))?;
        sqlx::query("INSERT INTO draft_history(revision,action,payload_json,node_count,group_count,created_at) VALUES(?,?,?,?,?,?)")
            .bind(stored.draft.revision)
            .bind(action)
            .bind(payload)
            .bind(stored.draft.proxies.len() as i64)
            .bind(stored.draft.groups.len() as i64)
            .bind(now_ms())
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM draft_history WHERE id NOT IN (SELECT id FROM draft_history ORDER BY id DESC LIMIT 50)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_draft_history(&self) -> AppResult<Vec<DraftHistory>> {
        let rows = sqlx::query("SELECT id,revision,action,node_count,group_count,created_at FROM draft_history ORDER BY id DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| DraftHistory {
                id: row.get("id"),
                revision: row.get("revision"),
                action: row.get("action"),
                node_count: row.get("node_count"),
                group_count: row.get("group_count"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn draft_history_snapshot(&self, id: i64) -> AppResult<StoredDraft> {
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload_json FROM draft_history WHERE id=?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        serde_json::from_str(
            payload
                .ok_or_else(|| AppError::Operation("历史记录不存在或已清理".into()))?
                .as_str(),
        )
        .map_err(|e| AppError::Config(e.to_string()))
    }

    pub async fn delete_draft_history(&self, id: i64, current_revision: i64) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM draft_history WHERE id=? AND revision<>?")
            .bind(id)
            .bind(current_revision)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Operation(
                "草稿历史不存在，或该记录属于当前草稿".into(),
            ));
        }
        Ok(())
    }

    pub async fn delete_other_draft_history(&self, current_revision: i64) -> AppResult<()> {
        sqlx::query("DELETE FROM draft_history WHERE revision<>?")
            .bind(current_revision)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn publish(&self, stored: &StoredDraft, hash: &str) -> AppResult<i64> {
        let mut tx = self.pool.begin().await?;
        let issues = serde_json::to_string(&stored.draft.issues)
            .map_err(|e| AppError::Config(e.to_string()))?;
        let result = sqlx::query("INSERT INTO published_versions(template_id,template_version,merge_mode,yaml_content,content_hash,validation_json,created_at) VALUES(?,?,?,?,?,?,?)")
            .bind(&stored.draft.template_id).bind(stored.draft.template_version).bind(&stored.draft.merge_mode).bind(&stored.draft.yaml).bind(hash).bind(issues).bind(now_ms()).execute(&mut *tx).await?;
        let version = result.last_insert_rowid();
        let now = now_ms();
        sqlx::query("UPDATE publish_settings SET active_version_no=?,active_yaml=?,content_hash=?,last_published_at=? WHERE id=1").bind(version).bind(&stored.draft.yaml).bind(hash).bind(now).execute(&mut *tx).await?;
        sqlx::query("UPDATE drafts SET published_at=? WHERE id=1")
            .bind(now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(version)
    }

    pub async fn publish_record(
        &self,
    ) -> AppResult<(
        u16,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
    )> {
        let row = sqlx::query("SELECT s.port,s.access_token,s.active_version_no,v.template_id,s.active_yaml,s.content_hash,s.last_published_at FROM publish_settings s LEFT JOIN published_versions v ON v.version_no=s.active_version_no WHERE s.id=1").fetch_one(&self.pool).await?;
        Ok((
            row.get::<i64, _>("port") as u16,
            row.get("access_token"),
            row.get("active_version_no"),
            row.get("template_id"),
            row.get("active_yaml"),
            row.get("content_hash"),
            row.get("last_published_at"),
        ))
    }

    pub async fn list_published_versions(&self) -> AppResult<Vec<PublishedVersion>> {
        let rows = sqlx::query("SELECT v.version_no,v.template_id,v.template_version,v.merge_mode,v.content_hash,v.created_at,CASE WHEN v.version_no=s.active_version_no THEN 1 ELSE 0 END AS active FROM published_versions v CROSS JOIN publish_settings s WHERE s.id=1 ORDER BY v.version_no DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| PublishedVersion {
                version_no: row.get("version_no"),
                template_id: row.get("template_id"),
                template_version: row.get("template_version"),
                merge_mode: row.get("merge_mode"),
                content_hash: row.get("content_hash"),
                created_at: row.get("created_at"),
                active: row.get::<i64, _>("active") != 0,
            })
            .collect())
    }

    pub async fn activate_published_version(&self, version_no: i64) -> AppResult<()> {
        let row = sqlx::query("SELECT yaml_content,content_hash,created_at FROM published_versions WHERE version_no=?")
            .bind(version_no)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::Operation("发布版本不存在或已删除".into()))?;
        sqlx::query("UPDATE publish_settings SET active_version_no=?,active_yaml=?,content_hash=?,last_published_at=? WHERE id=1")
            .bind(version_no)
            .bind(row.get::<String, _>("yaml_content"))
            .bind(row.get::<String, _>("content_hash"))
            .bind(row.get::<i64, _>("created_at"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_published_version(&self, version_no: i64) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let active: Option<i64> =
            sqlx::query_scalar("SELECT active_version_no FROM publish_settings WHERE id=1")
                .fetch_one(&mut *tx)
                .await?;
        let result = sqlx::query("DELETE FROM published_versions WHERE version_no=?")
            .bind(version_no)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Operation("发布版本不存在或已删除".into()));
        }
        if active == Some(version_no) {
            let fallback = sqlx::query("SELECT version_no,yaml_content,content_hash,created_at FROM published_versions ORDER BY version_no DESC LIMIT 1")
                .fetch_optional(&mut *tx)
                .await?;
            if let Some(row) = fallback {
                sqlx::query("UPDATE publish_settings SET active_version_no=?,active_yaml=?,content_hash=?,last_published_at=? WHERE id=1")
                    .bind(row.get::<i64, _>("version_no"))
                    .bind(row.get::<String, _>("yaml_content"))
                    .bind(row.get::<String, _>("content_hash"))
                    .bind(row.get::<i64, _>("created_at"))
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query("UPDATE publish_settings SET active_version_no=NULL,active_yaml=NULL,content_hash=NULL,last_published_at=NULL WHERE id=1")
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_other_published_versions(&self) -> AppResult<()> {
        let active: Option<i64> =
            sqlx::query_scalar("SELECT active_version_no FROM publish_settings WHERE id=1")
                .fetch_one(&self.pool)
                .await?;
        let active = active.ok_or_else(|| AppError::Operation("当前没有发布版本".into()))?;
        sqlx::query("DELETE FROM published_versions WHERE version_no<>?")
            .bind(active)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_port(&self, port: u16) -> AppResult<()> {
        sqlx::query("UPDATE publish_settings SET port=? WHERE id=1")
            .bind(port as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn rotate_token(&self, token: &str) -> AppResult<()> {
        sqlx::query("UPDATE publish_settings SET access_token=? WHERE id=1")
            .bind(token)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn validate_http_url(value: &str) -> AppResult<()> {
    let parsed =
        url::Url::parse(value).map_err(|_| AppError::InvalidInput("订阅地址格式无效".into()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(
            "仅支持 http 或 https 订阅地址".into(),
        ));
    }
    if parsed.host_str().is_none() {
        return Err(AppError::InvalidInput("订阅地址缺少主机名".into()));
    }
    Ok(())
}

pub fn mask_url(value: &str) -> String {
    match url::Url::parse(value) {
        Ok(url) => format!(
            "{}://{}/...",
            url.scheme(),
            url.host_str().unwrap_or("unknown")
        ),
        Err(_) => "无效地址".into(),
    }
}
