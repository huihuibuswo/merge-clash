use crate::error::{AppError, AppResult};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use std::{
    env,
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};
use tokio::sync::{oneshot, RwLock};

pub struct NetworkEnvironment {
    pub lan_addresses: Vec<String>,
    pub proxy_detected: bool,
}

#[derive(Clone)]
pub struct PublishedContent {
    pub token: String,
    pub body: String,
    pub hash: String,
    pub content_type: String,
    pub file_name: String,
}

pub struct ServerHandle {
    pub shutdown: oneshot::Sender<()>,
    pub content: Arc<RwLock<PublishedContent>>,
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub async fn start(port: u16, content: PublishedContent) -> AppResult<ServerHandle> {
    let state = Arc::new(RwLock::new(content));
    let app = Router::new()
        .route("/subscription/:token/config", get(serve_config))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|e| AppError::Operation(format!("端口 {port} 无法监听: {e}")))?;
    let (shutdown, receiver) = oneshot::channel();
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = receiver.await;
            })
            .await;
    });
    Ok(ServerHandle {
        shutdown,
        content: state,
    })
}

async fn serve_config(
    Path(token): Path<String>,
    State(state): State<Arc<RwLock<PublishedContent>>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let content = state.read().await;
    if token.as_bytes() != content.token.as_bytes() {
        return (StatusCode::NOT_FOUND, HeaderMap::new(), String::new());
    }
    let etag = format!("\"{}\"", content.hash);
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        == Some(etag.as_str())
    {
        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::ETAG, etag.parse().unwrap());
        return (StatusCode::NOT_MODIFIED, response_headers, String::new());
    }
    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, content.content_type.parse().unwrap());
    response_headers.insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
    response_headers.insert(
        header::CONTENT_DISPOSITION,
        format!("inline; filename=\"{}\"", content.file_name)
            .parse()
            .unwrap(),
    );
    response_headers.insert(header::ETAG, etag.parse().unwrap());
    (StatusCode::OK, response_headers, content.body.clone())
}

pub fn network_environment() -> NetworkEnvironment {
    let interfaces = local_ip_address::list_afinet_netifas().unwrap_or_default();
    let default_ip = local_ip_address::local_ip().ok().and_then(|ip| match ip {
        IpAddr::V4(ip) => Some(ip),
        IpAddr::V6(_) => None,
    });
    let proxy_interface_detected = interfaces.iter().any(|(name, ip)| {
        matches!(ip, IpAddr::V4(ip) if !ip.is_loopback())
            && (is_proxy_interface(name) || matches!(ip, IpAddr::V4(ip) if is_proxy_ipv4(*ip)))
    });
    NetworkEnvironment {
        lan_addresses: select_lan_addresses(default_ip, interfaces),
        proxy_detected: proxy_interface_detected || proxy_setting_enabled(),
    }
}

fn select_lan_addresses(
    default_ip: Option<Ipv4Addr>,
    interfaces: Vec<(String, IpAddr)>,
) -> Vec<String> {
    let mut candidates = interfaces
        .into_iter()
        .filter_map(|(name, ip)| match ip {
            IpAddr::V4(ip) if is_lan_ipv4(ip) && !is_virtual_interface(&name) => Some((name, ip)),
            _ => None,
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(name, ip)| {
        (
            if Some(*ip) == default_ip { 0 } else { 1 },
            physical_interface_rank(name),
            lan_address_rank(*ip),
            ip.octets(),
        )
    });
    candidates.dedup_by_key(|(_, ip)| *ip);
    candidates
        .into_iter()
        .map(|(_, ip)| ip.to_string())
        .collect()
}

fn is_lan_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private() || (octets[0] == 100 && (64..=127).contains(&octets[1]))
}

fn is_virtual_interface(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "clash",
        "meta",
        "mihomo",
        "wintun",
        "tun",
        "tap",
        "vpn",
        "wireguard",
        "tailscale",
        "zerotier",
        "vethernet",
        "hyper-v",
        "vmware",
        "virtualbox",
        "docker",
        "wsl",
        "loopback",
        "npcap",
    ]
    .iter()
    .any(|marker| name.contains(marker))
}

fn is_proxy_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 198 && (octets[1] == 18 || octets[1] == 19)
}

fn is_proxy_interface(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "clash",
        "mihomo",
        "wintun",
        "tun",
        "tap",
        "vpn",
        "wireguard",
        "tailscale",
        "zerotier",
    ]
    .iter()
    .any(|marker| name.contains(marker))
}

fn physical_interface_rank(name: &str) -> u8 {
    let name = name.to_ascii_lowercase();
    if ["ethernet", "wi-fi", "wifi", "wlan", "以太网", "无线"]
        .iter()
        .any(|marker| name.contains(marker))
    {
        0
    } else {
        1
    }
}

fn lan_address_rank(ip: Ipv4Addr) -> u8 {
    match ip.octets() {
        [192, 168, _, _] => 0,
        [10, _, _, _] => 1,
        [172, second, _, _] if (16..=31).contains(&second) => 2,
        [100, second, _, _] if (64..=127).contains(&second) => 3,
        _ => 4,
    }
}

fn proxy_setting_enabled() -> bool {
    let env_proxy = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ]
    .iter()
    .any(|name| env::var(name).is_ok_and(|value| !value.trim().is_empty()));
    env_proxy || windows_proxy_enabled()
}

#[cfg(windows)]
fn windows_proxy_enabled() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let Ok(settings) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
    else {
        return false;
    };
    let proxy_enabled = settings
        .get_value::<u32, _>("ProxyEnable")
        .is_ok_and(|value| value != 0);
    let pac_enabled = settings
        .get_value::<String, _>("AutoConfigURL")
        .is_ok_and(|value| !value.trim().is_empty());
    proxy_enabled || pac_enabled
}

#[cfg(not(windows))]
fn windows_proxy_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ipv4(value: [u8; 4]) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(value))
    }

    #[test]
    fn filters_proxy_and_virtual_interfaces() {
        let addresses = select_lan_addresses(
            Some(Ipv4Addr::new(198, 18, 0, 1)),
            vec![
                ("Mihomo Wintun".into(), ipv4([198, 18, 0, 1])),
                ("vEthernet (WSL)".into(), ipv4([172, 28, 16, 1])),
                ("Wi-Fi".into(), ipv4([192, 168, 1, 23])),
            ],
        );
        assert_eq!(addresses, vec!["192.168.1.23"]);
    }

    #[test]
    fn keeps_real_default_interface_first() {
        let addresses = select_lan_addresses(
            Some(Ipv4Addr::new(10, 0, 0, 8)),
            vec![
                ("Wi-Fi".into(), ipv4([192, 168, 1, 23])),
                ("Ethernet".into(), ipv4([10, 0, 0, 8])),
            ],
        );
        assert_eq!(addresses, vec!["10.0.0.8", "192.168.1.23"]);
    }

    #[test]
    fn accepts_shared_lan_address_but_rejects_public_address() {
        let addresses = select_lan_addresses(
            None,
            vec![
                ("WLAN".into(), ipv4([100, 64, 2, 3])),
                ("Ethernet".into(), ipv4([8, 8, 8, 8])),
            ],
        );
        assert_eq!(addresses, vec!["100.64.2.3"]);
    }

    #[test]
    fn recognizes_clash_fake_ip_network() {
        assert!(is_proxy_ipv4(Ipv4Addr::new(198, 18, 0, 1)));
        assert!(is_proxy_ipv4(Ipv4Addr::new(198, 19, 255, 254)));
        assert!(!is_proxy_ipv4(Ipv4Addr::new(192, 168, 1, 23)));
    }
}
