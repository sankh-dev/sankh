//! Request body encodings, matching curl's output.

/// URL-encodes like `curl --data-urlencode`: unreserved characters stay,
/// space becomes `+`, everything else `%xx` (uppercase hex, as curl writes it).
pub fn urlencode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A random multipart boundary in curl's shape (24 dashes, 22 characters).
pub fn boundary() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = "-".repeat(24);
    let mut seed = random_u64();
    for i in 0..22 {
        if i % 10 == 0 {
            seed ^= random_u64();
        }
        out.push(ALPHABET[(seed % ALPHABET.len() as u64) as usize] as char);
        seed /= ALPHABET.len() as u64;
    }
    out
}

fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
    );
    h.finish()
}

pub struct Part<'a> {
    pub name: &'a str,
    pub filename: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub content: &'a [u8],
}

/// Encodes `multipart/form-data`.
pub fn multipart<'a>(boundary: &str, parts: impl IntoIterator<Item = Part<'a>>) -> Vec<u8> {
    let mut out = Vec::new();
    for p in parts {
        out.extend(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"",
                escape(p.name)
            )
            .bytes(),
        );
        if let Some(f) = p.filename {
            out.extend(format!("; filename=\"{}\"", escape(f)).bytes());
        }
        out.extend(b"\r\n");
        if let Some(t) = p.content_type {
            out.extend(format!("Content-Type: {t}\r\n").bytes());
        }
        out.extend(b"\r\n");
        out.extend_from_slice(p.content);
        out.extend(b"\r\n");
    }
    out.extend(format!("--{boundary}--\r\n").bytes());
    out
}

fn escape(s: &str) -> String {
    s.replace('"', "%22")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Content type of an uploaded file, from curl's extension table.
pub fn guess_type(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "gif" => "image/gif",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "htm" | "html" => "text/html",
        "pdf" => "application/pdf",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencodes_like_curl() {
        assert_eq!(
            urlencode("AZaz09-._~!*'()/:;@+,=?#[]{}$&%\"<>\\^`| \tÄ€".as_bytes()),
            "AZaz09-._~%21%2A%27%28%29%2F%3A%3B%40%2B%2C%3D%3F%23%5B%5D%7B%7D%24%26%25%22%3C%3E%5C%5E%60%7C+%09%C3%84%E2%82%AC"
        );
    }

    #[test]
    fn boundaries_look_like_curls() {
        let b = boundary();
        assert_eq!(b.len(), 46);
        assert!(b.starts_with(&"-".repeat(24)));
        assert_ne!(b, boundary());
    }
}
