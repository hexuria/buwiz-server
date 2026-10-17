//! MCP over Streamable HTTP, the parts that need no I/O: JSON-RPC parsing,
//! version negotiation, the tool list, and result shapes.
//!
//! The endpoint is stateless: no `Mcp-Session-Id`, no server-sent events.
//! Every POST gets one JSON answer (or 202 when it held only notifications).

use serde_json::{json, Value};

/// Newest first. The first one is offered when a client asks for a version
/// this server does not know.
pub const SUPPORTED_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

pub const TOOL_LIST_PROFILES: &str = "list_tax_profiles";
pub const TOOL_GET_PROFILE: &str = "get_tax_profile";

pub const INSTRUCTIONS: &str = "Read-only access to the Buwiz tax profiles this user allowed you on. \
Start with list_tax_profiles to see them and what you may do on each. You cannot change data, \
publish to BIR, or share profiles. The full TIN is never given out; only its last 4 digits.";

/// What one JSON-RPC message asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Routed {
    /// Answer already known.
    Reply(Value),
    /// A notification or a response from the client: nothing to send back.
    Silent,
    /// Run a tool, then answer with [`tool_result`] or [`tool_error`].
    CallTool {
        id: Value,
        name: String,
        arguments: Value,
    },
}

/// Whether a `MCP-Protocol-Version` header value is acceptable. A request
/// without one is taken to speak 2025-03-26, which is supported.
pub fn header_version_supported(value: Option<&str>) -> bool {
    value.is_none_or(|v| SUPPORTED_VERSIONS.contains(&v.trim()))
}

/// The version to answer `initialize` with: the client's own when supported,
/// otherwise the newest this server speaks.
pub fn negotiate(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|r| SUPPORTED_VERSIONS.iter().find(|v| **v == r))
        .copied()
        .unwrap_or(SUPPORTED_VERSIONS[0])
}

/// Splits a POST body into messages. `Err` is the reply for a body that is
/// not JSON or is an empty batch.
pub fn parse_body(body: &[u8]) -> Result<(Vec<Value>, bool), Value> {
    let value: Value =
        serde_json::from_slice(body).map_err(|_| error(Value::Null, PARSE_ERROR, "parse error"))?;
    match value {
        Value::Array(items) if items.is_empty() => {
            Err(error(Value::Null, INVALID_REQUEST, "empty batch"))
        }
        Value::Array(items) => Ok((items, true)),
        other => Ok((vec![other], false)),
    }
}

/// Routes one message.
pub fn route(message: &Value, server_version: &str) -> Routed {
    let Some(object) = message.as_object() else {
        return Routed::Reply(error(Value::Null, INVALID_REQUEST, "message must be an object"));
    };
    let id = object.get("id").cloned();
    let method = object.get("method").and_then(Value::as_str);
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Routed::Reply(error(id.unwrap_or(Value::Null), INVALID_REQUEST, "jsonrpc must be \"2.0\""));
    }
    let Some(method) = method else {
        // A response to something we sent; we never send requests.
        return Routed::Silent;
    };
    let Some(id) = id else {
        // Notifications (initialized, cancelled, ...) need no answer.
        return Routed::Silent;
    };
    if !(id.is_string() || id.is_number()) {
        return Routed::Reply(error(Value::Null, INVALID_REQUEST, "id must be a string or number"));
    }
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "initialize" => {
            let requested = params.get("protocolVersion").and_then(Value::as_str);
            Routed::Reply(success(
                id,
                json!({
                    "protocolVersion": negotiate(requested),
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "buwiz-tax-profiles",
                        "title": "Buwiz tax profiles",
                        "version": server_version,
                    },
                    "instructions": INSTRUCTIONS,
                }),
            ))
        }
        "ping" => Routed::Reply(success(id, json!({}))),
        "tools/list" => Routed::Reply(success(id, json!({ "tools": tools() }))),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Routed::Reply(error(id, INVALID_PARAMS, "params.name is required"));
            };
            if !tools().iter().any(|tool| tool["name"] == name) {
                return Routed::Reply(error(id, INVALID_PARAMS, &format!("unknown tool: {name}")));
            }
            Routed::CallTool {
                id,
                name: name.to_owned(),
                arguments: params.get("arguments").cloned().unwrap_or_else(|| json!({})),
            }
        }
        _ => Routed::Reply(error(id, METHOD_NOT_FOUND, &format!("method not found: {method}"))),
    }
}

fn read_only() -> Value {
    json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false,
    })
}

pub fn tools() -> Vec<Value> {
    let permissions = json!({ "type": "array", "items": { "type": "string" } });
    vec![
        json!({
            "name": TOOL_LIST_PROFILES,
            "title": "List tax profiles",
            "description": "The tax profiles you were allowed on, with what you may do on each right now. \
A profile with no permissions is still connected but out of reach (for example, it moved to someone else).",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "app_name": { "type": "string" },
                    "profiles": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "profile_id": { "type": "string" },
                                "name": { "type": "string" },
                                "tin_last4": { "type": "string" },
                                "branch_code": { "type": "string" },
                                "taxpayer_type": { "type": "string" },
                                "permissions": permissions,
                            },
                            "required": ["profile_id", "name", "permissions"],
                        },
                    },
                },
                "required": ["profiles"],
            },
            "annotations": read_only(),
        }),
        json!({
            "name": TOOL_GET_PROFILE,
            "title": "Get a tax profile",
            "description": "One tax profile's registration details: name, RDO, address, contact, \
taxpayer type, classification, VAT status. Needs the profile.view permission. The TIN is shown only as its last 4 digits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "profile_id": { "type": "string", "description": "From list_tax_profiles." }
                },
                "required": ["profile_id"],
                "additionalProperties": false,
            },
            "annotations": read_only(),
        }),
    ]
}

pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool that ran: the data as text (older clients) and as
/// `structuredContent`.
pub fn tool_result(id: Value, data: Value) -> Value {
    let text = serde_json::to_string_pretty(&data).unwrap_or_default();
    success(
        id,
        json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": data,
            "isError": false,
        }),
    )
}

/// A tool that could not do what was asked (not allowed, bad arguments).
/// Reported inside the result so the model can read it and adjust.
pub fn tool_error(id: Value, message: &str) -> Value {
    success(
        id,
        json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(message: Value) -> Value {
        match route(&message, "test") {
            Routed::Reply(value) => value,
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn initialize_echoes_a_known_version_and_offers_the_newest_otherwise() {
        let known = reply(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}));
        assert_eq!(known["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(known["result"]["capabilities"]["tools"]["listChanged"], false);
        let unknown = reply(json!({"jsonrpc":"2.0","id":"a","method":"initialize","params":{"protocolVersion":"1999-01-01"}}));
        assert_eq!(unknown["result"]["protocolVersion"], SUPPORTED_VERSIONS[0]);
        assert_eq!(unknown["id"], "a");
    }

    #[test]
    fn notifications_and_client_responses_get_no_answer() {
        assert_eq!(route(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}), "t"), Routed::Silent);
        assert_eq!(route(&json!({"jsonrpc":"2.0","id":3,"result":{}}), "t"), Routed::Silent);
    }

    #[test]
    fn bad_messages_get_json_rpc_errors() {
        assert_eq!(reply(json!({"id":1,"method":"ping"}))["error"]["code"], INVALID_REQUEST);
        assert_eq!(reply(json!({"jsonrpc":"2.0","id":null,"method":"ping"}))["error"]["code"], INVALID_REQUEST);
        assert_eq!(reply(json!({"jsonrpc":"2.0","id":1,"method":"resources/list"}))["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(reply(json!([1]))["error"]["code"], INVALID_REQUEST);
        assert_eq!(parse_body(b"{nope").unwrap_err()["error"]["code"], PARSE_ERROR);
        assert_eq!(parse_body(b"[]").unwrap_err()["error"]["code"], INVALID_REQUEST);
        assert!(parse_body(b"[{\"a\":1}]").unwrap().1);
    }

    #[test]
    fn tools_are_all_read_only() {
        let list = reply(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}));
        let tools = list["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        for tool in tools {
            assert_eq!(tool["annotations"]["readOnlyHint"], true, "{}", tool["name"]);
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn tool_calls_are_routed_and_unknown_tools_refused() {
        assert_eq!(
            route(&json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"get_tax_profile","arguments":{"profile_id":"p"}}}), "t"),
            Routed::CallTool { id: json!(7), name: "get_tax_profile".into(), arguments: json!({"profile_id":"p"}) }
        );
        let unknown = reply(json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"file_return"}}));
        assert_eq!(unknown["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn protocol_version_header() {
        assert!(header_version_supported(None));
        assert!(header_version_supported(Some("2025-06-18")));
        assert!(!header_version_supported(Some("2024-11-05")));
    }

    #[test]
    fn tool_results_carry_text_and_structure() {
        let ok = tool_result(json!(1), json!({"a":1}));
        assert_eq!(ok["result"]["structuredContent"]["a"], 1);
        assert_eq!(ok["result"]["isError"], false);
        let bad = tool_error(json!(1), "not allowed");
        assert_eq!(bad["result"]["isError"], true);
    }
}
