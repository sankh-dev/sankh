//! The HTTP engine: sends a [`RequestSpec`] with Sankh's own client.
//!
//! One [`Engine`] serves a run: it keeps connections alive, the cookie jar
//! for `-b`/`-c`, and a client per distinct set of TLS and proxy options.

pub mod body;
pub mod client;
pub mod cookies;
pub mod redirect;
pub mod response;
mod send;

pub use response::{RedirectHop, Response};

use crate::parse::spec::RequestSpec;
use client::ClientKey;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Default cap on a response body kept in memory.
pub const DEFAULT_MAX_BODY: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// DNS, connect, TLS, protocol and body errors.
    Transport(String),
    Timeout(Duration),
    Cancelled,
    /// Refused before anything was sent.
    Config(String),
}

/// Which hosts requests may reach (`sankh serve --allow-target/--deny-target`).
#[derive(Debug, Clone, Default)]
pub struct HostPolicy {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

impl HostPolicy {
    pub fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }

    pub fn check(&self, url: &url::Url) -> Result<(), String> {
        let host = url
            .host_str()
            .unwrap_or("")
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_ascii_lowercase();
        if let Some(p) = self.deny.iter().find(|p| host_matches(p, &host)) {
            return Err(format!(
                "requests to {host} are blocked (--deny-target {p})"
            ));
        }
        if !self.allow.is_empty() && !self.allow.iter().any(|p| host_matches(p, &host)) {
            return Err(format!(
                "requests to {host} are not allowed (see --allow-target)"
            ));
        }
        Ok(())
    }
}

/// `*`, `example.com`, `*.example.com`, an IP address or a CIDR range.
fn host_matches(pattern: &str, host: &str) -> bool {
    let p = pattern.trim().to_ascii_lowercase();
    if p == "*" || p == host {
        return true;
    }
    if let Some(suffix) = p.strip_prefix("*.") {
        return host == suffix || host.ends_with(&format!(".{suffix}"));
    }
    let Some((net, bits)) = p.split_once('/') else {
        return false;
    };
    let (Ok(net), Ok(bits), Ok(ip)) = (
        net.parse::<std::net::IpAddr>(),
        bits.parse::<u32>(),
        host.parse::<std::net::IpAddr>(),
    ) else {
        return false;
    };
    match (net, ip) {
        (std::net::IpAddr::V4(n), std::net::IpAddr::V4(i)) if bits <= 32 => {
            let mask = u32::MAX.checked_shl(32 - bits).unwrap_or(0);
            u32::from(n) & mask == u32::from(i) & mask
        }
        (std::net::IpAddr::V6(n), std::net::IpAddr::V6(i)) if bits <= 128 => {
            let mask = u128::MAX.checked_shl(128 - bits).unwrap_or(0);
            u128::from(n) & mask == u128::from(i) & mask
        }
        _ => false,
    }
}

pub struct Engine {
    rt: Option<tokio::runtime::Runtime>,
    clients: HashMap<ClientKey, reqwest::Client>,
    jar: cookies::Jar,
    loaded: HashSet<PathBuf>,
    /// Honor `HTTP_PROXY`, `HTTPS_PROXY` and `NO_PROXY`.
    pub env_proxy: bool,
    pub max_body: usize,
    pub policy: Option<Arc<HostPolicy>>,
}

impl Default for Engine {
    fn default() -> Engine {
        Engine {
            rt: None,
            clients: HashMap::new(),
            jar: cookies::Jar::default(),
            loaded: HashSet::new(),
            env_proxy: true,
            max_body: DEFAULT_MAX_BODY,
            policy: None,
        }
    }
}

impl Engine {
    /// Sends `spec`, giving up after `timeout` (per attempt) or when
    /// `cancel` is set.
    pub fn send(
        &mut self,
        spec: &RequestSpec,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<Response, SendError> {
        for path in &spec.cookies.load {
            if self.loaded.insert(path.clone()) {
                let text = std::fs::read_to_string(path).map_err(|e| {
                    SendError::Config(format!("cannot read {}: {e}", path.display()))
                })?;
                self.jar
                    .load(&text)
                    .map_err(|e| SendError::Config(format!("{}: {e}", path.display())))?;
            }
        }
        if self.rt.is_none() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| SendError::Config(format!("cannot start the HTTP runtime: {e}")))?;
            self.rt = Some(rt);
        }
        let Engine {
            rt,
            clients,
            jar,
            max_body,
            policy,
            env_proxy,
            ..
        } = self;
        let rt = rt.as_ref().expect("runtime was just created");
        let key = ClientKey::for_spec(spec, *env_proxy);
        let client = match clients.get(&key) {
            Some(c) => c.clone(),
            None => {
                let _guard = rt.enter();
                let c = client::build(&key).map_err(SendError::Config)?;
                clients.insert(key, c.clone());
                c
            }
        };
        let mut ctx = send::Ctx {
            client: &client,
            jar,
            policy: policy.as_deref(),
            max_body: *max_body,
        };
        let mut result = rt.block_on(async {
            tokio::select! {
                r = send::send(spec, &mut ctx, timeout) => r,
                _ = cancelled(cancel) => Err(SendError::Cancelled),
            }
        });
        if let (Ok(resp), Some(path)) = (&mut result, &spec.cookies.save) {
            if let Err(e) = std::fs::write(path, self.jar.to_netscape()) {
                resp.notes.push(format!(
                    "could not write cookies to {}: {e}",
                    path.display()
                ));
            }
        }
        result
    }
}

async fn cancelled(flag: &AtomicBool) {
    while !flag.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_patterns() {
        assert!(host_matches("*", "x"));
        assert!(host_matches("API.test", "api.test"));
        assert!(host_matches("*.test", "a.b.test"));
        assert!(host_matches("*.test", "test"));
        assert!(!host_matches("*.test", "atest"));
        assert!(host_matches("10.0.0.0/8", "10.2.3.4"));
        assert!(!host_matches("10.0.0.0/8", "11.0.0.1"));
        assert!(host_matches("::1/128", "::1"));
        assert!(host_matches("0.0.0.0/0", "8.8.8.8"));
        let p = HostPolicy {
            allow: vec!["*.internal".into()],
            deny: vec!["secret.internal".into()],
        };
        let u = |s: &str| url::Url::parse(s).unwrap();
        assert!(p.check(&u("http://api.internal/")).is_ok());
        assert!(p.check(&u("http://secret.internal/")).is_err());
        assert!(p.check(&u("http://example.com/")).is_err());
        assert!(p.check(&u("http://[::1]/")).is_err());
    }
}
