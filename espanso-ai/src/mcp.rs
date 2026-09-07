//! MCP 2025-06-18 JSON-RPC over stdio. No network listener or implicit clipboard access.
use crate::{load_key, redact, rewrite, validate_text, Settings};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Write},
    path::Path,
};

#[derive(Default)]
pub struct Session {
    initialized: bool,
    ready: bool,
}
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
impl Session {
    pub fn handle(&mut self, root: &Path, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request["method"].as_str();
        if !request.is_object()
            || request["jsonrpc"] != "2.0"
            || method.is_none()
            || id
                .as_ref()
                .is_some_and(|v| !v.is_string() && !v.is_number())
        {
            return Some(error(Value::Null, -32600, "Invalid Request"));
        }
        let method = method.unwrap();
        if id.is_none() {
            if method == "notifications/initialized" && self.initialized {
                self.ready = true;
            }
            return None;
        }
        let id = id.unwrap();
        let result = match method {
            "initialize" if !self.initialized => {
                if !request["params"]["protocolVersion"].is_string()
                    || !request["params"]["capabilities"].is_object()
                    || !request["params"]["clientInfo"].is_object()
                {
                    return Some(error(id, -32602, "Invalid initialization parameters"));
                }
                self.initialized = true;
                json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"respanso-ai","version":env!("CARGO_PKG_VERSION")},"instructions":"prepare_text is local. Before rewrite_text show the exact text and configured context to the user for review. Redaction is heuristic, not guaranteed anonymization. Never read clipboard or patient files implicitly."})
            }
            "ping" => json!({}),
            _ if !self.ready => return Some(error(id, -32000, "Initialize session first")),
            "tools/list" => json!({"tools":[
                {"name":"get_context","description":"Read the masked local rewrite context and provider, never credentials. Show this context with outgoing text before user review.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}},
                {"name":"prepare_text","description":"Locally mask common identifiers. No network. Human review still required.","inputSchema":{"type":"object","properties":{"text":{"type":"string","maxLength":24000}},"required":["text"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}},
                {"name":"rewrite_text","description":"Send user-reviewed text to the configured AI provider. Requires local AI and MCP rewrite opt-in. Never invent clinical facts.","inputSchema":{"type":"object","properties":{"text":{"type":"string","maxLength":24000},"reviewed":{"type":"boolean","const":true}},"required":["text","reviewed"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":true}}
            ]}),
            "tools/call" => {
                let args = &request["params"]["arguments"];
                let name = request["params"]["name"].as_str().unwrap_or_default();
                if name == "get_context" {
                    if !args.is_null() && !args.as_object().is_some_and(|m| m.is_empty()) {
                        return Some(error(id, -32602, "get_context accepts no arguments"));
                    }
                    let result = match Settings::load(root) {
                        Ok(s) => {
                            json!({"content":[{"type":"text","text":json!({"provider":s.provider,"model":s.model,"context":redact(&s.context),"rules":crate::CLINICAL_RULES,"enabled":s.enabled,"mcp_allow_rewrite":s.mcp_allow_rewrite}).to_string()}],"isError":false})
                        }
                        Err(e) => json!({"content":[{"type":"text","text":e}],"isError":true}),
                    };
                    return Some(json!({"jsonrpc":"2.0","id":id,"result":result}));
                }
                if !["prepare_text", "rewrite_text"].contains(&name) {
                    return Some(error(id, -32602, "Unknown tool"));
                }
                let Some(text) = args["text"].as_str() else {
                    return Some(error(id, -32602, "text must be a string"));
                };
                if args.as_object().is_none_or(|m| {
                    m.keys()
                        .any(|k| k != "text" && !(name == "rewrite_text" && k == "reviewed"))
                }) {
                    return Some(error(id, -32602, "Unexpected arguments"));
                }
                let result = validate_text(text).and_then(|_| {
                    if name == "prepare_text" {
                        return Ok(redact(text));
                    }
                    if args["reviewed"] != true {
                        return Err(
                            "Сначала покажите текст и контекст пользователю для проверки".into(),
                        );
                    }
                    let settings = Settings::load(root)?;
                    if !settings.mcp_allow_rewrite {
                        return Err("Отправка через MCP отключена в локальных настройках".into());
                    }
                    rewrite(&settings, &load_key(root, settings.provider)?, text)
                });
                match result {
                    Ok(text) => json!({"content":[{"type":"text","text":text}],"isError":false}),
                    Err(message) => {
                        json!({"content":[{"type":"text","text":message}],"isError":true})
                    }
                }
            }
            _ => return Some(error(id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}

pub fn serve(root: &Path, mut input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    let mut session = Session::default();
    loop {
        // A malformed peer cannot force an unbounded allocation. Close on oversized frames.
        let mut line = String::new();
        let count = std::io::Read::take(&mut input, 131_073).read_line(&mut line)?;
        if count == 0 {
            return Ok(());
        }
        if count > 131_072 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "MCP frame too large",
            ));
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => session.handle(root, request),
            Err(_) => Some(error(Value::Null, -32700, "Parse error")),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_and_privacy_gates() {
        let dir = tempdir::TempDir::new("mcp").unwrap();
        let root = dir.path();
        let mut s = Session::default();
        assert!(s
            .handle(root, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .unwrap()
            .get("error")
            .is_some());
        let init = s.handle(root, json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert!(s
            .handle(
                root,
                json!({"jsonrpc":"2.0","method":"notifications/initialized"})
            )
            .is_none());
        let reply = s.handle(root,json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"rewrite_text","arguments":{"text":"Боль 5 дней","reviewed":true}}})).unwrap();
        assert_eq!(reply["result"]["isError"], true);
        let reply = s.handle(root,json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"prepare_text","arguments":{"text":"test@example.org"}}})).unwrap();
        assert_eq!(reply["result"]["content"][0]["text"], "[СКРЫТО]");
    }
    #[test]
    fn malformed_and_notification_frames() {
        let mut out = Vec::new();
        serve(
            Path::new("."),
            &b"bad\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\"}\n"[..],
            &mut out,
        )
        .unwrap();
        assert_eq!(String::from_utf8(out).unwrap().lines().count(), 1);
    }
}
