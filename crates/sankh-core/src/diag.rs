//! Diagnostics: positioned errors and warnings, and their compiler-style
//! rendering.

use crate::request::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

/// Byte range in the request file text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// What kind of failure an error is, which decides the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorKind {
    /// An `@expect` failed or a `@capture` found nothing.
    Assertion,
    /// DNS, connect, TLS, timeout.
    Transport,
    /// Unsupported flag, shell syntax, bad quoting.
    Parse,
    /// Undefined variable, missing `@file`, bad URL.
    Config,
}

impl ErrorKind {
    /// 1 for assertion failures, 2 for parse and config errors, 3 for transport.
    pub fn exit_code(self) -> u8 {
        match self {
            ErrorKind::Assertion => 1,
            ErrorKind::Parse | ErrorKind::Config => 2,
            ErrorKind::Transport => 3,
        }
    }
}

/// A diagnostic before it is placed in a file: positions are byte spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub severity: Severity,
    pub kind: ErrorKind,
    pub message: String,
    pub span: Option<Span>,
    pub label: Option<String>,
    pub help: Option<String>,
    pub note: Option<String>,
}

impl Diag {
    pub fn error(message: impl Into<String>) -> Diag {
        Diag {
            severity: Severity::Error,
            kind: ErrorKind::Parse,
            message: message.into(),
            span: None,
            label: None,
            help: None,
            note: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Diag {
        Diag {
            severity: Severity::Warning,
            ..Diag::error(message)
        }
    }

    pub fn config(mut self) -> Diag {
        self.kind = ErrorKind::Config;
        self
    }

    pub fn at(mut self, span: Span) -> Diag {
        self.span = Some(span);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Diag {
        self.label = Some(label.into());
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Diag {
        self.help = Some(help.into());
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Diag {
        self.note = Some(note.into());
        self
    }

    /// Places the diagnostic in `text`, the whole request file.
    pub fn locate(&self, text: &str) -> Diagnostic {
        let (line, column, width) = match self.span {
            Some(span) => {
                let start = span.start.min(text.len());
                let (line, column) = line_col(text, start);
                let line_end = text[start..].find('\n').map_or(text.len(), |i| start + i);
                let end = span.end.clamp(start, line_end);
                let width = text[start..end].chars().count().max(1);
                (line, Some(column), Some(width))
            }
            None => (0, None, None),
        };
        Diagnostic {
            line,
            severity: self.severity,
            message: self.message.clone(),
            kind: (self.severity == Severity::Error).then_some(self.kind),
            column,
            width,
            label: self.label.clone(),
            help: self.help.clone(),
            note: self.note.clone(),
        }
    }
}

/// 1-based line and character column of byte offset `at`.
pub fn line_col(text: &str, at: usize) -> (usize, usize) {
    let before = &text[..at];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, before[line_start..].chars().count() + 1)
}

/// Renders a diagnostic like a compiler message:
///
/// ```text
/// error: shell syntax is not supported
///   --> users/01-list.sh:5:3
///    |
///  5 | | jq .
///    |   ^ pipes are not supported
///    = help: ...
/// ```
pub fn render(d: &Diagnostic, path: &str, content: &str, color: bool) -> String {
    let paint = |code: &str, s: &str| {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let (word, code) = match d.severity {
        Severity::Error => ("error", "1;31"),
        Severity::Warning => ("warning", "1;33"),
    };
    let mut out = format!("{}: {}\n", paint(code, word), paint("1", &d.message));
    if d.line == 0 {
        out.push_str(&format!("  {} {path}\n", paint("34", "-->")));
    } else {
        let line_text = content
            .lines()
            .nth(d.line - 1)
            .unwrap_or("")
            .trim_end_matches('\r');
        let num = d.line.to_string();
        let pad = " ".repeat(num.len());
        let bar = paint("34", "|");
        match d.column {
            Some(col) => out.push_str(&format!(
                "{pad}{} {path}:{}:{col}\n",
                paint("34", "-->"),
                d.line
            )),
            None => out.push_str(&format!("{pad}{} {path}:{}\n", paint("34", "-->"), d.line)),
        }
        out.push_str(&format!("{pad} {bar}\n"));
        out.push_str(&format!("{} {bar} {line_text}\n", paint("34", &num)));
        if let Some(col) = d.column {
            let carets = "^".repeat(d.width.unwrap_or(1));
            let mut marker = format!("{}{}", " ".repeat(col - 1), paint(code, &carets));
            if let Some(label) = &d.label {
                marker.push(' ');
                marker.push_str(&paint(code, label));
            }
            out.push_str(&format!("{pad} {bar} {marker}\n"));
        }
        for (name, text) in [("help", &d.help), ("note", &d.note)] {
            if let Some(text) = text {
                out.push_str(&format!(
                    "{pad} {} {name}: {}\n",
                    paint("34", "="),
                    indent(text, pad.len() + 4 + name.len())
                ));
            }
        }
        return out;
    }
    for (name, text) in [("help", &d.help), ("note", &d.note)] {
        if let Some(text) = text {
            out.push_str(&format!("  {} {name}: {text}\n", paint("34", "=")));
        }
    }
    out
}

fn indent(text: &str, width: usize) -> String {
    text.replace('\n', &format!("\n{}", " ".repeat(width + 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_like_a_compiler() {
        let text = "# @name x\ncurl -sS \"$U\" \\\n  | jq .\n";
        let d = Diag::error("shell syntax is not supported")
            .at(Span::new(
                text.find('|').unwrap(),
                text.find('|').unwrap() + 1,
            ))
            .label("pipes are not supported")
            .help("use \"# @expect json\" or \"# @capture\" instead")
            .locate(text);
        assert_eq!((d.line, d.column, d.width), (3, Some(3), Some(1)));
        assert_eq!(
            render(&d, "users/01-list.sh", text, false),
            "error: shell syntax is not supported\n --> users/01-list.sh:3:3\n  |\n3 |   | jq .\n  |   ^ pipes are not supported\n  = help: use \"# @expect json\" or \"# @capture\" instead\n"
        );
    }

    #[test]
    fn columns_count_characters() {
        assert_eq!(line_col("é\nab", 4), (2, 2));
    }
}
