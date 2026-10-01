//! `@capture` extraction.

use crate::jq;
use crate::request::CaptureSource;
use crate::runner::Response;
use serde_json::Value;

pub fn extract(source: &CaptureSource, response: &Response) -> Result<String, String> {
    match source {
        CaptureSource::Status => Ok(response.status.to_string()),
        CaptureSource::Header { name } => response
            .header(name)
            .map(str::to_string)
            .ok_or_else(|| format!("response has no `{name}` header")),
        CaptureSource::Jq { expr } => {
            let outputs = jq::run(expr, &response.body).map_err(|e| e.to_string())?;
            match outputs.as_slice() {
                [] | [Value::Null] => Err(format!("`{expr}` is null or missing")),
                [Value::String(s)] => Ok(s.clone()),
                [other] => Ok(other.to_string()),
                many => Err(format!(
                    "`{expr}` produced {} values; expected one",
                    many.len()
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp() -> Response {
        Response {
            status: 201,
            headers: vec![("X-Request-Id".into(), "r-1".into())],
            body: br#"{"data":{"token":"t0k3n","id":7,"obj":{"a":1},"items":[1,2]}}"#.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn extracts_values() {
        let r = resp();
        let jq = |e: &str| CaptureSource::Jq { expr: e.into() };
        assert_eq!(extract(&jq(".data.token"), &r).unwrap(), "t0k3n");
        assert_eq!(extract(&jq(".data.id"), &r).unwrap(), "7");
        assert_eq!(extract(&jq(".data.obj"), &r).unwrap(), r#"{"a":1}"#);
        assert!(extract(&jq(".data.missing"), &r).is_err());
        assert!(extract(&jq(".data.items[]"), &r).is_err());
        assert_eq!(
            extract(
                &CaptureSource::Header {
                    name: "x-request-id".into()
                },
                &r
            )
            .unwrap(),
            "r-1"
        );
        assert_eq!(extract(&CaptureSource::Status, &r).unwrap(), "201");
    }
}
