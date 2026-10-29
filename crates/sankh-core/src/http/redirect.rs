//! curl's redirect rules (`-L`).

use reqwest::Method;
use url::Url;

pub fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Whether the next request keeps the method and body: 301 and 302 turn a
/// `POST` into a `GET`, 303 turns anything but `HEAD` into a `GET`, 307
/// and 308 keep both.
pub fn switches_to_get(status: u16, method: &Method) -> bool {
    match status {
        301 | 302 => method == Method::POST,
        303 => method != Method::HEAD,
        _ => false,
    }
}

/// Scheme, host and port: credentials are not sent across a change.
pub fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme()
        && a.host_str().map(str::to_ascii_lowercase) == b.host_str().map(str::to_ascii_lowercase)
        && a.port_or_known_default() == b.port_or_known_default()
}

/// Headers a request must not repeat once its body is dropped, or once it
/// goes to another origin.
pub fn strip_headers(headers: &mut Vec<(String, String)>, body_dropped: bool, cross_origin: bool) {
    headers.retain(|(k, _)| {
        let k = k.to_ascii_lowercase();
        !(body_dropped && (k == "content-type" || k == "content-length"))
            && !(cross_origin && (k == "authorization" || k == "cookie"))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_rules() {
        assert!(switches_to_get(302, &Method::POST));
        assert!(!switches_to_get(302, &Method::PUT));
        assert!(switches_to_get(303, &Method::PUT));
        assert!(!switches_to_get(303, &Method::HEAD));
        assert!(!switches_to_get(307, &Method::POST));
        assert!(!switches_to_get(308, &Method::POST));
    }

    #[test]
    fn origins() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert!(same_origin(&u("http://A.test/x"), &u("http://a.test:80/y")));
        assert!(!same_origin(&u("http://a.test/"), &u("https://a.test/")));
        assert!(!same_origin(&u("http://a.test/"), &u("http://a.test:81/")));
        let mut h = vec![
            ("Authorization".to_string(), "x".to_string()),
            ("Content-Type".to_string(), "y".to_string()),
            ("X".to_string(), "z".to_string()),
        ];
        strip_headers(&mut h, true, true);
        assert_eq!(h, vec![("X".to_string(), "z".to_string())]);
    }
}
