//! Human-readable terminal output.

use sankh_core::Outcome;
use sankh_core::report::Summary;
use sankh_core::runner::RequestResult;
use std::io::IsTerminal;
use std::sync::OnceLock;

const BODY_PREVIEW: usize = 2000;

fn color_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal())
}

fn paint(code: &str, s: &str) -> String {
    if color_enabled() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn red(s: &str) -> String {
    paint("31", s)
}
pub fn green(s: &str) -> String {
    paint("32", s)
}
pub fn yellow(s: &str) -> String {
    paint("33", s)
}
pub fn dim(s: &str) -> String {
    paint("2", s)
}
pub fn bold(s: &str) -> String {
    paint("1", s)
}

pub struct Printer {
    /// When the report goes to stdout, human output moves to stderr.
    to_stderr: bool,
    verbose: bool,
}

impl Printer {
    pub fn new(to_stderr: bool, verbose: bool) -> Printer {
        Printer { to_stderr, verbose }
    }

    fn line(&self, s: &str) {
        if self.to_stderr {
            eprintln!("{s}");
        } else {
            println!("{s}");
        }
    }

    pub fn header(&self, collection: &str, env: Option<&str>, count: usize) {
        let env = env.map(|e| format!(" · env {e}")).unwrap_or_default();
        self.line(&format!(
            "{} {}",
            bold(&format!("sankh · {collection}{env}")),
            dim(&format!(
                "({count} request{})",
                if count == 1 { "" } else { "s" }
            ))
        ));
    }

    pub fn note(&self, s: &str) {
        self.line(&dim(s));
    }

    pub fn result(&self, r: &RequestResult) {
        let (mark, name) = match r.outcome {
            Outcome::Passed => (green("✓"), r.name.clone()),
            Outcome::Failed => (red("✗"), red(&r.name)),
            Outcome::Error => (red("!"), red(&r.name)),
            Outcome::Cancelled => (yellow("-"), yellow(&r.name)),
        };
        let mut meta = Vec::new();
        if let Some(resp) = &r.response {
            meta.push(resp.status.to_string());
            meta.push(format!("{:.0}ms", resp.time_ms));
        }
        meta.push(r.path.clone());
        self.line(&format!("{mark} {name}  {}", dim(&meta.join("  "))));
        for w in &r.warnings {
            self.line(&format!("    {} {w}", yellow("warning:")));
        }
        for a in r.assertions.iter().filter(|a| !a.passed) {
            self.line(&format!(
                "    {} {}",
                red(&format!("{}:", a.label)),
                a.message.as_deref().unwrap_or("failed")
            ));
        }
        if let Some(e) = &r.error {
            for l in e.lines() {
                self.line(&format!("    {}", red(l)));
            }
        }
        for c in &r.captures {
            self.line(&format!(
                "    {}",
                dim(&format!("captured {}={}", c.name, c.value))
            ));
        }
        let show_body = self.verbose || r.outcome != Outcome::Passed;
        if show_body {
            if !r.stderr.trim().is_empty() && r.outcome != Outcome::Passed {
                for l in r.stderr.trim().lines().take(20) {
                    self.line(&format!("    {}", dim(l)));
                }
            }
            if let Some(resp) = &r.response {
                let body = resp.body.trim();
                if !body.is_empty() {
                    let preview: String = body.chars().take(BODY_PREVIEW).collect();
                    for l in preview.lines() {
                        self.line(&format!("    {}", dim(&format!("│ {l}"))));
                    }
                    if body.chars().count() > BODY_PREVIEW {
                        self.line(&format!("    {}", dim("│ …")));
                    }
                }
            }
        }
    }

    pub fn summary(&self, s: &Summary) {
        let mut parts = vec![green(&format!("{} passed", s.passed))];
        if s.failed > 0 {
            parts.push(red(&format!("{} failed", s.failed)));
        }
        if s.errors > 0 {
            parts.push(red(&format!("{} errors", s.errors)));
        }
        self.line(&format!(
            "\n{}  {}",
            parts.join(", "),
            dim(&format!("{:.0}ms", s.duration_ms))
        ));
    }
}
