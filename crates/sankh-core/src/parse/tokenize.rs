//! Splits one curl command into words with POSIX shell quoting rules.
//!
//! This is not a shell. It reads quotes, backslash escapes, line
//! continuations, `$'...'` strings and `$VAR` / `${VAR}` / `${VAR:-default}`
//! references, and rejects everything else a shell would execute (pipes,
//! redirections, command substitution, ...). Globs and `~` stay literal.
//!
//! Variables are recorded as segments and expanded later; a word made from a
//! variable is never split or re-read, so a value cannot inject flags.

use crate::diag::{Diag, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Lit(String),
    Var {
        name: String,
        default: Option<String>,
        /// Written as `${NAME}`.
        braced: bool,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub segs: Vec<Seg>,
    /// Where the word is written, quotes included.
    pub span: Span,
}

impl Word {
    pub fn lit(text: &str, span: Span) -> Word {
        Word {
            segs: if text.is_empty() {
                Vec::new()
            } else {
                vec![Seg::Lit(text.to_string())]
            },
            span,
        }
    }

    /// The text when the word has no variables.
    pub fn literal(&self) -> Option<String> {
        let mut out = String::new();
        for s in &self.segs {
            match s {
                Seg::Lit(t) => out.push_str(t),
                Seg::Var { .. } => return None,
            }
        }
        Some(out)
    }

    /// The literal text before the first variable.
    pub fn literal_prefix(&self) -> &str {
        match self.segs.first() {
            Some(Seg::Lit(t)) => t,
            _ => "",
        }
    }

    pub fn has_vars(&self) -> bool {
        self.segs.iter().any(|s| matches!(s, Seg::Var { .. }))
    }

    pub fn vars(&self) -> impl Iterator<Item = &str> {
        self.segs.iter().filter_map(|s| match s {
            Seg::Var { name, .. } => Some(name.as_str()),
            Seg::Lit(_) => None,
        })
    }

    /// The word without the first `n` bytes of its leading literal (for
    /// values attached to a flag, as in `-XPOST`).
    pub fn tail(&self, n: usize) -> Word {
        let mut segs = self.segs.clone();
        if let Some(Seg::Lit(t)) = segs.first_mut() {
            *t = t[n.min(t.len())..].to_string();
            if t.is_empty() {
                segs.remove(0);
            }
        }
        Word {
            segs,
            span: Span::new((self.span.start + n).min(self.span.end), self.span.end),
        }
    }

    /// The word in Sankh's value convention: `$VAR` expands at run time and a
    /// literal dollar sign is `\$`.
    pub fn template(&self) -> String {
        let mut out = String::new();
        for (i, s) in self.segs.iter().enumerate() {
            match s {
                Seg::Lit(t) => out.push_str(&t.replace('$', "\\$")),
                Seg::Var {
                    name,
                    default,
                    braced,
                    ..
                } => {
                    let word_char_follows = matches!(
                        self.segs.get(i + 1),
                        Some(Seg::Lit(t)) if t.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
                    );
                    match default {
                        Some(d) => out.push_str(&format!("${{{name}:-{d}}}")),
                        None if *braced || word_char_follows => {
                            out.push_str(&format!("${{{name}}}"))
                        }
                        None => out.push_str(&format!("${name}")),
                    }
                }
            }
        }
        out
    }
}

/// Words of the command and the offset where it ends (its final newline, or
/// the end of the text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub words: Vec<Word>,
    pub end: usize,
}

/// Tokenizes the command that starts at byte `start` of `text`.
pub fn tokenize(text: &str, start: usize) -> Result<Tokens, Diag> {
    let mut lx = Lexer {
        text,
        i: start,
        words: Vec::new(),
        segs: Vec::new(),
        lit: String::new(),
        word_start: None,
    };
    let end = lx.run()?;
    Ok(Tokens {
        words: lx.words,
        end,
    })
}

const SHELL: &str = "shell syntax is not supported";

struct Lexer<'a> {
    text: &'a str,
    i: usize,
    words: Vec<Word>,
    segs: Vec<Seg>,
    lit: String,
    word_start: Option<usize>,
}

impl Lexer<'_> {
    fn peek_at(&self, at: usize) -> Option<char> {
        self.text.get(at..).and_then(|s| s.chars().next())
    }

    fn begin(&mut self, at: usize) {
        if self.word_start.is_none() {
            self.word_start = Some(at);
        }
    }

    fn flush_lit(&mut self) {
        if !self.lit.is_empty() {
            self.segs.push(Seg::Lit(std::mem::take(&mut self.lit)));
        }
    }

    fn end_word(&mut self, at: usize) {
        if let Some(start) = self.word_start.take() {
            self.flush_lit();
            self.words.push(Word {
                segs: std::mem::take(&mut self.segs),
                span: Span::new(start, at),
            });
        }
    }

    fn syntax(&self, at: usize, len: usize, label: &str, help: &str) -> Diag {
        let d = Diag::error(SHELL).at(Span::new(at, at + len)).label(label);
        if help.is_empty() { d } else { d.help(help) }
    }

    fn run(&mut self) -> Result<usize, Diag> {
        loop {
            let i = self.i;
            let Some(c) = self.peek_at(i) else {
                self.end_word(i);
                return Ok(i);
            };
            let next = self.peek_at(i + c.len_utf8());
            match c {
                ' ' | '\t' => {
                    self.end_word(i);
                    self.i += 1;
                }
                '\n' => {
                    self.end_word(i);
                    return Ok(i);
                }
                '\r' if next == Some('\n') => {
                    self.end_word(i);
                    return Ok(i);
                }
                '\r' => {
                    self.end_word(i);
                    self.i += 1;
                }
                '\\' => match next {
                    Some('\n') => self.i += 2,
                    Some('\r') if self.peek_at(i + 2) == Some('\n') => self.i += 3,
                    Some(n) => {
                        self.begin(i);
                        self.lit.push(n);
                        self.i += 1 + n.len_utf8();
                    }
                    None => {
                        self.begin(i);
                        self.lit.push('\\');
                        self.i += 1;
                    }
                },
                '\'' => {
                    self.begin(i);
                    let Some(close) = self.text[i + 1..].find('\'') else {
                        return Err(Diag::error("unterminated single quote")
                            .at(Span::new(i, i + 1))
                            .label("this quote is never closed"));
                    };
                    self.lit.push_str(&self.text[i + 1..i + 1 + close]);
                    self.i = i + close + 2;
                }
                '"' => {
                    self.begin(i);
                    self.double_quoted()?;
                }
                '$' => {
                    self.begin(i);
                    self.dollar(false)?;
                }
                '`' => {
                    return Err(self.syntax(
                        i,
                        1,
                        "command substitution is not supported",
                        "define the value in an environment file, or capture it from an earlier request with `# @capture`",
                    ));
                }
                '|' => {
                    return Err(if next == Some('|') {
                        self.syntax(i, 2, "command lists (`||`) are not supported", "")
                    } else {
                        self.syntax(
                            i,
                            1,
                            "pipes are not supported",
                            "use \"# @expect json\" or \"# @capture\" instead",
                        )
                    });
                }
                '&' => {
                    return Err(if next == Some('&') {
                        self.syntax(
                            i,
                            2,
                            "command lists (`&&`) are not supported",
                            "put each request in its own file",
                        )
                    } else {
                        self.syntax(
                            i,
                            1,
                            "background jobs (`&`) are not supported",
                            "quote a URL that contains `&`",
                        )
                    });
                }
                ';' => {
                    return Err(self.syntax(
                        i,
                        1,
                        "multiple commands (`;`) are not supported",
                        "put each request in its own file, or quote the `;`",
                    ));
                }
                '<' => {
                    return Err(if next == Some('<') {
                        self.syntax(
                            i,
                            2,
                            "heredocs are not supported",
                            "put the body in a file and use `--data-binary @file`",
                        )
                    } else {
                        self.syntax(
                            i,
                            1,
                            "input redirection is not supported",
                            "use `--data-binary @file`",
                        )
                    });
                }
                '>' => {
                    return Err(self.syntax(
                        i,
                        1,
                        "output redirection is not supported",
                        "Sankh records the response itself; use \"# @capture\" or \"# @expect\"",
                    ));
                }
                '(' | ')' => {
                    return Err(self.syntax(
                        i,
                        1,
                        "unquoted parentheses are not supported",
                        "quote the value that contains them",
                    ));
                }
                '#' if self.word_start.is_none() => {
                    return Err(Diag::error("comments after the command are not supported")
                        .at(Span::new(i, self.line_end(i)))
                        .label("trailing comment")
                        .help("put notes in `# @description` above the command"));
                }
                c => {
                    self.begin(i);
                    self.lit.push(c);
                    self.i += c.len_utf8();
                }
            }
        }
    }

    fn line_end(&self, from: usize) -> usize {
        self.text[from..]
            .find(['\n', '\r'])
            .map_or(self.text.len(), |n| from + n)
    }

    fn double_quoted(&mut self) -> Result<(), Diag> {
        let open = self.i;
        let mut j = open + 1;
        loop {
            let Some(c) = self.peek_at(j) else {
                return Err(Diag::error("unterminated double quote")
                    .at(Span::new(open, open + 1))
                    .label("this quote is never closed"));
            };
            match c {
                '"' => {
                    self.i = j + 1;
                    return Ok(());
                }
                '\\' => match self.peek_at(j + 1) {
                    Some(n @ ('\\' | '"' | '$' | '`')) => {
                        self.lit.push(n);
                        j += 2;
                    }
                    Some('\n') => j += 2,
                    Some('\r') if self.peek_at(j + 2) == Some('\n') => j += 3,
                    _ => {
                        self.lit.push('\\');
                        j += 1;
                    }
                },
                '$' => {
                    self.i = j;
                    self.dollar(true)?;
                    j = self.i;
                }
                '`' => {
                    return Err(self.syntax(
                        j,
                        1,
                        "command substitution is not supported",
                        "define the value in an environment file, or capture it from an earlier request with `# @capture`",
                    ));
                }
                '\r' if self.peek_at(j + 1) == Some('\n') => j += 1,
                c => {
                    self.lit.push(c);
                    j += c.len_utf8();
                }
            }
        }
    }

    /// Handles `$` at `self.i`.
    fn dollar(&mut self, in_double: bool) -> Result<(), Diag> {
        let at = self.i;
        let next = self.peek_at(at + 1);
        match next {
            Some('{') => self.braced(at),
            Some('(') => {
                let arith = self.peek_at(at + 2) == Some('(');
                Err(self.syntax(
                    at,
                    if arith { 3 } else { 2 },
                    if arith {
                        "arithmetic expansion is not supported"
                    } else {
                        "command substitution `$(...)` is not supported"
                    },
                    "define the value in an environment file, or capture it from an earlier request with `# @capture`",
                ))
            }
            Some('\'') if !in_double => self.ansi_c(at),
            Some('"') if !in_double => Err(Diag::error("`$\"...\"` strings are not supported")
                .at(Span::new(at, at + 2))
                .help("use plain double quotes")),
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                let name: String = self.text[at + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                let end = at + 1 + name.len();
                self.flush_lit();
                self.segs.push(Seg::Var {
                    name,
                    default: None,
                    braced: false,
                    span: Span::new(at, end),
                });
                self.i = end;
                Ok(())
            }
            Some(c) if c.is_ascii_digit() || "@*#?$!-".contains(c) => {
                Err(Diag::error(format!("`${c}` is not supported"))
                    .at(Span::new(at, at + 2))
                    .label("positional and special parameters are not available")
                    .help("use a named variable such as `$BASE_URL`, or write `\\$` for a literal dollar sign"))
            }
            _ => {
                self.lit.push('$');
                self.i = at + 1;
                Ok(())
            }
        }
    }

    fn braced(&mut self, at: usize) -> Result<(), Diag> {
        let Some(close) = self.text[at + 2..].find('}') else {
            return Err(Diag::error("unterminated `${`")
                .at(Span::new(at, at + 2))
                .label("no closing `}`"));
        };
        let end = at + 2 + close + 1;
        let span = Span::new(at, end);
        let inner = &self.text[at + 2..at + 2 + close];
        let name: String = inner
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let unsupported = |what: &str| {
            Diag::error(format!("`${{{inner}}}` is not supported"))
                .at(span)
                .label(what.to_string())
                .help("only `$NAME`, `${NAME}` and `${NAME:-default}` are supported")
        };
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
            return Err(unsupported(if inner.starts_with('#') {
                "length expansion"
            } else {
                "not a variable name"
            }));
        }
        let rest = &inner[name.len()..];
        let default = if rest.is_empty() {
            None
        } else if let Some(d) = rest.strip_prefix(":-") {
            if d.contains(['$', '`']) {
                return Err(unsupported("variables inside a default value"));
            }
            if d.contains(['\'', '"', '\\']) {
                return Err(unsupported(
                    "quotes inside a default value; write the default without them",
                ));
            }
            Some(d.to_string())
        } else {
            return Err(unsupported("parameter operators"));
        };
        self.flush_lit();
        self.segs.push(Seg::Var {
            name,
            default,
            braced: true,
            span,
        });
        self.i = end;
        Ok(())
    }

    /// `$'...'`: backslash escapes as in bash, no expansion.
    fn ansi_c(&mut self, at: usize) -> Result<(), Diag> {
        let mut j = at + 2;
        loop {
            let Some(c) = self.peek_at(j) else {
                return Err(Diag::error("unterminated `$'` string")
                    .at(Span::new(at, at + 2))
                    .label("this quote is never closed"));
            };
            match c {
                '\'' => {
                    self.i = j + 1;
                    return Ok(());
                }
                '\\' => {
                    let Some(e) = self.peek_at(j + 1) else {
                        self.lit.push('\\');
                        j += 1;
                        continue;
                    };
                    j += 1 + e.len_utf8();
                    let simple = match e {
                        'n' => Some('\n'),
                        't' => Some('\t'),
                        'r' => Some('\r'),
                        'a' => Some('\x07'),
                        'b' => Some('\x08'),
                        'e' | 'E' => Some('\x1b'),
                        'f' => Some('\x0c'),
                        'v' => Some('\x0b'),
                        '\\' | '\'' | '"' | '?' => Some(e),
                        _ => None,
                    };
                    if let Some(ch) = simple {
                        self.lit.push(ch);
                        continue;
                    }
                    let (radix, max) = match e {
                        'x' => (16, 2),
                        'u' => (16, 4),
                        'U' => (16, 8),
                        '0'..='7' => (8, 3),
                        _ => {
                            self.lit.push('\\');
                            self.lit.push(e);
                            continue;
                        }
                    };
                    let digits_from = if radix == 8 { j - 1 } else { j };
                    let digits: String = self.text[digits_from..]
                        .chars()
                        .take(max)
                        .take_while(|c| c.is_digit(radix))
                        .collect();
                    if digits.is_empty() {
                        self.lit.push('\\');
                        self.lit.push(e);
                        continue;
                    }
                    j = digits_from + digits.len();
                    let code = u32::from_str_radix(&digits, radix).unwrap_or(0);
                    match e {
                        'x' | '0'..='7' if code < 0x80 => self.lit.push(char::from(code as u8)),
                        'x' | '0'..='7' => {
                            return Err(Diag::error(
                                "non-ASCII byte escapes are not supported in `$'...'`",
                            )
                            .at(Span::new(digits_from - 2, j))
                            .help("write the character itself, or use `\\u` with its code point"));
                        }
                        _ => match char::from_u32(code) {
                            Some(ch) => self.lit.push(ch),
                            None => {
                                return Err(Diag::error(format!(
                                    "invalid code point `\\{e}{digits}`"
                                ))
                                .at(Span::new(digits_from - 2, j)));
                            }
                        },
                    }
                }
                c => {
                    self.lit.push(c);
                    j += c.len_utf8();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(src: &str) -> Vec<String> {
        tokenize(src, 0)
            .unwrap_or_else(|e| panic!("{src}: {e:?}"))
            .words
            .iter()
            .map(Word::template)
            .collect()
    }

    fn err(src: &str) -> Diag {
        tokenize(src, 0).expect_err(src)
    }

    #[test]
    fn quoting() {
        let table: &[(&str, &[&str])] = &[
            ("curl a b", &["curl", "a", "b"]),
            ("  a\t b  ", &["a", "b"]),
            (r#""a"'b'c"#, &["abc"]),
            (r#"'a b' "c d""#, &["a b", "c d"]),
            (r#"'$X' "$X" $X"#, &["\\$X", "$X", "$X"]),
            (r#""${X}y" ${X:-d e}"#, &["${X}y", "${X:-d e}"]),
            (r#"a\ b \$X \"q\""#, &["a b", "\\$X", "\"q\""]),
            (r#""a\"b\\c\$d\e""#, &["a\"b\\c\\$d\\e"]),
            ("a \\\n  b", &["a", "b"]),
            ("a\\\nb", &["ab"]),
            ("a \\\r\n b", &["a", "b"]),
            ("'' \"\"", &["", ""]),
            (
                "*.json ~/x [1-2] {a,b} a#x",
                &["*.json", "~/x", "[1-2]", "{a,b}", "a#x"],
            ),
            (r"$'a\nb\t\x41\u00e9\'\\'", &["a\nb\tAé'\\"]),
            ("'multi\nline'", &["multi\nline"]),
            ("a$ b$", &["a\\$", "b\\$"]),
            ("http://x?a=1#frag", &["http://x?a=1#frag"]),
        ];
        for (src, want) in table {
            assert_eq!(words(src), *want, "{src}");
        }
    }

    #[test]
    fn stops_at_newline() {
        let t = tokenize("curl a\ncurl b", 0).unwrap();
        assert_eq!(t.words.len(), 2);
        assert_eq!(t.end, 6);
    }

    #[test]
    fn spans_cover_quotes() {
        let src = r#"curl "a b" $X"#;
        let t = tokenize(src, 0).unwrap();
        assert_eq!(&src[t.words[1].span.start..t.words[1].span.end], "\"a b\"");
        match &t.words[2].segs[0] {
            Seg::Var { span, .. } => assert_eq!(&src[span.start..span.end], "$X"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rejects_shell_syntax() {
        for (src, label) in [
            ("curl x | jq .", "pipes"),
            ("curl x && curl y", "`&&`"),
            ("curl x; curl y", "`;`"),
            ("curl x > out", "output redirection"),
            ("curl x < in", "input redirection"),
            ("curl x <<EOF", "heredocs"),
            ("curl $(cat x)", "`$(...)`"),
            ("curl \"`cat x`\"", "command substitution"),
            ("curl $((1+2))", "arithmetic"),
            ("curl x &", "background"),
            ("curl (x)", "parentheses"),
        ] {
            let e = err(src);
            assert!(
                e.label.as_deref().unwrap_or("").contains(label),
                "{src}: {e:?}"
            );
        }
        assert!(err("curl x # note").message.contains("comments"));
        assert!(err("curl 'x").message.contains("single quote"));
        assert!(err("curl \"x").message.contains("double quote"));
        assert!(err("curl $1").message.contains("`$1`"));
        assert!(err("curl ${X#a}").message.contains("not supported"));
        assert!(err("curl ${#X}").message.contains("not supported"));
        assert!(err("curl ${X:-$Y}").message.contains("not supported"));
    }

    #[test]
    fn tail_strips_attached_flag() {
        let t = tokenize("-XPOST -H$X", 0).unwrap();
        assert_eq!(t.words[0].tail(2).template(), "POST");
        assert_eq!(t.words[1].tail(2).template(), "$X");
        assert_eq!(t.words[1].tail(2).segs.len(), 1);
    }
}
