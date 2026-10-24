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
pub const TOOL_LIST_YEARS: &str = "list_tax_years";
pub const TOOL_LIST_DRAFTS: &str = "list_form_drafts";
pub const TOOL_GET_DRAFT: &str = "get_form_draft";
pub const TOOL_LIST_FILINGS: &str = "list_filings";
pub const TOOL_QUEUE_ON_DEVICE: &str = "queue_on_device";
pub const TOOL_DEVICE_FILING_STATUS: &str = "device_filing_status";

pub const INSTRUCTIONS: &str = "Read the Buwiz tax profiles this user allowed you on. \
Start with list_tax_profiles. Then read a profile's tax years, form drafts, and filing history. \
queue_on_device is the only write: it leaves a filing for the owner's computer. This server never \
contacts BIR. The owner's app validates the form and uploads it only with a standing OK. \
The full TIN is never given out by the read tools; only its last 4 digits.";

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

/// Most messages one batch may carry: each tool call can load profiles.
pub const MAX_BATCH: usize = 16;

/// Splits a POST body into messages. `Err` is the reply for a body that is
/// not JSON, an empty or oversized batch, or a batch under a protocol
/// version that dropped batching (2025-06-18 and later; `header_version` is
/// the request's `MCP-Protocol-Version`).
pub fn parse_body(body: &[u8], header_version: Option<&str>) -> Result<(Vec<Value>, bool), Value> {
    let value: Value =
        serde_json::from_slice(body).map_err(|_| error(Value::Null, PARSE_ERROR, "parse error"))?;
    match value {
        Value::Array(_) if header_version.is_some_and(|v| v.trim() >= "2025-06-18") => Err(error(
            Value::Null,
            INVALID_REQUEST,
            "batches are not part of this protocol version",
        )),
        Value::Array(items) if items.is_empty() => {
            Err(error(Value::Null, INVALID_REQUEST, "empty batch"))
        }
        Value::Array(items) if items.len() > MAX_BATCH => Err(error(
            Value::Null,
            INVALID_REQUEST,
            &format!("at most {MAX_BATCH} messages per batch"),
        )),
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
            "inputSchema": profile_input(json!({}), &[]),
            "annotations": read_only(),
        }),
        json!({
            "name": TOOL_LIST_YEARS,
            "title": "List tax years",
            "description": "A profile's tax years, newest first: the forms it files each year \
(form code, monthly/quarterly/annual/one-time, active or not) and any per-year registration \
values that differ from the profile's. Needs years.view.",
            "inputSchema": profile_input(json!({}), &[]),
            "annotations": read_only(),
        }),
        json!({
            "name": TOOL_LIST_DRAFTS,
            "title": "List form drafts",
            "description": "A profile's form drafts, newest first, without their contents: form, \
year, period, revision, and whether this revision was reviewed. Needs drafts.view.",
            "inputSchema": profile_input(json!({
                "tax_year": { "type": "integer", "minimum": 2000, "maximum": 2100,
                              "description": "Only this year's drafts." }
            }), &[]),
            "annotations": read_only(),
        }),
        json!({
            "name": TOOL_GET_DRAFT,
            "title": "Get a form draft",
            "description": "One form draft with its contents (the desktop app's form data, as \
saved). Needs drafts.view.",
            "inputSchema": profile_input(json!({
                "draft_id": { "type": "string", "description": "From list_form_drafts." }
            }), &["draft_id"]),
            "annotations": read_only(),
        }),
        json!({
            "name": TOOL_LIST_FILINGS,
            "title": "List filings",
            "description": "A profile's filing history, newest first: queued, submitted, confirmed \
(with the BIR receipt reference), failed, or paid. Needs filings.view. Filing itself is done by \
the taxpayer's installed app, never through this server.",
            "inputSchema": profile_input(json!({}), &[]),
            "annotations": read_only(),
        }),
        json!({
            "name": TOOL_QUEUE_ON_DEVICE,
            "title": "Queue on the owner's computer",
            "description": "Leave a filing for the account owner's computer to send. This server never \
contacts BIR. The owner's app runs that form's Validate before upload and sends only when the owner \
has granted a standing OK for this taxpayer and form. The owner must turn Queue on for this agent \
under Connected agents, or the call is refused and nothing is stored.",
            "inputSchema": profile_input(json!({
                "form_code": { "type": "string", "description": "BIR form code, such as 2551Q." },
                "tax_year": { "type": "integer", "minimum": 1990, "maximum": 2100 },
                "period": { "type": "integer", "minimum": 1, "maximum": 12,
                            "description": "Quarter 1-4 or month 1-12, matching the form." },
                "tin": { "type": "string", "description": "The taxpayer TIN. Its last 4 digits must match the profile." },
                "fields": { "type": "object", "description": "Official box key to value, as bir-mcp would fill them.",
                            "additionalProperties": { "type": ["string", "number", "boolean", "null"] } }
            }), &["form_code", "tax_year", "period", "tin", "fields"]),
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false,
            },
        }),
        json!({
            "name": TOOL_DEVICE_FILING_STATUS,
            "title": "Device filing status",
            "description": "Read a filing you queued with queue_on_device. status is queued until the \
owner's computer picks it up, then picked, invalid (with the form's field errors), or failed. \
This server never contacts BIR.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "From queue_on_device." }
                },
                "required": ["id"],
                "additionalProperties": false,
            },
            "annotations": read_only(),
        }),
    ]
}

/// An input schema with `profile_id` and `extra` properties; `profile_id`
/// and the `also_required` ones must be given.
fn profile_input(extra: Value, also_required: &[&str]) -> Value {
    let mut properties = json!({
        "profile_id": { "type": "string", "description": "From list_tax_profiles." }
    });
    if let (Some(all), Value::Object(extra)) = (properties.as_object_mut(), extra) {
        all.extend(extra);
    }
    let mut required = vec!["profile_id"];
    required.extend_from_slice(also_required);
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
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
        assert_eq!(parse_body(b"{nope", None).unwrap_err()["error"]["code"], PARSE_ERROR);
        assert_eq!(parse_body(b"[]", None).unwrap_err()["error"]["code"], INVALID_REQUEST);
        assert!(parse_body(b"[{\"a\":1}]", None).unwrap().1);
        assert!(parse_body(b"[{\"a\":1}]", Some("2025-03-26")).unwrap().1);
        assert_eq!(
            parse_body(b"[{\"a\":1}]", Some("2025-06-18")).unwrap_err()["error"]["code"],
            INVALID_REQUEST,
            "batching was dropped in 2025-06-18"
        );
        let many = format!("[{}]", vec!["{}"; MAX_BATCH + 1].join(","));
        assert_eq!(parse_body(many.as_bytes(), None).unwrap_err()["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn tools_are_all_read_only() {
        let list = reply(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}));
        let tools = list["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|tool| tool["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            [
                TOOL_LIST_PROFILES, TOOL_GET_PROFILE, TOOL_LIST_YEARS, TOOL_LIST_DRAFTS,
                TOOL_GET_DRAFT, TOOL_LIST_FILINGS, TOOL_QUEUE_ON_DEVICE, TOOL_DEVICE_FILING_STATUS,
            ]
        );
        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            if name == TOOL_QUEUE_ON_DEVICE {
                assert_eq!(tool["annotations"]["readOnlyHint"], false, "{name}");
                let description = tool["description"].as_str().unwrap();
                assert!(description.contains("never"), "{description}");
                assert!(description.to_ascii_lowercase().contains("bir"), "{description}");
            } else {
                assert_eq!(tool["annotations"]["readOnlyHint"], true, "{name}");
            }
            assert_eq!(tool["annotations"]["destructiveHint"], false, "{name}");
            assert_eq!(tool["inputSchema"]["type"], "object");
            assert_eq!(tool["inputSchema"]["additionalProperties"], false, "{}", tool["name"]);
            // Every required argument is described.
            for name in tool["inputSchema"]["required"].as_array().into_iter().flatten() {
                let name = name.as_str().unwrap();
                assert!(tool["inputSchema"]["properties"][name].is_object(), "{name}");
            }
        }
    }

    #[test]
    fn record_tools_need_a_profile_and_a_draft_needs_its_id() {
        let required = |name: &str| -> Vec<String> {
            let tool = tools().into_iter().find(|tool| tool["name"] == name).unwrap();
            match &tool["inputSchema"]["required"] {
                Value::Null => Vec::new(),
                list => serde_json::from_value(list.clone()).unwrap(),
            }
        };
        assert_eq!(required(TOOL_LIST_PROFILES), Vec::<String>::new());
        for name in [TOOL_GET_PROFILE, TOOL_LIST_YEARS, TOOL_LIST_DRAFTS, TOOL_LIST_FILINGS] {
            assert_eq!(required(name), ["profile_id"], "{name}");
        }
        assert_eq!(required(TOOL_GET_DRAFT), ["profile_id", "draft_id"]);
        assert_eq!(
            required(TOOL_QUEUE_ON_DEVICE),
            ["profile_id", "form_code", "tax_year", "period", "tin", "fields"]
        );
        assert_eq!(required(TOOL_DEVICE_FILING_STATUS), ["id"]);
        let drafts = tools().into_iter().find(|tool| tool["name"] == TOOL_LIST_DRAFTS).unwrap();
        assert_eq!(drafts["inputSchema"]["properties"]["tax_year"]["type"], "integer");
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
