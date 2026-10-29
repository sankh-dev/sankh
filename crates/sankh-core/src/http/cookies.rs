//! A run's cookie jar (`-b file`, `-c file`): stores `Set-Cookie` and sends
//! matching cookies, scoped to one run.

use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    /// Lowercase, without a leading dot.
    pub domain: String,
    /// Only for exactly `domain`, not its subdomains.
    pub host_only: bool,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// Unix seconds; `None` for a session cookie.
    pub expires: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct Jar {
    pub cookies: Vec<Cookie>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// The directory part of a URL path, the default cookie path.
fn default_path(url: &Url) -> String {
    let path = url.path();
    match path.rfind('/') {
        Some(0) | None => "/".into(),
        Some(i) => path[..i].to_string(),
    }
}

fn domain_matches(host: &str, domain: &str) -> bool {
    host == domain || (host.ends_with(domain) && host[..host.len() - domain.len()].ends_with('.'))
}

fn path_matches(path: &str, cookie_path: &str) -> bool {
    path == cookie_path
        || (path.starts_with(cookie_path)
            && (cookie_path.ends_with('/') || path[cookie_path.len()..].starts_with('/')))
}

fn parse_date(text: &str) -> Option<u64> {
    let normalized = text.trim().replace('-', " ");
    let t = httpdate::parse_http_date(&normalized)
        .or_else(|_| httpdate::parse_http_date(text.trim()))
        .ok()?;
    Some(
        t.duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
}

impl Jar {
    fn upsert(&mut self, c: Cookie) {
        self.cookies
            .retain(|o| !(o.name == c.name && o.domain == c.domain && o.path == c.path));
        if c.expires.is_none_or(|e| e > now()) {
            self.cookies.push(c);
        }
    }

    /// Stores the `Set-Cookie` header values of a response to `url`.
    pub fn store<'a>(&mut self, url: &Url, values: impl IntoIterator<Item = &'a str>) {
        let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
            return;
        };
        for v in values {
            if let Some(c) = parse_set_cookie(v, &host, url) {
                self.upsert(c);
            }
        }
    }

    /// The `Cookie` header value for a request to `url`.
    pub fn header_for(&self, url: &Url) -> Option<String> {
        let host = url.host_str()?.to_ascii_lowercase();
        let path = url.path();
        let secure = url.scheme() == "https";
        let t = now();
        let mut matching: Vec<&Cookie> = self
            .cookies
            .iter()
            .filter(|c| {
                (if c.host_only {
                    host == c.domain
                } else {
                    domain_matches(&host, &c.domain)
                }) && path_matches(path, &c.path)
                    && (!c.secure || secure)
                    && c.expires.is_none_or(|e| e > t)
            })
            .collect();
        if matching.is_empty() {
            return None;
        }
        matching.sort_by_key(|c| std::cmp::Reverse(c.path.len()));
        Some(
            matching
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }

    /// Loads a Netscape cookie file, or a file of `Set-Cookie:` lines.
    pub fn load(&mut self, text: &str) -> Result<(), String> {
        for (i, raw) in text.lines().enumerate() {
            let (line, http_only) = match raw.strip_prefix("#HttpOnly_") {
                Some(rest) => (rest, true),
                None => (raw, false),
            };
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(header) = line
                .strip_prefix("Set-Cookie:")
                .or_else(|| line.strip_prefix("set-cookie:"))
            {
                let fake = Url::parse("http://localhost/").expect("static url");
                if let Some(c) = parse_set_cookie(header.trim(), "localhost", &fake) {
                    self.upsert(c);
                }
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 7 {
                return Err(format!(
                    "line {}: not a Netscape cookie line (7 tab-separated fields)",
                    i + 1
                ));
            }
            let domain = fields[0].trim_start_matches('.').to_ascii_lowercase();
            let expires = fields[4].parse::<u64>().ok().filter(|e| *e > 0);
            self.upsert(Cookie {
                name: fields[5].to_string(),
                value: fields[6].to_string(),
                host_only: !fields[1].eq_ignore_ascii_case("TRUE"),
                domain,
                path: fields[2].to_string(),
                secure: fields[3].eq_ignore_ascii_case("TRUE"),
                http_only,
                expires,
            });
        }
        Ok(())
    }

    /// The jar in Netscape format, as `curl -c` writes it.
    pub fn to_netscape(&self) -> String {
        let mut out = String::from(
            "# Netscape HTTP Cookie File\n# Written by Sankh. Edit at your own risk.\n\n",
        );
        for c in &self.cookies {
            let domain = if c.host_only {
                c.domain.clone()
            } else {
                format!(".{}", c.domain)
            };
            out.push_str(&format!(
                "{}{domain}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                if c.http_only { "#HttpOnly_" } else { "" },
                if c.host_only { "FALSE" } else { "TRUE" },
                c.path,
                if c.secure { "TRUE" } else { "FALSE" },
                c.expires.unwrap_or(0),
                c.name,
                c.value
            ));
        }
        out
    }
}

fn parse_set_cookie(value: &str, host: &str, url: &Url) -> Option<Cookie> {
    let mut parts = value.split(';');
    let (name, val) = parts.next()?.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let mut c = Cookie {
        name: name.to_string(),
        value: val.trim().to_string(),
        domain: host.to_string(),
        host_only: true,
        path: default_path(url),
        secure: false,
        http_only: false,
        expires: None,
    };
    let mut max_age: Option<i64> = None;
    for attr in parts {
        let (k, v) = match attr.split_once('=') {
            Some((k, v)) => (k.trim().to_ascii_lowercase(), v.trim()),
            None => (attr.trim().to_ascii_lowercase(), ""),
        };
        match k.as_str() {
            "domain" if !v.is_empty() => {
                let d = v.trim_start_matches('.').to_ascii_lowercase();
                if !domain_matches(host, &d) {
                    return None;
                }
                c.domain = d;
                c.host_only = false;
            }
            "path" if v.starts_with('/') => c.path = v.to_string(),
            "expires" => {
                if let Some(t) = parse_date(v) {
                    c.expires = Some(t);
                }
            }
            "max-age" => max_age = v.parse().ok(),
            "secure" => c.secure = true,
            "httponly" => c.http_only = true,
            _ => {}
        }
    }
    if let Some(age) = max_age {
        c.expires = Some(if age <= 0 { 1 } else { now() + age as u64 });
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn stores_and_matches() {
        let mut jar = Jar::default();
        jar.store(
            &u("https://api.test/auth/login"),
            [
                "sid=1; Path=/; HttpOnly",
                "pref=dark; Domain=.api.test; Path=/",
                "local=x",
                "sec=y; Secure; Path=/",
                "gone=z; Max-Age=0",
            ],
        );
        assert_eq!(
            jar.header_for(&u("https://api.test/auth/me")).as_deref(),
            Some("local=x; sid=1; pref=dark; sec=y")
        );
        assert_eq!(
            jar.header_for(&u("http://v1.api.test/other")).as_deref(),
            Some("pref=dark")
        );
        assert_eq!(jar.header_for(&u("http://other.test/")), None);
        jar.store(
            &u("https://api.test/"),
            ["sid=; Path=/; Expires=Thu, 01-Jan-1970 00:00:01 GMT"],
        );
        assert!(!jar.cookies.iter().any(|c| c.name == "sid"));
    }

    #[test]
    fn rejects_foreign_domains() {
        let mut jar = Jar::default();
        jar.store(&u("http://a.test/"), ["x=1; Domain=b.test"]);
        assert!(jar.cookies.is_empty());
    }

    #[test]
    fn netscape_round_trip() {
        let mut jar = Jar::default();
        jar.store(
            &u("http://a.test/x/y"),
            ["a=1; HttpOnly", "b=2; Domain=a.test; Path=/"],
        );
        let text = jar.to_netscape();
        let mut back = Jar::default();
        back.load(&text).unwrap();
        assert_eq!(back.cookies, jar.cookies);
        assert!(back.load("bad line").is_err());
    }
}
