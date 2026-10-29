//! Variable expansion: one pass over a word's segments. Values are inserted
//! as they are and never re-read, so a value cannot inject syntax.

use super::tokenize::{Seg, Word};
use crate::diag::Diag;

/// What a variable resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Value(String),
    /// Known only once the run reaches it (captured by an earlier request),
    /// or not checked at all (static parsing).
    Deferred,
    Missing,
}

/// A variable source.
pub trait Vars {
    fn lookup(&self, name: &str) -> Lookup;
    /// Where to tell the user to define a missing variable.
    fn define_hint(&self) -> String {
        "define it in an environment file, .env.local or the process environment, or capture it from an earlier request".into()
    }
}

/// Defers every variable: used to check a file without an environment.
pub struct Unresolved;

impl Vars for Unresolved {
    fn lookup(&self, _: &str) -> Lookup {
        Lookup::Deferred
    }
}

/// An expanded word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    pub text: String,
    /// Some variable was deferred: the text is incomplete and must not be validated.
    pub deferred: bool,
}

pub fn expand(word: &Word, vars: &dyn Vars) -> Result<Value, Diag> {
    let mut out = Value {
        text: String::new(),
        deferred: false,
    };
    for seg in &word.segs {
        match seg {
            Seg::Lit(t) => out.text.push_str(t),
            Seg::Var {
                name,
                default,
                span,
                ..
            } => match vars.lookup(name) {
                Lookup::Value(v) if v.is_empty() && default.is_some() => {
                    out.text.push_str(default.as_deref().unwrap_or_default())
                }
                Lookup::Value(v) => out.text.push_str(&v),
                Lookup::Deferred => out.deferred = true,
                Lookup::Missing => match default {
                    Some(d) => out.text.push_str(d),
                    None => {
                        return Err(Diag::error(format!("variable {name} is not defined"))
                            .config()
                            .at(*span)
                            .help(vars.define_hint()));
                    }
                },
            },
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tokenize::tokenize;
    use std::collections::BTreeMap;

    struct Map(BTreeMap<&'static str, &'static str>);

    impl Vars for Map {
        fn lookup(&self, name: &str) -> Lookup {
            match self.0.get(name) {
                Some(v) => Lookup::Value(v.to_string()),
                None if name == "LATER" => Lookup::Deferred,
                None => Lookup::Missing,
            }
        }
    }

    fn run(src: &str) -> Result<Value, Diag> {
        let vars = Map(BTreeMap::from([
            ("A", "1"),
            ("EMPTY", ""),
            ("TRICKY", "$A `x` $(y) '\""),
        ]));
        let w = tokenize(src, 0).unwrap().words.remove(0);
        expand(&w, &vars)
    }

    #[test]
    fn expands() {
        let text = |s: &str| run(s).unwrap().text;
        assert_eq!(text("x$A/${A}y"), "x1/1y");
        assert_eq!(text("'$A'\"$A\""), "$A1");
        assert_eq!(text("${NOPE:-d}${EMPTY:-e}${A:-f}"), "de1");
        assert_eq!(text("$EMPTY."), ".");
        assert_eq!(text("\"$TRICKY\""), "$A `x` $(y) '\"");
        assert_eq!(text("\\$A"), "$A");
        assert!(run("$LATER/x").unwrap().deferred);
    }

    #[test]
    fn undefined_is_an_error() {
        let e = run("x/$TOKEN").unwrap_err();
        assert_eq!(e.message, "variable TOKEN is not defined");
        assert_eq!(e.kind, crate::diag::ErrorKind::Config);
        assert_eq!(e.span.map(|s| (s.start, s.end)), Some((2, 8)));
    }
}
