use crate::{
    database::now_ms,
    error::{AppError, AppResult},
    fetcher::{proxy_name, proxy_type, FetchedConfig},
    models::*,
    templates,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use regex::Regex;
use serde_json::json;
use serde_yaml::{Mapping, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const BUILTIN_MEMBERS: [&str; 5] = ["DIRECT", "REJECT", "REJECT-DROP", "PASS", "GLOBAL"];

pub struct SourceConfig {
    pub subscription: InternalSubscription,
    pub fetched: FetchedConfig,
}

pub fn build(
    settings: &ProjectSettings,
    sources: &[SourceConfig],
    previous: Option<&StoredDraft>,
    source_failures: Vec<String>,
) -> AppResult<StoredDraft> {
    let mut issues = Vec::new();
    let mut stored_proxies = Vec::new();
    let mut seen = HashMap::<String, String>::new();
    let mut collisions = Vec::new();
    for source in sources {
        for raw in &source.fetched.proxies {
            let Some(name) = proxy_name(raw) else {
                continue;
            };
            if let Some(first_source) = seen.get(&name) {
                collisions.push(format!(
                    "{name}（{} / {}）",
                    first_source, source.subscription.safe.name
                ));
                if settings.merge_mode == "embedded-proxies" {
                    continue;
                }
            } else {
                seen.insert(name.clone(), source.subscription.safe.name.clone());
            }
            stored_proxies.push(StoredProxy {
                meta: ProxyNode {
                    id: Uuid::new_v4().to_string(),
                    name,
                    proxy_type: proxy_type(raw).unwrap_or_else(|| "unknown".into()),
                    source_id: source.subscription.safe.id.clone(),
                    source_name: source.subscription.safe.name.clone(),
                },
                raw: raw.clone(),
            });
        }
    }
    if !collisions.is_empty() {
        issues.push(ValidationIssue {
            severity: if settings.merge_mode == "proxy-providers" {
                "blocker".into()
            } else {
                "warning".into()
            },
            code: "duplicate-proxy-names".into(),
            message: format!("发现 {} 个跨来源同名节点", collisions.len()),
            target: Some(
                collisions
                    .into_iter()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("、"),
            ),
        });
    }
    if settings.merge_mode == "proxy-providers" {
        issues.push(ValidationIssue {
            severity: "warning".into(),
            code: "sensitive-provider-urls".into(),
            message: "动态模式生成文件包含原始订阅地址".into(),
            target: None,
        });
    }
    if !source_failures.is_empty() {
        issues.push(ValidationIssue {
            severity: "warning".into(),
            code: "partial-refresh".into(),
            message: format!("{} 个订阅刷新失败，未纳入本次草稿", source_failures.len()),
            target: Some(source_failures.join("、")),
        });
    }
    if stored_proxies.is_empty() {
        issues.push(ValidationIssue {
            severity: "blocker".into(),
            code: "no-proxies".into(),
            message: "没有可用于生成配置的节点".into(),
            target: None,
        });
    }

    let mut groups = previous
        .filter(|draft| {
            draft.draft.template_id == settings.template_id
                && draft.draft.merge_mode == settings.merge_mode
        })
        .map(|draft| draft.draft.groups.clone())
        .unwrap_or_else(|| templates::groups(&settings.template_id));
    if settings.template_id == "clash-mihomo" {
        templates::upgrade_groups(&mut groups);
        populate_groups(&mut groups, &stored_proxies, &settings.merge_mode);
        issues.extend(validate_groups(
            &groups,
            &stored_proxies,
            &settings.merge_mode,
        ));
    } else {
        groups.clear();
    }
    let yaml = render_output(settings, sources, &stored_proxies, &groups, &mut issues)?;
    let revision = previous.map_or(1, |draft| draft.draft.revision + 1);
    let draft = Draft {
        revision,
        template_id: settings.template_id.clone(),
        template_version: settings.template_version,
        merge_mode: settings.merge_mode.clone(),
        proxies: stored_proxies
            .iter()
            .map(|item| item.meta.clone())
            .collect(),
        groups,
        yaml,
        issues,
        source_failures,
        updated_at: now_ms(),
        published_at: previous.and_then(|item| item.draft.published_at),
    };
    Ok(StoredDraft {
        draft,
        stored_proxies,
    })
}

pub fn rerender(
    settings: &ProjectSettings,
    stored: &mut StoredDraft,
    subscriptions: &[InternalSubscription],
) -> AppResult<()> {
    if settings.template_id == "clash-mihomo" {
        templates::upgrade_groups(&mut stored.draft.groups);
    }
    sanitize_provider_group_members(&mut stored.draft.groups, &settings.merge_mode);
    stored.draft.issues.retain(|issue| {
        matches!(
            issue.code.as_str(),
            "sensitive-provider-urls" | "partial-refresh" | "duplicate-proxy-names"
        )
    });
    if settings.template_id == "clash-mihomo" {
        stored.draft.issues.extend(validate_groups(
            &stored.draft.groups,
            &stored.stored_proxies,
            &settings.merge_mode,
        ));
    } else {
        stored.draft.groups.clear();
    }
    let empty_sources: Vec<SourceConfig> = subscriptions
        .iter()
        .cloned()
        .map(|subscription| SourceConfig {
            subscription,
            fetched: FetchedConfig {
                text: String::new(),
                proxies: vec![],
                proxy_types: vec![],
                elapsed_ms: 0,
                hash: String::new(),
            },
        })
        .collect();
    stored.draft.yaml = render_output(
        settings,
        &empty_sources,
        &stored.stored_proxies,
        &stored.draft.groups,
        &mut stored.draft.issues,
    )?;
    stored.draft.updated_at = now_ms();
    stored.draft.revision += 1;
    Ok(())
}

pub fn sanitize_provider_group_members(groups: &mut [ProxyGroup], merge_mode: &str) -> bool {
    if merge_mode != "proxy-providers" {
        return false;
    }
    let group_names = groups
        .iter()
        .map(|group| group.name.clone())
        .collect::<HashSet<_>>();
    let mut changed = false;
    for group in groups {
        let group_name = group.name.clone();
        let mut seen = HashSet::new();
        let before = group.members.len();
        group.members.retain(|member| {
            member != &group_name
                && (group_names.contains(member) || BUILTIN_MEMBERS.contains(&member.as_str()))
                && seen.insert(member.clone())
        });
        changed |= before != group.members.len();
    }
    changed
}

fn populate_groups(groups: &mut [ProxyGroup], proxies: &[StoredProxy], merge_mode: &str) {
    if merge_mode != "embedded-proxies" {
        sanitize_provider_group_members(groups, merge_mode);
        return;
    }
    for group in groups {
        if group.group_type == "select" {
            continue;
        }
        let include = group
            .filter
            .as_deref()
            .and_then(|value| Regex::new(value).ok());
        let exclude = group
            .exclude_filter
            .as_deref()
            .and_then(|value| Regex::new(value).ok());
        group.members = proxies
            .iter()
            .filter(|proxy| {
                include
                    .as_ref()
                    .is_none_or(|regex| regex.is_match(&proxy.meta.name))
                    && exclude
                        .as_ref()
                        .is_none_or(|regex| !regex.is_match(&proxy.meta.name))
            })
            .map(|proxy| proxy.meta.name.clone())
            .collect();
        if group.members.is_empty() {
            group.members.push("DIRECT".into());
        }
    }
}

fn validate_groups(
    groups: &[ProxyGroup],
    proxies: &[StoredProxy],
    merge_mode: &str,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let group_names: HashSet<&str> = groups.iter().map(|group| group.name.as_str()).collect();
    if group_names.len() != groups.len() {
        issues.push(issue(
            "blocker",
            "duplicate-group-names",
            "代理组名称必须唯一",
            None,
        ));
    }
    let proxy_names: HashSet<&str> = proxies
        .iter()
        .map(|proxy| proxy.meta.name.as_str())
        .collect();
    for group in groups {
        if group.name.trim().is_empty() {
            issues.push(issue(
                "blocker",
                "empty-group-name",
                "代理组名称不能为空",
                None,
            ));
        }
        if !["select", "url-test", "fallback", "load-balance"].contains(&group.group_type.as_str())
        {
            issues.push(issue(
                "blocker",
                "unsupported-group-type",
                "代理组类型不受支持",
                Some(group.name.clone()),
            ));
        }
        if let Some(filter) = &group.filter {
            if Regex::new(filter).is_err() {
                issues.push(issue(
                    "blocker",
                    "invalid-filter",
                    "filter 正则无效",
                    Some(group.name.clone()),
                ));
            }
        }
        if let Some(filter) = &group.exclude_filter {
            if Regex::new(filter).is_err() {
                issues.push(issue(
                    "blocker",
                    "invalid-exclude-filter",
                    "exclude-filter 正则无效",
                    Some(group.name.clone()),
                ));
            }
        }
        for member in &group.members {
            if member == &group.name {
                issues.push(issue(
                    "blocker",
                    "self-reference",
                    "代理组不能引用自身",
                    Some(group.name.clone()),
                ));
            }
            let valid_member = group_names.contains(member.as_str())
                || BUILTIN_MEMBERS.contains(&member.as_str())
                || (merge_mode == "embedded-proxies" && proxy_names.contains(member.as_str()));
            if !valid_member {
                issues.push(issue(
                    "blocker",
                    "invalid-group-reference",
                    "代理组包含无效成员",
                    Some(format!("{} -> {}", group.name, member)),
                ));
            }
        }
    }
    if has_cycle(groups) {
        issues.push(issue(
            "blocker",
            "group-cycle",
            "代理组之间存在循环引用",
            None,
        ));
    }
    issues
}

fn has_cycle(groups: &[ProxyGroup]) -> bool {
    let map: HashMap<&str, Vec<&str>> = groups
        .iter()
        .map(|group| {
            (
                group.name.as_str(),
                group.members.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    fn visit<'a>(
        name: &'a str,
        map: &HashMap<&'a str, Vec<&'a str>>,
        visiting: &mut HashSet<&'a str>,
        visited: &mut HashSet<&'a str>,
    ) -> bool {
        if visiting.contains(name) {
            return true;
        }
        if visited.contains(name) {
            return false;
        }
        visiting.insert(name);
        if let Some(members) = map.get(name) {
            for member in members {
                if map.contains_key(member) && visit(member, map, visiting, visited) {
                    return true;
                }
            }
        }
        visiting.remove(name);
        visited.insert(name);
        false
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    map.keys()
        .any(|name| visit(name, &map, &mut visiting, &mut visited))
}

fn render_yaml(
    settings: &ProjectSettings,
    sources: &[SourceConfig],
    proxies: &[StoredProxy],
    groups: &[ProxyGroup],
) -> AppResult<String> {
    let mut root = Mapping::new();
    put(&mut root, "mixed-port", 7890);
    put(&mut root, "allow-lan", false);
    put(&mut root, "mode", "rule");
    put(&mut root, "log-level", "info");
    put(&mut root, "ipv6", false);
    let mut profile = Mapping::new();
    put(&mut profile, "store-selected", true);
    root.insert(key("profile"), Value::Mapping(profile));
    let mut dns = Mapping::new();
    put(&mut dns, "enable", true);
    put(&mut dns, "ipv6", false);
    put(&mut dns, "enhanced-mode", "fake-ip");
    put(&mut dns, "fake-ip-range", "198.18.0.1/16");
    dns.insert(
        key("nameserver"),
        Value::Sequence(vec![
            value("https://dns.alidns.com/dns-query"),
            value("https://doh.pub/dns-query"),
        ]),
    );
    root.insert(key("dns"), Value::Mapping(dns));
    let provider_ids = sources
        .iter()
        .map(|source| provider_id(&source.subscription.safe.id))
        .collect::<Vec<_>>();
    if settings.merge_mode == "proxy-providers" {
        let mut providers = Mapping::new();
        for source in sources {
            let id = provider_id(&source.subscription.safe.id);
            let mut provider = Mapping::new();
            put(&mut provider, "type", "http");
            put(&mut provider, "url", source.subscription.url.as_str());
            put(&mut provider, "path", format!("./providers/{id}.yaml"));
            put(&mut provider, "interval", 86400);
            let mut health = Mapping::new();
            put(&mut health, "enable", true);
            put(&mut health, "url", "https://www.gstatic.com/generate_204");
            put(&mut health, "interval", 300);
            provider.insert(key("health-check"), Value::Mapping(health));
            providers.insert(key(&id), Value::Mapping(provider));
        }
        root.insert(key("proxy-providers"), Value::Mapping(providers));
    } else {
        root.insert(
            key("proxies"),
            Value::Sequence(proxies.iter().map(|proxy| proxy.raw.clone()).collect()),
        );
    }
    let group_names = groups
        .iter()
        .map(|group| group.name.as_str())
        .collect::<HashSet<_>>();
    let group_values = groups
        .iter()
        .map(|group| group_yaml(group, &provider_ids, &settings.merge_mode, &group_names))
        .collect();
    root.insert(key("proxy-groups"), Value::Sequence(group_values));
    if settings.template_id == "clash-mihomo" {
        let mut rule_providers = Mapping::new();
        let mut cn = Mapping::new();
        put(&mut cn, "type", "http");
        put(&mut cn, "behavior", "domain");
        put(&mut cn, "format", "mrs");
        put(&mut cn, "interval", 86400);
        put(&mut cn, "path", "./rules/geosite-cn.mrs");
        put(
            &mut cn,
            "url",
            "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/cn.mrs",
        );
        rule_providers.insert(key("cn"), Value::Mapping(cn));
        root.insert(key("rule-providers"), Value::Mapping(rule_providers));
    }
    let mut rules = vec![
        "GEOSITE,private,DIRECT".to_string(),
        "GEOIP,private,DIRECT,no-resolve".to_string(),
    ];
    if settings.template_id == "clash-mihomo" {
        rules.push("RULE-SET,cn,DIRECT".into());
    }
    rules.extend(["GEOIP,CN,DIRECT".into(), "MATCH,节点选择".into()]);
    root.insert(
        key("rules"),
        Value::Sequence(rules.into_iter().map(Value::String).collect()),
    );
    serde_yaml::to_string(&Value::Mapping(root)).map_err(|e| AppError::Config(e.to_string()))
}

fn render_output(
    settings: &ProjectSettings,
    sources: &[SourceConfig],
    proxies: &[StoredProxy],
    groups: &[ProxyGroup],
    issues: &mut Vec<ValidationIssue>,
) -> AppResult<String> {
    if settings.template_id == "clash-mihomo" {
        return render_yaml(settings, sources, proxies, groups);
    }
    let trojan_only = settings.template_id == "trojan";
    let mut links = Vec::new();
    let mut unsupported = Vec::new();
    for proxy in proxies {
        if trojan_only && proxy.meta.proxy_type != "trojan" {
            continue;
        }
        match proxy_uri(proxy) {
            Ok(link) => links.push(link),
            Err(reason) => unsupported.push(format!("{}（{}）", proxy.meta.name, reason)),
        }
    }
    if !unsupported.is_empty() {
        issues.push(issue(
            "warning",
            "unsupported-output-proxy",
            &format!("{} 个节点无法转换，已跳过", unsupported.len()),
            Some(
                unsupported
                    .into_iter()
                    .take(5)
                    .collect::<Vec<_>>()
                    .join("、"),
            ),
        ));
    }
    if links.is_empty() {
        issues.push(issue(
            "blocker",
            "no-compatible-proxies",
            if trojan_only {
                "没有可用于 Trojan 订阅的节点"
            } else {
                "没有可转换为通用分享链接的节点"
            },
            None,
        ));
    }
    Ok(STANDARD.encode(links.join("\n")))
}

fn proxy_uri(proxy: &StoredProxy) -> Result<String, String> {
    match proxy.meta.proxy_type.as_str() {
        "ss" => ss_uri(proxy),
        "vmess" => vmess_uri(proxy),
        "vless" => vless_uri(proxy),
        "trojan" => trojan_uri(proxy),
        other => Err(format!("不支持 {other}")),
    }
}

fn ss_uri(proxy: &StoredProxy) -> Result<String, String> {
    if optional_value(&proxy.raw, "plugin").is_some() {
        return Err("暂不支持带插件的 Shadowsocks 节点".into());
    }
    let cipher = field_str(&proxy.raw, "cipher")?;
    let password = field_str(&proxy.raw, "password")?;
    let server = field_str(&proxy.raw, "server")?;
    let port = field_port(&proxy.raw)?;
    let userinfo = URL_SAFE_NO_PAD.encode(format!("{cipher}:{password}"));
    let mut url = url::Url::parse(&format!(
        "ss://{userinfo}@{}:{port}",
        authority_host(server)
    ))
    .map_err(|_| "服务器地址无效".to_string())?;
    url.set_fragment(Some(&proxy.meta.name));
    Ok(url.to_string())
}

fn trojan_uri(proxy: &StoredProxy) -> Result<String, String> {
    let password = field_str(&proxy.raw, "password")?;
    let server = field_str(&proxy.raw, "server")?;
    let port = field_port(&proxy.raw)?;
    let mut url = url::Url::parse(&format!("trojan://{}:{port}", authority_host(server)))
        .map_err(|_| "服务器地址无效".to_string())?;
    url.set_username(password)
        .map_err(|_| "密码无效".to_string())?;
    append_common_query(&mut url, &proxy.raw);
    url.set_fragment(Some(&proxy.meta.name));
    Ok(url.to_string())
}

fn vless_uri(proxy: &StoredProxy) -> Result<String, String> {
    let uuid = field_str(&proxy.raw, "uuid")?;
    let server = field_str(&proxy.raw, "server")?;
    let port = field_port(&proxy.raw)?;
    let mut url = url::Url::parse(&format!("vless://{}:{port}", authority_host(server)))
        .map_err(|_| "服务器地址无效".to_string())?;
    url.set_username(uuid)
        .map_err(|_| "UUID 无效".to_string())?;
    append_common_query(&mut url, &proxy.raw);
    if let Some(flow) = optional_str(&proxy.raw, "flow") {
        url.query_pairs_mut().append_pair("flow", flow);
    }
    url.set_fragment(Some(&proxy.meta.name));
    Ok(url.to_string())
}

fn vmess_uri(proxy: &StoredProxy) -> Result<String, String> {
    let server = field_str(&proxy.raw, "server")?;
    let port = field_port(&proxy.raw)?;
    let uuid = field_str(&proxy.raw, "uuid")?;
    let network = optional_str(&proxy.raw, "network").unwrap_or("tcp");
    let tls = if field_bool(&proxy.raw, "tls") {
        "tls"
    } else {
        ""
    };
    let ws = mapping_field(&proxy.raw, "ws-opts");
    let path = ws.and_then(|map| map_str(map, "path")).unwrap_or("");
    let host = ws
        .and_then(|map| map.get(key("headers")).and_then(Value::as_mapping))
        .and_then(|map| map_str(map, "Host"))
        .unwrap_or("");
    let payload = json!({
        "v": "2", "ps": proxy.meta.name, "add": server, "port": port.to_string(),
        "id": uuid, "aid": optional_i64(&proxy.raw, "alterId").unwrap_or(0).to_string(),
        "scy": optional_str(&proxy.raw, "cipher").unwrap_or("auto"), "net": network,
        "type": "none", "host": host, "path": path, "tls": tls,
        "sni": optional_str(&proxy.raw, "servername").or_else(|| optional_str(&proxy.raw, "sni")).unwrap_or("")
    });
    let json = serde_json::to_vec(&payload).map_err(|_| "VMess JSON 生成失败".to_string())?;
    Ok(format!("vmess://{}", STANDARD.encode(json)))
}

fn append_common_query(url: &mut url::Url, raw: &Value) {
    let network = optional_str(raw, "network").unwrap_or("tcp");
    let reality = mapping_field(raw, "reality-opts");
    let security = if reality.is_some() {
        "reality"
    } else if field_bool(raw, "tls") {
        "tls"
    } else {
        "none"
    };
    let mut query = url.query_pairs_mut();
    query
        .append_pair("type", network)
        .append_pair("security", security);
    if let Some(sni) = optional_str(raw, "servername").or_else(|| optional_str(raw, "sni")) {
        query.append_pair("sni", sni);
    }
    if field_bool(raw, "skip-cert-verify") {
        query.append_pair("allowInsecure", "1");
    }
    if let Some(fingerprint) = optional_str(raw, "client-fingerprint") {
        query.append_pair("fp", fingerprint);
    }
    if let Some(alpn) = optional_value(raw, "alpn").and_then(Value::as_sequence) {
        let joined = alpn
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(",");
        if !joined.is_empty() {
            query.append_pair("alpn", &joined);
        }
    }
    if let Some(reality) = reality {
        if let Some(public_key) = map_str(reality, "public-key") {
            query.append_pair("pbk", public_key);
        }
        if let Some(short_id) = map_str(reality, "short-id") {
            query.append_pair("sid", short_id);
        }
    }
    if network == "ws" {
        if let Some(ws) = mapping_field(raw, "ws-opts") {
            if let Some(path) = map_str(ws, "path") {
                query.append_pair("path", path);
            }
            if let Some(host) = ws
                .get(key("headers"))
                .and_then(Value::as_mapping)
                .and_then(|map| map_str(map, "Host"))
            {
                query.append_pair("host", host);
            }
        }
    } else if network == "grpc" {
        if let Some(grpc) = mapping_field(raw, "grpc-opts") {
            if let Some(service_name) = map_str(grpc, "grpc-service-name") {
                query.append_pair("serviceName", service_name);
            }
        }
    }
}

fn authority_host(server: &str) -> String {
    if server.contains(':') && !server.starts_with('[') {
        format!("[{server}]")
    } else {
        server.into()
    }
}

fn mapping_field<'a>(raw: &'a Value, name: &str) -> Option<&'a Mapping> {
    raw.as_mapping()?.get(key(name))?.as_mapping()
}
fn optional_value<'a>(raw: &'a Value, name: &str) -> Option<&'a Value> {
    raw.as_mapping()?.get(key(name))
}
fn optional_str<'a>(raw: &'a Value, name: &str) -> Option<&'a str> {
    optional_value(raw, name)?.as_str()
}
fn field_str<'a>(raw: &'a Value, name: &str) -> Result<&'a str, String> {
    optional_str(raw, name).ok_or_else(|| format!("缺少 {name}"))
}
fn optional_i64(raw: &Value, name: &str) -> Option<i64> {
    optional_value(raw, name)?.as_i64()
}
fn field_port(raw: &Value) -> Result<u16, String> {
    let value = optional_value(raw, "port").ok_or_else(|| "缺少 port".to_string())?;
    let port = value
        .as_u64()
        .or_else(|| value.as_str()?.parse().ok())
        .ok_or_else(|| "port 无效".to_string())?;
    u16::try_from(port).map_err(|_| "port 超出范围".to_string())
}
fn field_bool(raw: &Value, name: &str) -> bool {
    optional_value(raw, name)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
fn map_str<'a>(map: &'a Mapping, name: &str) -> Option<&'a str> {
    map.get(key(name)).and_then(Value::as_str)
}

fn group_yaml(
    group: &ProxyGroup,
    provider_ids: &[String],
    merge_mode: &str,
    group_names: &HashSet<&str>,
) -> Value {
    let mut map = Mapping::new();
    put(&mut map, "name", group.name.as_str());
    put(&mut map, "type", group.group_type.as_str());
    if group.name == "GLOBAL" {
        put(&mut map, "include-all", true);
        put(&mut map, "exclude-type", "Direct");
        return Value::Mapping(map);
    }
    if merge_mode == "proxy-providers" {
        map.insert(
            key("use"),
            Value::Sequence(provider_ids.iter().cloned().map(Value::String).collect()),
        );
    }
    let members = group
        .members
        .iter()
        .filter(|member| {
            merge_mode != "proxy-providers"
                || group_names.contains(member.as_str())
                || BUILTIN_MEMBERS.contains(&member.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    if !members.is_empty() {
        map.insert(
            key("proxies"),
            Value::Sequence(members.into_iter().map(Value::String).collect()),
        );
    }
    if let Some(filter) = &group.filter {
        put(&mut map, "filter", filter.as_str());
    }
    if let Some(filter) = &group.exclude_filter {
        put(&mut map, "exclude-filter", filter.as_str());
    }
    if let Some(url) = &group.url {
        put(&mut map, "url", url.as_str());
    }
    if let Some(interval) = group.interval {
        put(&mut map, "interval", interval);
    }
    if let Some(tolerance) = group.tolerance {
        put(&mut map, "tolerance", tolerance);
    }
    if let Some(lazy) = group.lazy {
        put(&mut map, "lazy", lazy);
    }
    if group.group_type == "url-test" {
        put(&mut map, "expected-status", 204);
    }
    Value::Mapping(map)
}

fn provider_id(id: &str) -> String {
    format!(
        "sub_{}",
        id.replace('-', "").chars().take(8).collect::<String>()
    )
}
fn issue(severity: &str, code: &str, message: &str, target: Option<String>) -> ValidationIssue {
    ValidationIssue {
        severity: severity.into(),
        code: code.into(),
        message: message.into(),
        target,
    }
}
fn key(value: &str) -> Value {
    Value::String(value.into())
}
fn value<T: serde::Serialize>(value: T) -> Value {
    serde_yaml::to_value(value).unwrap_or(Value::Null)
}
fn put<T: serde::Serialize>(mapping: &mut Mapping, name: &str, item: T) {
    mapping.insert(key(name), value(item));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stored(proxy_type: &str, yaml: &str) -> StoredProxy {
        StoredProxy {
            meta: ProxyNode {
                id: "1".into(),
                name: "测试 节点".into(),
                proxy_type: proxy_type.into(),
                source_id: "s1".into(),
                source_name: "源".into(),
            },
            raw: serde_yaml::from_str(yaml).unwrap(),
        }
    }
    #[test]
    fn detects_group_cycle() {
        let groups = vec![
            ProxyGroup {
                name: "A".into(),
                group_type: "select".into(),
                members: vec!["B".into()],
                filter: None,
                exclude_filter: None,
                url: None,
                interval: None,
                tolerance: None,
                lazy: None,
            },
            ProxyGroup {
                name: "B".into(),
                group_type: "select".into(),
                members: vec!["A".into()],
                filter: None,
                exclude_filter: None,
                url: None,
                interval: None,
                tolerance: None,
                lazy: None,
            },
        ];
        assert!(has_cycle(&groups));
    }

    #[test]
    fn provider_mode_removes_snapshot_node_members() {
        let mut groups = vec![
            ProxyGroup {
                name: "节点选择".into(),
                group_type: "select".into(),
                members: vec!["自动选择".into(), "快照节点".into(), "DIRECT".into()],
                filter: None,
                exclude_filter: None,
                url: None,
                interval: None,
                tolerance: None,
                lazy: None,
            },
            ProxyGroup {
                name: "自动选择".into(),
                group_type: "url-test".into(),
                members: vec!["快照节点".into()],
                filter: Some("日本".into()),
                exclude_filter: None,
                url: Some("https://www.gstatic.com/generate_204".into()),
                interval: Some(300),
                tolerance: Some(50),
                lazy: Some(true),
            },
        ];
        assert!(sanitize_provider_group_members(
            &mut groups,
            "proxy-providers"
        ));
        assert_eq!(groups[0].members, vec!["自动选择", "DIRECT"]);
        assert!(groups[1].members.is_empty());

        let names = groups
            .iter()
            .map(|group| group.name.as_str())
            .collect::<HashSet<_>>();
        let rendered = group_yaml(&groups[1], &["sub_demo".into()], "proxy-providers", &names);
        let mapping = rendered.as_mapping().expect("group should be a mapping");
        assert!(mapping.contains_key(key("use")));
        assert!(!mapping.contains_key(key("proxies")));
    }

    #[test]
    fn upgrades_legacy_groups_for_global_and_sticky_auto_selection() {
        let mut groups = vec![
            ProxyGroup {
                name: "节点选择".into(),
                group_type: "select".into(),
                members: vec!["发达地区自动".into(), "DIRECT".into()],
                filter: None,
                exclude_filter: None,
                url: None,
                interval: None,
                tolerance: None,
                lazy: None,
            },
            ProxyGroup {
                name: "发达地区自动".into(),
                group_type: "url-test".into(),
                members: vec![],
                filter: Some("日本".into()),
                exclude_filter: None,
                url: Some("https://www.gstatic.com/generate_204".into()),
                interval: Some(300),
                tolerance: Some(50),
                lazy: Some(true),
            },
        ];

        templates::upgrade_groups(&mut groups);

        assert_eq!(groups[0].name, "GLOBAL");
        assert!(groups[0].members.is_empty());
        assert_eq!(groups[2].tolerance, Some(65535));
    }

    #[test]
    fn rendered_yaml_customizes_global_without_direct_and_stores_selection() {
        let settings = ProjectSettings {
            template_id: "clash-mihomo".into(),
            template_version: 1,
            merge_mode: "embedded-proxies".into(),
            theme: "system".into(),
        };
        let groups = templates::groups("clash-mihomo");
        let yaml = render_yaml(&settings, &[], &[], &groups).unwrap();
        let root: Value = serde_yaml::from_str(&yaml).unwrap();
        let mapping = root.as_mapping().unwrap();
        let profile = mapping.get(key("profile")).unwrap().as_mapping().unwrap();
        assert_eq!(
            profile.get(key("store-selected")).and_then(Value::as_bool),
            Some(true)
        );
        let rendered_groups = mapping
            .get(key("proxy-groups"))
            .unwrap()
            .as_sequence()
            .unwrap();
        let global = rendered_groups[0].as_mapping().unwrap();
        assert_eq!(
            global.get(key("name")).and_then(Value::as_str),
            Some("GLOBAL")
        );
        assert_eq!(
            global.get(key("include-all")).and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            global.get(key("exclude-type")).and_then(Value::as_str),
            Some("Direct")
        );
        assert!(!global.contains_key(key("proxies")));
        assert!(!global.contains_key(key("use")));
    }

    #[test]
    fn renders_supported_share_links() {
        let ss = proxy_uri(&stored(
            "ss",
            "server: example.com\nport: 443\ncipher: aes-128-gcm\npassword: secret",
        ))
        .unwrap();
        assert!(ss.starts_with("ss://"));
        assert!(ss.contains("example.com:443"));

        let trojan = proxy_uri(&stored("trojan", "server: example.com\nport: 443\npassword: secret\ntls: true\nsni: cdn.example.com\nnetwork: ws\nws-opts:\n  path: /socket\n  headers:\n    Host: ws.example.com")).unwrap();
        assert!(trojan.starts_with("trojan://secret@example.com:443"));
        assert!(trojan.contains("security=tls"));
        assert!(trojan.contains("path=%2Fsocket"));

        let vless = proxy_uri(&stored("vless", "server: example.com\nport: 443\nuuid: 00000000-0000-0000-0000-000000000000\ntls: true\nflow: xtls-rprx-vision")).unwrap();
        assert!(vless.starts_with("vless://00000000-0000-0000-0000-000000000000@example.com:443"));
        assert!(vless.contains("flow=xtls-rprx-vision"));

        let vmess = proxy_uri(&stored("vmess", "server: example.com\nport: 443\nuuid: 00000000-0000-0000-0000-000000000000\nalterId: 0\ncipher: auto\nnetwork: ws\ntls: true\nws-opts:\n  path: /socket")).unwrap();
        let payload = STANDARD
            .decode(vmess.trim_start_matches("vmess://"))
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(json["add"], "example.com");
        assert_eq!(json["net"], "ws");
    }

    #[test]
    fn trojan_template_blocks_when_no_trojan_nodes() {
        let settings = ProjectSettings {
            template_id: "trojan".into(),
            template_version: 1,
            merge_mode: "embedded-proxies".into(),
            theme: "system".into(),
        };
        let mut issues = Vec::new();
        let output = render_output(
            &settings,
            &[],
            &[stored(
                "ss",
                "server: example.com\nport: 443\ncipher: aes-128-gcm\npassword: secret",
            )],
            &[],
            &mut issues,
        )
        .unwrap();
        assert!(output.is_empty());
        assert!(issues
            .iter()
            .any(|item| item.code == "no-compatible-proxies" && item.severity == "blocker"));
    }
}
