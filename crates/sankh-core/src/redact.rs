//! Secret masking for everything Sankh prints, reports or sends to the UI.

use crate::env::Vars;

pub const MASK: &str = "***";
const SECRET_MARKERS: &[&str] = &[
    "TOKEN", "SECRET", "KEY", "PASSWORD", "PASSWD", "AUTH", "COOKIE",
];
/// Shorter values are not masked: they would mangle unrelated output.
const MIN_SECRET_LEN: usize = 4;

pub fn is_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_MARKERS.iter().any(|m| upper.contains(m))
}

#[derive(Debug, Clone, Default)]
pub struct Redactor {
    secrets: Vec<String>,
}

impl Redactor {
    pub fn from_vars(vars: &Vars) -> Redactor {
        let mut r = Redactor::default();
        for (k, v) in vars {
            if is_secret_name(k) {
                r.add(v);
            }
        }
        r
    }

    pub fn add(&mut self, value: &str) {
        let value = value.trim();
        if value.len() >= MIN_SECRET_LEN && !self.secrets.iter().any(|s| s == value) {
            self.secrets.push(value.to_string());
            self.secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for s in &self.secrets {
            if out.contains(s.as_str()) {
                out = out.replace(s.as_str(), MASK);
            }
        }
        out
    }

    /// Masks a variable for display: secret-named variables are always fully masked.
    pub fn display_var(&self, name: &str, value: &str) -> String {
        if is_secret_name(name) && !value.is_empty() {
            MASK.to_string()
        } else {
            self.redact(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_secret_values() {
        let vars = Vars::from([
            ("API_TOKEN".to_string(), "abc123xyz".to_string()),
            ("BASE_URL".to_string(), "http://x".to_string()),
            ("SHORT_KEY".to_string(), "ab".to_string()),
        ]);
        let r = Redactor::from_vars(&vars);
        assert_eq!(
            r.redact("Bearer abc123xyz at http://x ab"),
            "Bearer *** at http://x ab"
        );
        assert_eq!(r.display_var("SHORT_KEY", "ab"), "***");
        assert_eq!(r.display_var("BASE_URL", "http://x"), "http://x");
    }
}
