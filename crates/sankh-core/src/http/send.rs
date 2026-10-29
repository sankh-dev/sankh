//! Executes a [`RequestSpec`]: redirects, cookies, body limit and timing.

use super::cookies::Jar;
use super::redirect;
use super::{HostPolicy, RedirectHop, Response, SendError};
use crate::parse::spec::RequestSpec;
use reqwest::Method;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, LOCATION, SET_COOKIE};
use std::time::{Duration, Instant};
use url::Url;

pub(crate) struct Ctx<'a> {
    pub client: &'a reqwest::Client,
    pub jar: &'a mut Jar,
    pub policy: Option<&'a HostPolicy>,
    pub max_body: usize,
}

/// One attempt: the request and the redirects `-L` follows.
pub(crate) async fn send_once(
    spec: &RequestSpec,
    ctx: &mut Ctx<'_>,
) -> Result<Response, SendError> {
    let start = Instant::now();
    let mut method = Method::from_bytes(spec.method.as_bytes())
        .map_err(|_| SendError::Config(format!("invalid method `{}`", spec.method)))?;
    let first = Url::parse(&spec.url)
        .map_err(|e| SendError::Config(format!("invalid URL `{}`: {e}", spec.url)))?;
    let mut url = first.clone();
    let mut headers = spec.headers.clone();
    let mut body = spec.body.clone();
    let mut redirects = Vec::new();
    let opts = &spec.options;
    loop {
        if let Some(policy) = ctx.policy {
            policy.check(&url).map_err(SendError::Config)?;
        }
        let mut map = HeaderMap::new();
        for (k, v) in &headers {
            let name = HeaderName::from_bytes(k.as_bytes())
                .map_err(|_| SendError::Config(format!("invalid header name `{k}`")))?;
            let value = HeaderValue::from_bytes(v.as_bytes())
                .map_err(|_| SendError::Config(format!("invalid value for header `{k}`")))?;
            map.append(name, value);
        }
        if spec.cookies.engine {
            if let Some(jar_cookies) = ctx.jar.header_for(&url) {
                let merged = match map
                    .get(reqwest::header::COOKIE)
                    .and_then(|v| v.to_str().ok())
                {
                    Some(own) if !own.is_empty() => format!("{own}; {jar_cookies}"),
                    _ => jar_cookies,
                };
                if let Ok(v) = HeaderValue::from_str(&merged) {
                    map.insert(reqwest::header::COOKIE, v);
                }
            }
        }
        let mut req = ctx.client.request(method.clone(), url.clone()).headers(map);
        if let Some(b) = &body {
            req = req.body(b.clone());
        }
        let resp = req
            .send()
            .await
            .map_err(|e| SendError::Transport(describe(&e, &url)))?;
        let ttfb = start.elapsed();
        let status = resp.status().as_u16();
        if spec.cookies.engine {
            ctx.jar.store(
                &url,
                resp.headers()
                    .get_all(SET_COOKIE)
                    .iter()
                    .filter_map(|v| v.to_str().ok()),
            );
        }
        let location = resp
            .headers()
            .get(LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        if let (true, true, Some(location)) = (
            opts.follow_redirects,
            redirect::is_redirect(status),
            location,
        ) {
            if redirects.len() as u32 >= opts.max_redirects {
                return Err(SendError::Transport(format!(
                    "maximum ({}) redirects followed",
                    opts.max_redirects
                )));
            }
            let next = url.join(&location).map_err(|e| {
                SendError::Transport(format!("invalid redirect location `{location}`: {e}"))
            })?;
            if !matches!(next.scheme(), "http" | "https") {
                return Err(SendError::Transport(format!(
                    "redirect to unsupported URL `{next}`"
                )));
            }
            redirects.push(RedirectHop {
                status,
                url: url.to_string(),
                location: next.to_string(),
            });
            let to_get = redirect::switches_to_get(status, &method);
            if to_get {
                method = Method::GET;
                body = None;
            }
            let cross = !opts.location_trusted && !redirect::same_origin(&first, &next);
            redirect::strip_headers(&mut headers, to_get, cross);
            url = next;
            continue;
        }
        let version = format!("{:?}", resp.version());
        let out_headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        let final_url = resp.url().to_string();
        let mut resp = resp;
        let mut buf: Vec<u8> = Vec::new();
        let mut truncated = false;
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| SendError::Transport(describe(&e, &url)))?
        {
            let room = ctx.max_body.saturating_sub(buf.len());
            if chunk.len() > room {
                buf.extend_from_slice(&chunk[..room]);
                truncated = true;
                break;
            }
            buf.extend_from_slice(&chunk);
        }
        return Ok(Response {
            status,
            version,
            headers: out_headers,
            body: buf,
            body_truncated: truncated,
            time_ms: start.elapsed().as_secs_f64() * 1000.0,
            ttfb_ms: Some(ttfb.as_secs_f64() * 1000.0),
            url: final_url,
            redirects,
            notes: Vec::new(),
        });
    }
}

/// Retries `--retry` transient failures: timeouts and 408, 429, 500, 502,
/// 503 and 504, waiting `Retry-After`, `--retry-delay` or 1 s doubling.
pub(crate) async fn send(
    spec: &RequestSpec,
    ctx: &mut Ctx<'_>,
    timeout: Duration,
) -> Result<Response, SendError> {
    let mut attempt = 0;
    let mut delay = spec.options.retry_delay.unwrap_or(Duration::from_secs(1));
    loop {
        let result = match tokio::time::timeout(timeout, send_once(spec, ctx)).await {
            Ok(r) => r,
            Err(_) => Err(SendError::Timeout(timeout)),
        };
        let transient = match &result {
            Err(SendError::Timeout(_)) => true,
            Ok(r) => matches!(r.status, 408 | 429 | 500 | 502 | 503 | 504),
            Err(_) => false,
        };
        if !transient || attempt >= spec.options.retry {
            return result.map(|mut r| {
                if attempt > 0 {
                    r.notes.push(format!("retried {attempt} time(s)"));
                }
                r
            });
        }
        attempt += 1;
        let wait = result
            .as_ref()
            .ok()
            .and_then(|r| r.header("retry-after"))
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(delay);
        tokio::time::sleep(wait.min(Duration::from_secs(600))).await;
        if spec.options.retry_delay.is_none() {
            delay = (delay * 2).min(Duration::from_secs(600));
        }
    }
}

/// A short, specific message for a client error.
fn describe(e: &reqwest::Error, url: &Url) -> String {
    let mut chain: Vec<String> = Vec::new();
    let mut src: Option<&dyn std::error::Error> = std::error::Error::source(e);
    while let Some(s) = src {
        let m = s.to_string();
        if !chain.iter().any(|c| c == &m) {
            chain.push(m);
        }
        src = s.source();
    }
    let host = url.host_str().unwrap_or("");
    let port = url.port_or_known_default().unwrap_or(0);
    let inner = chain.last().cloned().unwrap_or_else(|| e.to_string());
    let all = chain.join(": ").to_ascii_lowercase();
    if all.contains("dns error")
        || all.contains("failed to lookup address")
        || all.contains("name or service not known")
    {
        return format!("could not resolve host `{host}`");
    }
    if all.contains("certificate")
        || all.contains("tls")
        || all.contains("handshake")
        || all.contains("peer")
    {
        let mut msg = format!("TLS error with {host}:{port}: {inner}");
        if all.contains("unknownissuer")
            || all.contains("unknown issuer")
            || all.contains("certificate")
        {
            msg.push_str(" (trust its CA with --cacert, or skip verification with -k)");
        }
        return msg;
    }
    if e.is_connect() {
        return format!("could not connect to {host}:{port}: {inner}");
    }
    if e.is_body() || e.is_decode() {
        return format!("error reading the response body: {inner}");
    }
    if chain.is_empty() {
        e.to_string()
    } else {
        chain.join(": ")
    }
}
