//! Hybrid MCP JSON-RPC over stdio: legacy 2025-06-18 plus stateless 2026-07-28.
//! Workspace editing is exposed only to a registered, authenticated local agent.
use crate::{
    agents::{self, AuthenticatedAgent},
    load_key, redact, rewrite, validate_text, workspace, Settings,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Write},
    path::Path,
};

const MAX_MCP_FRAME: u64 = 1_048_576;
const MODERN_PROTOCOL: &str = "2026-07-28";
const LEGACY_PROTOCOL: &str = "2025-06-18";
const INSTRUCTIONS: &str = "prepare_text is local. Workspace tools are visible only to a registered MCP agent authenticated by RESPANSO_MCP_AGENT_ID and RESPANSO_MCP_TOKEN. The workspace is sandboxed to match/**/*.yml|yaml and scripts/**/*.rhai. Writes require per-agent permission, the local master write opt-in, explicit confirmed=true, validation and optimistic concurrency. Redaction is heuristic, not guaranteed anonymization. Never read clipboard or patient files implicitly.";

pub struct Session {
    initialized: bool,
    ready: bool,
    agent: Option<AuthenticatedAgent>,
    auth_error: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            initialized: false,
            ready: false,
            agent: None,
            auth_error: Some("MCP-агент не аутентифицирован".into()),
        }
    }
}

impl Session {
    fn with_auth(result: Result<AuthenticatedAgent, String>) -> Self {
        match result {
            Ok(agent) => Self {
                initialized: false,
                ready: false,
                agent: Some(agent),
                auth_error: None,
            },
            Err(error) => Self {
                initialized: false,
                ready: false,
                agent: None,
                auth_error: Some(error),
            },
        }
    }

    fn workspace_agent(&self) -> Result<&AuthenticatedAgent, String> {
        self.agent.as_ref().ok_or_else(|| {
            self.auth_error
                .clone()
                .unwrap_or_else(|| "MCP-агент не аутентифицирован".into())
        })
    }

    fn workspace_call(&self, root: &Path, name: &str, args: &Value) -> Result<Value, String> {
        let agent = self.workspace_agent()?;
        match name {
            "workspace_list" => {
                require(agent.permissions.read_workspace, "чтение workspace")?;
                let object = object_args(args, &["scope"])?;
                let scope = object.get("scope").and_then(Value::as_str).unwrap_or("all");
                workspace::list(root, scope)
            }
            "workspace_read" => {
                require(agent.permissions.read_workspace, "чтение workspace")?;
                let object = object_args(args, &["path"])?;
                workspace::read(root, required_string(object, "path")?)
            }
            "workspace_validate" => {
                require(agent.permissions.read_workspace, "проверка workspace")?;
                let object = object_args(args, &["path", "content"])?;
                workspace::validate(
                    required_string(object, "path")?,
                    required_text(object, "content")?,
                )
            }
            "workspace_write" => {
                require(agent.permissions.write_workspace, "запись workspace")?;
                let object = object_args(args, &["path", "content", "expected_hash", "confirmed"])?;
                let confirmed = object.get("confirmed").and_then(Value::as_bool).unwrap_or(false);
                let expected_hash = object.get("expected_hash").and_then(Value::as_str);
                let settings = Settings::load(root)?;
                workspace::write(
                    root,
                    required_string(object, "path")?,
                    required_text(object, "content")?,
                    expected_hash,
                    confirmed,
                    settings.mcp_allow_workspace_write,
                )
            }
            "workspace_delete" => {
                require(agent.permissions.delete_workspace, "удаление из workspace")?;
                let object = object_args(args, &["path", "expected_hash", "confirmed"])?;
                let confirmed = object.get("confirmed").and_then(Value::as_bool).unwrap_or(false);
                let settings = Settings::load(root)?;
                workspace::delete(
                    root,
                    required_string(object, "path")?,
                    required_string(object, "expected_hash")?,
                    confirmed,
                    settings.mcp_allow_workspace_write,
                )
            }
            _ => Err("Unknown workspace tool".into()),
        }
    }

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
        let modern = is_modern(&request);
        let result = match method {
            "server/discover" => discover_result(self.agent.as_ref()),
            "initialize" if !self.initialized => {
                if !request["params"]["protocolVersion"].is_string()
                    || !request["params"]["capabilities"].is_object()
                    || !request["params"]["clientInfo"].is_object()
                {
                    return Some(error(id, -32602, "Invalid initialization parameters"));
                }
                self.initialized = true;
                json!({
                    "protocolVersion":LEGACY_PROTOCOL,
                    "capabilities":{"tools":{}},
                    "serverInfo":{"name":"respanso-ai","version":env!("CARGO_PKG_VERSION")},
                    "instructions":INSTRUCTIONS
                })
            }
            "ping" if modern || self.ready => json!({"resultType":"complete"}),
            _ if !modern && !self.ready => return Some(error(id, -32000, "Initialize session first")),
            "tools/list" => tools_result(self.agent.as_ref()),
            "tools/call" => {
                let args = &request["params"]["arguments"];
                let name = request["params"]["name"].as_str().unwrap_or_default();
                if name.starts_with("workspace_") {
                    let result = self
                        .workspace_call(root, name, args)
                        .map(tool_success)
                        .unwrap_or_else(tool_failure);
                    return Some(json!({"jsonrpc":"2.0","id":id,"result":result}));
                }
                if name == "get_context" {
                    if !args.is_null() && !args.as_object().is_some_and(|m| m.is_empty()) {
                        return Some(error(id, -32602, "get_context accepts no arguments"));
                    }
                    let result = match Settings::load(root) {
                        Ok(s) => tool_success(json!({
                            "provider":s.provider,
                            "model":s.model,
                            "context":redact(&s.context),
                            "rules":crate::CLINICAL_RULES,
                            "enabled":s.enabled,
                            "mcp_allow_rewrite":s.mcp_allow_rewrite,
                            "mcp_allow_workspace_write":s.mcp_allow_workspace_write,
                            "authenticated_agent": self.agent.as_ref().map(agent_public)
                        })),
                        Err(e) => tool_failure(e),
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
                    if let Some(agent) = &self.agent {
                        require(agent.permissions.rewrite_ai, "ИИ-переформулировка")?;
                    }
                    if args["reviewed"] != true {
                        return Err("Сначала покажите текст и контекст пользователю для проверки".into());
                    }
                    let settings = Settings::load(root)?;
                    if !settings.mcp_allow_rewrite {
                        return Err("Отправка через MCP отключена в локальных настройках".into());
                    }
                    rewrite(&settings, &load_key(root, settings.provider)?, text)
                });
                match result {
                    Ok(text) => json!({"resultType":"complete","content":[{"type":"text","text":text}],"isError":false}),
                    Err(message) => json!({"resultType":"complete","content":[{"type":"text","text":message}],"isError":true}),
                }
            }
            _ => return Some(error(id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}

fn require(allowed: bool, permission: &str) -> Result<(), String> {
    if allowed {
        Ok(())
    } else {
        Err(format!("У зарегистрированного MCP-агента нет права: {permission}"))
    }
}

fn agent_public(agent: &AuthenticatedAgent) -> Value {
    json!({"id":agent.id,"name":agent.name,"permissions":agent.permissions})
}

fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn tool_success(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    json!({"resultType":"complete","content":[{"type":"text","text":text}],"structuredContent":value,"isError":false})
}

fn tool_failure(message: String) -> Value {
    json!({"resultType":"complete","content":[{"type":"text","text":message}],"isError":true})
}

fn object_args<'a>(args: &'a Value, allowed: &[&str]) -> Result<&'a serde_json::Map<String, Value>, String> {
    let object = args.as_object().ok_or_else(|| "arguments должен быть JSON-объектом".to_owned())?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("Переданы неизвестные аргументы".into());
    }
    Ok(object)
}

fn required_string<'a>(object: &'a serde_json::Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object.get(key).and_then(Value::as_str).filter(|value| !value.is_empty()).ok_or_else(|| format!("{key} должен быть непустой строкой"))
}

fn required_text<'a>(object: &'a serde_json::Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object.get(key).and_then(Value::as_str).ok_or_else(|| format!("{key} должен быть строкой"))
}

fn request_protocol(request: &Value) -> Option<&str> {
    request["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str()
}

fn is_modern(request: &Value) -> bool {
    request_protocol(request) == Some(MODERN_PROTOCOL)
}

fn discover_result(agent: Option<&AuthenticatedAgent>) -> Value {
    json!({
        "resultType":"complete",
        "supportedVersions":[MODERN_PROTOCOL],
        "capabilities":{"tools":{}},
        "_meta":{"io.modelcontextprotocol/serverInfo":{"name":"respanso-ai","version":env!("CARGO_PKG_VERSION"),"description":"rEspanso local AI and protected trigger/script workspace tools"}},
        "authenticatedAgent":agent.map(agent_public),
        "instructions":INSTRUCTIONS,
        "ttlMs":300000,
        "cacheScope":"private"
    })
}

fn tools_result(agent: Option<&AuthenticatedAgent>) -> Value {
    let mut tools = vec![
        json!({"name":"get_context","description":"Read masked local rewrite context and connection status; never credentials.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"prepare_text","description":"Locally mask common identifiers. No network. Human review still required.","inputSchema":{"type":"object","properties":{"text":{"type":"string","maxLength":24000}},"required":["text"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"rewrite_text","description":"Send user-reviewed text to the configured AI provider. Requires local AI opt-in.","inputSchema":{"type":"object","properties":{"text":{"type":"string","maxLength":24000},"reviewed":{"type":"boolean","const":true}},"required":["text","reviewed"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":true}}),
    ];
    if agent.is_some() {
        tools.extend([
            json!({"name":"workspace_list","description":"List editable rEspanso trigger YAML and Rhai script files.","inputSchema":{"type":"object","properties":{"scope":{"type":"string","enum":["all","match","scripts"],"default":"all"}},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
            json!({"name":"workspace_read","description":"Read one UTF-8 file under match/ or scripts/. Returns an optimistic-concurrency hash.","inputSchema":{"type":"object","properties":{"path":{"type":"string","maxLength":512}},"required":["path"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
            json!({"name":"workspace_validate","description":"Validate proposed trigger YAML or Rhai source without writing.","inputSchema":{"type":"object","properties":{"path":{"type":"string","maxLength":512},"content":{"type":"string","maxLength":524288}},"required":["path","content"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
            json!({"name":"workspace_write","description":"Create or replace one validated file under match/ or scripts/. Existing files require expected_hash, confirmed=true and write permission.","inputSchema":{"type":"object","properties":{"path":{"type":"string","maxLength":512},"content":{"type":"string","maxLength":524288},"expected_hash":{"type":"string","maxLength":64},"confirmed":{"type":"boolean","const":true}},"required":["path","content","confirmed"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false}}),
            json!({"name":"workspace_delete","description":"Move one active workspace file to the local MCP trash. Requires expected_hash, confirmed=true and delete permission.","inputSchema":{"type":"object","properties":{"path":{"type":"string","maxLength":512},"expected_hash":{"type":"string","maxLength":64},"confirmed":{"type":"boolean","const":true}},"required":["path","expected_hash","confirmed"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":true,"idempotentHint":false,"openWorldHint":false}}),
        ]);
    }
    json!({"resultType":"complete","tools":tools,"ttlMs":300000,"cacheScope":"private"})
}

pub fn serve(root: &Path, mut input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    let mut session = Session::with_auth(agents::authenticate_from_env(root));
    loop {
        let mut line = String::new();
        let count = std::io::Read::take(&mut input, MAX_MCP_FRAME + 1).read_line(&mut line)?;
        if count == 0 {
            return Ok(());
        }
        if count as u64 > MAX_MCP_FRAME {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "MCP frame too large"));
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
    use crate::agents::AgentPermissions;

    fn legacy_ready(mut session: Session, root: &Path) -> Session {
        let init = session.handle(root, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":LEGACY_PROTOCOL,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], LEGACY_PROTOCOL);
        let _ = session.handle(root, json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        session
    }

    #[test]
    fn unauthenticated_client_cannot_discover_workspace_tools() {
        let dir = tempdir::TempDir::new("mcp-no-agent").unwrap();
        let mut session = legacy_ready(Session::default(), dir.path());
        let result = session.handle(dir.path(), json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert!(!result["result"]["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "workspace_write"));
    }

    #[test]
    fn registered_agent_gets_workspace_tools_but_permissions_are_enforced() {
        let dir = tempdir::TempDir::new("mcp-agent").unwrap();
        std::fs::create_dir_all(dir.path().join("match")).unwrap();
        let permissions = AgentPermissions { read_workspace: true, write_workspace: false, delete_workspace: false, rewrite_ai: false };
        let pairing = agents::register(dir.path(), "agent", permissions).unwrap();
        let auth = agents::authenticate(dir.path(), &pairing.agent.id, &pairing.token).unwrap();
        let mut session = legacy_ready(Session::with_auth(Ok(auth)), dir.path());
        let result = session.handle(dir.path(), json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert!(result["result"]["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "workspace_read"));
        let denied = session.handle(dir.path(), json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"workspace_write","arguments":{"path":"match/a.yml","content":"matches: []\n","confirmed":true}}})).unwrap();
        assert_eq!(denied["result"]["isError"], true);
    }
}
