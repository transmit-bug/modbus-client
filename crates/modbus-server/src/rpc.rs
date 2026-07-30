//! JSON-RPC 2.0 framing: parsing, response/error builders, and standard
//! error codes. Tested at the agreed framing/error-mapper unit seam.

#![allow(dead_code)]

use serde::Serialize;
use serde_json::Value;

/// JSON-RPC protocol version string.
pub const VERSION: &str = "2.0";

/// Standard JSON-RPC error codes.
pub mod code {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
}

/// A parsed JSON-RPC request (a notification when `id` is `None`).
#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Parse a raw JSON-RPC message into a [`ParsedRequest`], or return the
/// `(id, code, message)` needed to build an error response.
pub fn parse(raw: &str) -> Result<ParsedRequest, (Option<Value>, i32, String)> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|_| (None, code::PARSE_ERROR, "Parse error".to_string()))?;
    let obj = value
        .as_object()
        .ok_or((None, code::INVALID_REQUEST, "Invalid Request".to_string()))?;
    if obj.get("jsonrpc").and_then(|v| v.as_str()) != Some(VERSION) {
        return Err((None, code::INVALID_REQUEST, "Invalid Request".to_string()));
    }
    let id = obj.get("id").cloned();
    let method = obj
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or((id.clone(), code::INVALID_REQUEST, "Invalid Request".to_string()))?
        .to_string();
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    Ok(ParsedRequest { id, method, params })
}

/// Build a success response envelope.
pub fn success(id: Option<Value>, result: Value) -> Value {
    serde_json::json!({ "jsonrpc": VERSION, "id": id, "result": result })
}

/// Build an error response envelope.
pub fn error(id: Option<Value>, code: i32, message: impl Into<String>, data: Option<Value>) -> Value {
    let err = ErrorObject {
        code,
        message: message.into(),
        data,
    };
    serde_json::json!({ "jsonrpc": VERSION, "id": id, "error": err })
}

/// Is `method` one this server dispatches?
pub fn is_known_method(method: &str) -> bool {
    matches!(
        method,
        "connection.create"
            | "connection.list"
            | "connection.close"
            | "read.holdingRegisters"
    )
}

/// Validate that `method` is dispatchable, else return the standard error code.
pub fn validate_method(method: &str) -> Result<(), i32> {
    if is_known_method(method) {
        Ok(())
    } else {
        Err(code::METHOD_NOT_FOUND)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_a_valid_request() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"connection.create","params":{"name":"x"}}"#;
        let r = parse(raw).unwrap();
        assert_eq!(r.method, "connection.create");
        assert_eq!(r.id, Some(json!(1)));
        assert_eq!(r.params["name"], "x");
    }

    #[test]
    fn treats_absent_id_as_notification() {
        let raw = r#"{"jsonrpc":"2.0","method":"connection.list"}"#;
        let r = parse(raw).unwrap();
        assert!(r.id.is_none());
    }

    #[test]
    fn success_response_envelope_correlates_id_and_omits_error() {
        let resp = success(Some(json!(7)), json!({"id": "c1"}));
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 7);
        assert_eq!(resp["result"]["id"], "c1");
        assert!(resp.get("error").is_none());
    }

    #[test]
    fn error_response_carries_code_message_and_optional_data() {
        let resp = error(Some(json!(1)), code::INVALID_PARAMS, "bad params", Some(json!(42)));
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["error"]["code"], -32602);
        assert_eq!(resp["error"]["message"], "bad params");
        assert_eq!(resp["error"]["data"], 42);
    }

    #[test]
    fn known_methods_are_dispatchable() {
        for m in [
            "connection.create",
            "connection.list",
            "connection.close",
            "read.holdingRegisters",
        ] {
            assert!(is_known_method(m), "{m:?} should be known");
            assert!(validate_method(m).is_ok());
        }
    }

    #[test]
    fn unknown_method_maps_to_method_not_found() {
        assert_eq!(validate_method("bogus.method"), Err(code::METHOD_NOT_FOUND));
    }

    #[test]
    fn bad_json_is_a_parse_error() {
        let (_, c, _) = parse("{not json").unwrap_err();
        assert_eq!(c, code::PARSE_ERROR);
    }

    #[test]
    fn missing_method_is_an_invalid_request() {
        let (_, c, _) = parse(r#"{"jsonrpc":"2.0","id":1}"#).unwrap_err();
        assert_eq!(c, code::INVALID_REQUEST);
    }

    #[test]
    fn wrong_version_is_an_invalid_request() {
        let raw = r#"{"jsonrpc":"1.0","id":1,"method":"x"}"#;
        let (_, c, _) = parse(raw).unwrap_err();
        assert_eq!(c, code::INVALID_REQUEST);
    }
}
