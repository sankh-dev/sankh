//! Minimal shell lexing: enough to split a single `curl` command into words
//! and to detect shell features that push a file into raw mode.
//!
//! Word values follow one convention used across Sankh: `$VAR` means "expand
//! at run time", and a literal dollar sign is written `\$`.

/// Joins backslash-newline continuations into single spaces.
pub fn join_continuations(text: &str) -> String {
    text.replace("\\\r\n", " ").replace("\\\n", " ")
}

/// Returns a description of the first shell feature that makes `cmd` more
/// than a single plain command, if any.
pub fn find_shell_feature(cmd: &str) -> Option<&'static str> {
    #[derive(PartialEq)]
    enum Q {
        None,
        Single,
        Double,
    }
    let mut q = Q::None;
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    let mut word_start = true;
    while i < chars.len() {
        let c = chars[i];
        match q {
            Q::Single => {
                if c == '\'' {
                    q = Q::None;
                }
            }
            Q::Double => match c {
                '\\' => i += 1,
                '"' => q = Q::None,
                '`' => return Some("command substitution"),
                '$' if chars.get(i + 1) == Some(&'(') => return Some("command substitution"),
                _ => {}
            },
            Q::None => match c {
                '\\' => i += 1,
                '\'' => q = Q::Single,
                '"' => q = Q::Double,
                '`' => return Some("command substitution"),
                '$' if chars.get(i + 1) == Some(&'(') => return Some("command substitution"),
                '$' if chars.get(i + 1) == Some(&'\'') => return Some("ANSI-C quoting"),
                ';' => return Some("multiple commands (`;`)"),
                '|' => return Some("a pipe"),
                '&' => return Some("`&`"),
                '<' | '>' => return Some("redirection"),
                '\n' => return Some("multiple lines"),
                '#' if word_start => return Some("an inline comment"),
                _ => {}
            },
        }
        word_start = q == Q::None && c.is_whitespace();
        i += 1;
    }
    None
}

/// Splits a command line into words. Quotes are removed; see module docs for
/// how dollar signs are represented.
pub fn split_words(cmd: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some('$') => cur.push_str("\\$"),
                        Some(ch) => cur.push(ch),
                        None => return Err("unterminated single quote".into()),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('$') => cur.push_str("\\$"),
                            Some(ch @ ('"' | '\\' | '`')) => cur.push(ch),
                            Some('\n') => {}
                            Some(ch) => {
                                cur.push('\\');
                                cur.push(ch);
                            }
                            None => return Err("unterminated double quote".into()),
                        },
                        Some(ch) => cur.push(ch),
                        None => return Err("unterminated double quote".into()),
                    }
                }
            }
            '\\' => {
                in_word = true;
                match chars.next() {
                    Some('$') => cur.push_str("\\$"),
                    Some('\n') => {}
                    Some(ch) => cur.push(ch),
                    None => {}
                }
            }
            c => {
                in_word = true;
                cur.push(c);
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    Ok(words)
}

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

    #[test]
    fn splits_quoted_words() {
        let w = split_words(r#"curl -X POST "$BASE_URL/u" -H 'A: b c' -d '{"x":"$y"}'"#).unwrap();
        assert_eq!(
            w,
            vec![
                "curl",
                "-X",
                "POST",
                "$BASE_URL/u",
                "-H",
                "A: b c",
                "-d",
                r#"{"x":"\$y"}"#
            ]
        );
    }

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
        ] {
            let quoted = quote(word);
            let back = split_words(&quoted).unwrap();
            assert_eq!(back, vec![word.to_string()], "quoted as {quoted}");
        }
    }

    #[test]
    fn detects_shell_features() {
        assert_eq!(find_shell_feature("curl a | jq ."), Some("a pipe"));
        assert_eq!(find_shell_feature("curl 'a|b'"), None);
        assert_eq!(
            find_shell_feature("curl \"$(cat x)\""),
            Some("command substitution")
        );
        assert_eq!(find_shell_feature("curl a > out"), Some("redirection"));
        assert_eq!(find_shell_feature("curl a#b"), None);
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
