//! What a request returned.

use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct Response {
    pub status: u16,
    /// `HTTP/1.1`, `HTTP/2.0`.
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// The body was longer than `max_body` and was cut.
    pub body_truncated: bool,
    /// Until the body was read, redirects included.
    pub time_ms: f64,
    /// Until the final response's headers arrived.
    pub ttfb_ms: Option<f64>,
    /// The final URL, after redirects.
    pub url: String,
    pub redirects: Vec<RedirectHop>,
    /// Problems that did not fail the request (e.g. saving the cookie jar).
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RedirectHop {
    pub status: u16,
    pub url: String,
    pub location: String,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}
