use crate::models::{ProxyGroup, TemplateSummary};

pub fn list() -> Vec<TemplateSummary> {
    vec![
        TemplateSummary {
            id: "clash-mihomo".into(),
            version: 1,
            name: "Clash / Mihomo".into(),
            description: "适用于 Clash Meta、Mihomo 及其兼容客户端的通用 YAML 配置。".into(),
            core: "Clash / Mihomo".into(),
            output_format: "mihomo-yaml".into(),
            file_name: "merge-clash.yaml".into(),
            supported_modes: vec!["proxy-providers".into(), "embedded-proxies".into()],
            default_mode: "proxy-providers".into(),
            groups: vec!["节点选择".into(), "发达地区自动".into(), "美国自动".into()],
            external_dependencies: vec!["MetaCubeX 中国大陆域名规则集".into()],
        },
        TemplateSummary {
            id: "v2rayn".into(),
            version: 1,
            name: "v2RayN".into(),
            description: "Base64 编码的通用分享链接订阅，支持 SS、VMess、VLESS 和 Trojan。".into(),
            core: "v2RayN".into(),
            output_format: "base64-uri-list".into(),
            file_name: "v2rayn.txt".into(),
            supported_modes: vec!["embedded-proxies".into()],
            default_mode: "embedded-proxies".into(),
            groups: vec![],
            external_dependencies: vec![],
        },
        TemplateSummary {
            id: "trojan".into(),
            version: 1,
            name: "Trojan".into(),
            description: "仅包含 Trojan 节点的 Base64 通用 URI 订阅。".into(),
            core: "Trojan URI".into(),
            output_format: "base64-uri-list".into(),
            file_name: "trojan.txt".into(),
            supported_modes: vec!["embedded-proxies".into()],
            default_mode: "embedded-proxies".into(),
            groups: vec![],
            external_dependencies: vec![],
        },
        TemplateSummary {
            id: "shadowrocket".into(),
            version: 1,
            name: "Shadowrocket".into(),
            description:
                "适用于 Shadowrocket 的 Base64 分享链接订阅，支持 SS、VMess、VLESS 和 Trojan。"
                    .into(),
            core: "Shadowrocket".into(),
            output_format: "base64-uri-list".into(),
            file_name: "shadowrocket.txt".into(),
            supported_modes: vec!["embedded-proxies".into()],
            default_mode: "embedded-proxies".into(),
            groups: vec![],
            external_dependencies: vec![],
        },
    ]
}

pub fn file_name(template_id: &str) -> &'static str {
    match template_id {
        "clash-mihomo" => "merge-clash.yaml",
        "v2rayn" => "v2rayn.txt",
        "trojan" => "trojan.txt",
        "shadowrocket" => "shadowrocket.txt",
        _ => "subscription.txt",
    }
}

pub fn content_type(template_id: &str) -> &'static str {
    if template_id == "clash-mihomo" {
        "text/yaml; charset=utf-8"
    } else {
        "text/plain; charset=utf-8"
    }
}

pub fn groups(template_id: &str) -> Vec<ProxyGroup> {
    if template_id != "clash-mihomo" {
        return vec![];
    }
    let auto = "发达地区自动";
    let groups = vec![
        ProxyGroup { name: "节点选择".into(), group_type: "select".into(), members: vec![auto.into(), "美国自动".into(), "DIRECT".into()], filter: None, exclude_filter: None, url: None, interval: None, tolerance: None, lazy: None },
        ProxyGroup { name: auto.into(), group_type: "url-test".into(), members: vec![], filter: Some(developed_filter()), exclude_filter: Some(notice_filter()), url: Some("https://www.gstatic.com/generate_204".into()), interval: Some(300), tolerance: Some(50), lazy: Some(true) },
        ProxyGroup { name: "美国自动".into(), group_type: "url-test".into(), members: vec![], filter: Some("(?i)(美国|美國|美西|美东|美東|美中|美南|\\bus\\b|\\busa\\b|united states|america|los angeles|san jose|seattle|new york|dallas|chicago|washington)".into()), exclude_filter: Some(notice_filter()), url: Some("https://www.gstatic.com/generate_204".into()), interval: Some(300), tolerance: Some(50), lazy: Some(true) },
    ];
    groups
}

fn developed_filter() -> String {
    "(?i)(台湾|台灣|\\btw\\b|taiwan|新加坡|狮城|獅城|\\bsg\\b|singapore|日本|东京|東京|大阪|\\bjp\\b|japan|tokyo|osaka|韓國|首爾|\\bkr\\b|korea|seoul|美国|美國|\\bus\\b|\\busa\\b|united states|america|美西|美东|美東|加拿大|canada|英国|united kingdom|澳大利亚|australia|新西兰|new zealand|法国|france|德国|germany|荷兰|netherlands)".into()
}

fn notice_filter() -> String {
    "(?i)(剩余流量|套餐到期|到期时间|流量重置|traffic|expire|subscription|reset|plan|官网|官方|通知|客户端|更新|升级|备用域名|旧节点|帮助中心|流量|套餐|到期|重置|剩余)".into()
}
