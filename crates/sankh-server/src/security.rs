//! Request guards: Host allow-list (DNS rebinding), Origin check (CSRF) and
//! bearer token auth for the API.

use crate::AppState;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::net::IpAddr;
use std::sync::Arc;

pub fn is_loopback(listen: &str) -> bool {
    listen == "localhost"
        || listen
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Host part of a `Host` header or origin authority, without the port.
fn host_only(authority: &str) -> String {
    let a = authority.trim().to_ascii_lowercase();
    if let Some(rest) = a.strip_prefix('[') {
        return format!("[{}]", rest.split(']').next().unwrap_or(""));
    }
    a.rsplit_once(':')
        .filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit()))
        .map(|(h, _)| h.to_string())
        .unwrap_or(a)
}

fn deny(status: StatusCode, msg: &str) -> Response {
    (status, axum::Json(serde_json::json!({ "error": msg }))).into_response()
}

pub async fn guard(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();

    if state.check_host && !state.allowed_hosts.contains(&host_only(&host)) {
        return deny(StatusCode::FORBIDDEN, "host not allowed");
    }

    let is_api = req.uri().path().starts_with("/api/");
    if is_api {
        if let Some(origin) = headers.get(header::ORIGIN).and_then(|o| o.to_str().ok()) {
            let authority = origin.split_once("://").map(|(_, a)| a).unwrap_or("");
            if authority.is_empty() || !authority.eq_ignore_ascii_case(&host) {
                return deny(StatusCode::FORBIDDEN, "cross-origin request rejected");
            }
        }
        if let Some(token) = &state.token {
            if !bearer_matches(headers, token) {
                return deny(StatusCode::UNAUTHORIZED, "missing or invalid token");
            }
        }
    }

    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert("x-frame-options", "DENY".parse().unwrap());
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("referrer-policy", "no-referrer".parse().unwrap());
    h.insert(
        "content-security-policy",
        "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none'"
            .parse()
            .unwrap(),
    );
    resp
}

fn bearer_matches(headers: &HeaderMap, token: &str) -> bool {
    let Some(given) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    else {
        return false;
    };
    constant_time_eq(given.trim().as_bytes(), token.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hosts() {
        assert_eq!(host_only("localhost:4747"), "localhost");
        assert_eq!(host_only("[::1]:4747"), "[::1]");
        assert_eq!(host_only("Example.com"), "example.com");
        assert!(is_loopback("127.0.0.1"));
        assert!(is_loopback("::1"));
        assert!(!is_loopback("0.0.0.0"));
    }
}
