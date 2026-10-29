//! Finds the single `curl` command in the body of a request file.

use super::tokenize::{self, Word};
use crate::diag::{Diag, Span};

/// The command's words after `curl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub words: Vec<Word>,
    /// The `curl` word.
    pub curl: Span,
    /// From `curl` to the end of the command.
    pub span: Span,
}

/// Extracts the command from `text`, starting at byte `body_start` (after the
/// annotation header). Blank lines and `#` comment lines may surround it.
pub fn extract(text: &str, body_start: usize) -> Result<Command, Diag> {
    let Some(start) = next_content(text, body_start) else {
        return Err(Diag::error("no curl command found")
            .help("add a request line such as `curl -sS \"$BASE_URL/health\"`"));
    };
    let mut start = start;
    if text[start..].starts_with("$ ") {
        start += 2;
        while text[start..].starts_with([' ', '\t']) {
            start += 1;
        }
    }
    let tokens = tokenize::tokenize(text, start)?;
    let Some(first) = tokens.words.first() else {
        return Err(Diag::error("no curl command found"));
    };
    if first.literal().as_deref() != Some("curl") {
        let shown = first.template();
        let mut d = Diag::error("expected a `curl` command").at(first.span);
        d = if shown.contains('=') && !shown.starts_with('-') {
            d.label("variable assignments are not supported")
                .help("define the variable in an environment file or `.env.local`")
        } else if ["wget", "http", "https", "xh"].contains(&shown.as_str()) {
            d.label(format!("`{shown}` is not supported"))
                .help("rewrite the request with curl syntax")
        } else {
            d.label("request files hold exactly one curl command")
        };
        return Err(d);
    }
    if let Some(extra) = next_content(text, tokens.end) {
        let line_end = text[extra..].find('\n').map_or(text.len(), |n| extra + n);
        let mut d = Diag::error("only one command per file is supported")
            .at(Span::new(extra, line_end.max(extra + 1)))
            .label("second command");
        let before = text[..tokens.end].trim_end_matches(['\r']);
        let trimmed = before.trim_end_matches([' ', '\t']);
        if trimmed.len() < before.len() && trimmed.ends_with('\\') {
            d = d.help("the previous line ends with `\\` followed by spaces, which does not continue the line; remove the trailing spaces");
        } else {
            d = d.help("put each request in its own file, or end the previous line with `\\` to continue it");
        }
        return Err(d);
    }
    let mut words = tokens.words;
    let curl = words.remove(0).span;
    Ok(Command {
        words,
        curl,
        span: Span::new(curl.start, tokens.end),
    })
}

/// Offset of the first character of the first line from `from` that is
/// neither blank nor a `#` comment.
fn next_content(text: &str, from: usize) -> Option<usize> {
    let mut at = from;
    while at < text.len() {
        let line_end = text[at..].find('\n').map_or(text.len(), |n| at + n);
        let line = &text[at..line_end];
        let trimmed = line.trim_start();
        if !trimmed.trim_end().is_empty() && !trimmed.starts_with('#') {
            return Some(at + (line.len() - trimmed.len()));
        }
        at = line_end + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn templates(c: &Command) -> Vec<String> {
        c.words.iter().map(Word::template).collect()
    }

    #[test]
    fn finds_the_command() {
        let c = extract("\n# note\ncurl -sS \\\n  \"$U/x\"\n\n# trailing note\n", 0).unwrap();
        assert_eq!(templates(&c), vec!["-sS", "$U/x"]);
        let c = extract("$ curl x", 0).unwrap();
        assert_eq!(templates(&c), vec!["x"]);
    }

    #[test]
    fn errors() {
        assert!(
            extract("\n# only comments\n", 0)
                .unwrap_err()
                .message
                .contains("no curl")
        );
        let e = extract("wget x", 0).unwrap_err();
        assert!(e.label.unwrap().contains("wget"));
        let e = extract("TOKEN=1 curl x", 0).unwrap_err();
        assert!(e.label.unwrap().contains("assignments"));
        let e = extract("curl a\ncurl b\n", 0).unwrap_err();
        assert_eq!(e.span, Some(Span::new(7, 13)));
        let e = extract("curl a \\ \n  -H x\n", 0).unwrap_err();
        assert!(
            e.help.unwrap().contains("trailing spaces"),
            "trailing-space hint"
        );
    }
}
