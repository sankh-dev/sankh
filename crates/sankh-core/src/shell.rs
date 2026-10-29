//! Shell-style quoting for rendering request files, and `$VAR` helpers.
//!
//! Word values follow one convention used across Sankh: `$VAR` means "expand
//! at run time", and a literal dollar sign is written `\$`.

/// Quotes a word so the shell reproduces it, expanding `$VAR` references.
pub fn quote(word: &str) -> String {
    let has_expansion = has_unescaped_dollar(word);
    if !has_expansion {
        let literal = word.replace("\\$", "$");
        if !literal.is_empty()
            && literal
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c))
        {
            return literal;
        }
        return format!("'{}'", literal.replace('\'', "'\\''"));
    }
    let mut out = String::from("\"");
    let chars: Vec<char> = word.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if chars.get(i + 1) == Some(&'$') => {
                out.push_str("\\$");
                i += 1;
            }
            '\\' | '"' | '`' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out.push('"');
    out
}

fn has_unescaped_dollar(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    (0..chars.len()).any(|i| chars[i] == '$' && (i == 0 || chars[i - 1] != '\\'))
}

/// Lists variable names referenced as `$VAR` or `${VAR}` (ignores `\$`).
pub fn referenced_vars(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut in_single = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            in_single = !in_single;
        } else if c == '\\' {
            i += 2;
            continue;
        } else if c == '$' && !in_single {
            let (name, len) = var_at(&chars[i + 1..]);
            if let Some(name) = name {
                if !out.contains(&name) {
                    out.push(name);
                }
                i += len;
            }
        }
        i += 1;
    }
    out
}

fn var_at(chars: &[char]) -> (Option<String>, usize) {
    if chars.first() == Some(&'{') {
        let end = chars.iter().position(|&c| c == '}');
        if let Some(end) = end {
            let inner: String = chars[1..end].iter().collect();
            let name: String = inner
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if is_var_name(&name) {
                return (Some(name), end + 1);
            }
        }
        return (None, 0);
    }
    let name: String = chars
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == '_')
        .collect();
    if is_var_name(&name) {
        let len = name.len();
        (Some(name), len)
    } else {
        (None, 0)
    }
}

fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
}

/// Replaces `$VAR` / `${VAR}` with values from `lookup`; unknown names expand
/// to the empty string, like the shell. `\$` becomes a literal `$`.
pub fn expand(text: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && chars.get(i + 1) == Some(&'$') {
            out.push('$');
            i += 2;
            continue;
        }
        if c == '$' {
            let (name, len) = var_at(&chars[i + 1..]);
            if let Some(name) = name {
                out.push_str(&lookup(&name).unwrap_or_default());
                i += len + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tokenize::tokenize;

    #[test]
    fn quote_round_trips() {
        for word in [
            "$BASE_URL/users",
            "a b",
            "it's",
            r#"{"a":1}"#,
            "\\$lit",
            "x\"y`z",
            "plain",
            "${A:-x}/b",
        ] {
            let quoted = quote(word);
            let words = tokenize(&quoted, 0).unwrap().words;
            assert_eq!(words.len(), 1, "quoted as {quoted}");
            assert_eq!(words[0].template(), word, "quoted as {quoted}");
        }
    }

    #[test]
    fn finds_vars() {
        assert_eq!(
            referenced_vars(r#""$A/${B_2}" '$NOT' \$NO $_C"#),
            vec!["A", "B_2", "_C"]
        );
        assert_eq!(
            expand("x=$A ${B} \\$C", |n| Some(n.to_lowercase())),
            "x=a b $C"
        );
    }
}
