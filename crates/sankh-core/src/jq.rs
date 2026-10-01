//! Thin wrapper around the embedded `jaq` interpreter.

use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Compiler, Ctx, Vars, data, unwrap_valr};
use jaq_json::Val;
use serde_json::Value;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum JqError {
    #[error("invalid jq expression `{0}`")]
    Compile(String),
    #[error("response body is not valid JSON")]
    InvalidJson,
    #[error("jq error: {0}")]
    Runtime(String),
}

/// Checks that an expression parses and compiles, without running it.
pub fn validate(expr: &str) -> Result<(), JqError> {
    compile(expr).map(|_| ())
}

fn compile(expr: &str) -> Result<jaq_core::Filter<data::JustLut<Val>>, JqError> {
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    let loader = Loader::new(defs);
    let arena = Arena::default();
    let program = File {
        code: expr,
        path: (),
    };
    let modules = loader
        .load(&arena, program)
        .map_err(|_| JqError::Compile(expr.to_string()))?;
    Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|_| JqError::Compile(expr.to_string()))
}

/// Runs `expr` against a JSON document given as raw bytes and returns all outputs.
pub fn run(expr: &str, body: &[u8]) -> Result<Vec<Value>, JqError> {
    let filter = compile(expr)?;
    let input = jaq_json::read::parse_single(body).map_err(|_| JqError::InvalidJson)?;
    let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
    let mut out = Vec::new();
    for item in filter.id.run((ctx, input)).map(unwrap_valr) {
        let val = item.map_err(|e| JqError::Runtime(e.to_string()))?;
        let text = val.to_string();
        let value = serde_json::from_str(&text).unwrap_or(Value::String(text));
        out.push(value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runs_simple_paths() {
        let body = br#"{"data":{"id":7,"name":"x","tags":["a","b"]}}"#;
        assert_eq!(run(".data.id", body).unwrap(), vec![json!(7)]);
        assert_eq!(run(".data.name", body).unwrap(), vec![json!("x")]);
        assert_eq!(run(".data.tags | length", body).unwrap(), vec![json!(2)]);
        assert_eq!(
            run(".data.tags[]", body).unwrap(),
            vec![json!("a"), json!("b")]
        );
        assert_eq!(run(".missing", body).unwrap(), vec![json!(null)]);
    }

    #[test]
    fn reports_errors() {
        assert!(matches!(run(".[", b"{}"), Err(JqError::Compile(_))));
        assert_eq!(run(".a", b"not json"), Err(JqError::InvalidJson));
        assert!(validate(".a | length").is_ok());
    }
}
