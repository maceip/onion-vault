//! Configuration of the CEF engine.

use std::net::SocketAddr;

/// Everything the CEF handlers need to know.
#[derive(Clone, Debug)]
pub struct OvConfig {
    /// The address of the local SOCKS5 proxy that carries the Tor traffic.
    pub proxy_endpoint: SocketAddr,
    /// When `true` every clearnet request is blocked: the browser can only
    /// reach `.onion` services (hidden service only mode).
    pub hidden_service_only: bool,
    /// When `true` SVG resources are blocked; this shrinks the parser attack
    /// surface of a hostile hidden service.
    pub block_svg: bool,
}

impl Default for OvConfig {
    fn default() -> Self {
        Self {
            proxy_endpoint: SocketAddr::from(([127, 0, 0, 1], 9050)),
            hidden_service_only: true,
            block_svg: false,
        }
    }
}

impl OvConfig {
    /// The `--proxy-server` value.
    pub fn proxy_server(&self) -> String {
        format!("socks5://{}", self.proxy_endpoint)
    }

    /// The `--host-resolver-rules` value.
    ///
    /// `MAP * ~NOTFOUND` makes Chromium resolve every name through the proxy
    /// (the SOCKS5 proxy resolves the `.onion` address), and `EXCLUDE
    /// 127.0.0.1` keeps the local SOCKS5 listener reachable.  Without this
    /// rule Chromium would leak DNS queries to the local resolver.
    pub fn host_resolver_rules(&self) -> &'static str {
        "MAP * ~NOTFOUND , EXCLUDE 127.0.0.1"
    }
}

/// `true` when the URL may be loaded in hidden service only mode.
pub fn is_allowed_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();

    // Internal schemes that never leave the device.
    if lower.starts_with("about:") || lower.starts_with("data:") || lower.starts_with("blob:") {
        return true;
    }

    // Loopback URLs: the SOCKS5 listener and the local dev tools.
    if lower.starts_with("http://127.0.0.1") || lower.starts_with("http://localhost") {
        return true;
    }

    // Hidden services.
    if let Some(rest) = lower.split_once("://") {
        let host = rest.1.split(['/', '?', '#']).next().unwrap_or("");
        let host = host.rsplit('@').next().unwrap_or(host);
        let host = host.split(':').next().unwrap_or(host);
        if host.ends_with(".onion") {
            return true;
        }
    }

    false
}

/// `true` when the URL points at an SVG resource.
pub fn is_svg_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or("");
    path.to_ascii_lowercase().ends_with(".svg")
}
