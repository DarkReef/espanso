//! Hybrid MCP JSON-RPC over stdio: legacy 2025-06-18 plus stateless 2026-07-28.
//! Workspace editing and provider-backed rewrites are exposed only to a registered agent.
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
const INSTRUCTIONS: &str = "prepare_text is local. Workspace tools and rewrite_text are visible only to a registered MCP agent authenticated by RESPANSO_MCP_AGENT_ID and RESPANSO_MCP_TOKEN. The workspace is sandboxed to match/**/*.yml|yaml and scripts/**/*.rhai. Writes require per-agent permission, the local master write opt-in, explicit confirmed=true, validation and optimistic concurrency. Agent authorization is revalidated for every protected call. Redaction is heuristic, not guaranteed anonymization. Never read clipboard or patient files implicitly.";

#[derive(Debug, Clone)]
struct AgentCredentials {
    id: String,
    token: String,
}

pub struct Session {
    initialized: bool,
    ready: bool,
    credentials: Option<AgentCredentials>,
    auth_error: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            initialized: false,
            ready: false,
            credentials: None,
            auth_error: Some("MCP-агент не аутентифицирован".into()),
        }
    }
}

impl Session {
    fn with_credentials(root: &Path, id: String, token: String) -> Self {
        let auth_error = agents::authenticate(root, &id, &token).err();
        Self {
            initialized: false,
            ready: false,
            credentials: Some(AgentCredentials { id, token }),
            auth_error,
        }
    }

    fn from_env(root: &Path) -> Self {
        let id = match std::env::var("RESPANSO_MCP_AGENT_ID") {
            Ok(value) if !value.trim().is_empty() => value.trim().to_owned(),
            _ => {
                return Self {
                    auth_error: Some("Не задан RESPANSO_MCP_AGENT_ID".into()),
                    ..Self::default()
                }
            }
        };
        let token = match std::env::var("RESPANSO_MCP_TOKEN") {
            Ok(value) if !value.trim().is_empty() => value.trim().to_owned(),
            _ => {
                return Self {
                    auth_error: Some("Не задан RESPANSO_MCP_TOKEN".into()),
                    ..Self::default()
                }
            }
        };
        Self::with_credentials(root, id, token)
    }

    fn current_agent(&self, root: &Path) -> Result<AuthenticatedAgent, String> {
        let credentials = self.credentials.as_ref().ok_or_else(|| {
            self.auth_error
                .clone()
                .unwrap_or_else(|| "MCP-агент не аутентифицирован".into())
        })?;
        agents::authenticate(root, &credentials.id, &credentials.token)
    }

    fn workspace_call(&self, root: &Path, name: &str, args: &Value) -> Result<Value, String> {
        let agent = self.current_agent(root)?;
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
                workspace::read(root, required_path(object, "path")?)
            }
            "workspace_validate" => {
                require(agent.permissions.read_workspace, "проверка workspace")?;
                let object = object_args(args, &["path", "content"])?;
                workspace::validate(
                    required_path(object, "path")?,
                    required_text(object, "content")?,
                )
            }
            "workspace_write" => {
                let object = object_args(args, &["path", "content", "expected_hash", "confirmed"])?;
                let path = required_path(object, "path")?;
                if let Err(error) = require(agent.permissions.write_workspace, "запись workspace") {
                    let _ = crate::audit::record(
                        root,
                        &agent.id,
                        &agent.name,
                        "write",
                        &safe_audit_path(path),
                        false,
                    );
                    return Err(error);
                }
                let confirmed = object.get("confirmed").and_then(Value::as_bool).unwrap_or(false);
                let expected_hash = object.get("expected_hash").and_then(Value::as_str);
                let settings = Settings::load(root)?;
                workspace::write(
                    root,
                    path,
                    required_text(object, "content")?,
                    expected_hash,
                    confirmed,
                    settings.mcp_allow_workspace_write,
                )
            }
            "workspace_delete" => {
                let object = object_args(args, &["path", "expected_hash", "confirmed"])?;
                let path = required_path(object, "path")?;
                if let Err(error) = require(agent.permissions.delete_workspace, "удаление из workspace") {
                    let _ = crate::audit::record(
                        root,
                        &agent.id,
                        &agent.name,
                        "delete",
                        &safe_audit_path(path),
                        false,
                    );
                    return Err(error);
                }
                let confirmed = object.get("confirmed").and_then(Value::as_bool).unwrap_or(false);
                let settings = Settings::load(root)?;
                workspace::delete(
                    root,
                    path,
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
                .is_some_and(|value| !value.is_string() && !value.is_number())
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

        if let Some(version) = request_protocol(&request) {
            if version != MODERN_PROTOCOL {
                return Some(error(id, -32022, "Unsupported protocol version"));
            }
            if let Err(message) = validate_modern_envelope(&request) {
                return Some(error(id, -32602, &message));
            }
        }
        let modern = is_modern(&request);

        if method == "server/discover" && !modern {
            return Some(error(id, -32601, "Method not found"));
        }
        if method == "initialize" && modern {
            return Some(error(id, -32601, "Method not found"));
        }

        let current_agent = self.current_agent(root).ok();
        let result = match method {
            "server/discover" => discover_result(current_agent.as_ref()),
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
            "ping" if !modern && self.ready => json!({}),
            _ if !modern && !self.ready => {
                return Some(error(id, -32000, "Initialize session first"));
            }
            "tools/list" => tools_result(current_agent.as_ref(), modern),
            "tools/call" => {
                let args = &request["params"]["arguments"];
                let name = request["params"]["name"].as_str().unwrap_or_default();
                if name.starts_with("workspace_") {
                    let result = self
                        .workspace_call(root, name, args)
                        .map(|value| tool_success(value, modern))
                        .unwrap_or_else(|message| tool_failure(message, modern));
                    return Some(response(id, result, modern));
                }
                if name == "get_context" {
                    if !args.is_null() && !args.as_object().is_some_and(|map| map.is_empty()) {
                        return Some(error(id, -32602, "get_context accepts no arguments"));
                    }
                    let result = match Settings::load(root) {
                        Ok(settings) => tool_success(
                            json!({
                                "provider":settings.provider,
                                "model":settings.model,
                                "context":redact(&settings.context),
                                "rules":crate::CLINICAL_RULES,
                                "enabled":settings.enabled,
                                "mcp_allow_rewrite":settings.mcp_allow_rewrite,
                                "mcp_allow_workspace_write":settings.mcp_allow_workspace_write,
                                "authenticated_agent":current_agent.as_ref().map(agent_public)
                            }),
                            modern,
                        ),
                        Err(message) => tool_failure(message, modern),
                    };
                    return Some(response(id, result, modern));
                }
                if !["prepare_text", "rewrite_text"].contains(&name) {
                    return Some(error(id, -32602, "Unknown tool"));
                }
                let Some(text) = args["text"].as_str() else {
                    return Some(error(id, -32602, "text must be a string"));
                };
                if args.as_object().is_none_or(|map| {
                    map.keys().any(|key| {
                        key != "text" && !(name == "rewrite_text" && key == "reviewed")
                    })
                }) {
                    return Some(error(id, -32602, "Unexpected arguments"));
                }
                let result = validate_text(text).and_then(|_| {
                    if name == "prepare_text" {
                        return Ok(redact(text));
                    }
                    let agent = self.current_agent(root)?;
                    require(agent.permissions.rewrite_ai, "ИИ-переформулировка")?;
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
                    Ok(text) => text_tool_result(text, false, modern),
                    Err(message) => text_tool_result(message, true, modern),
                }
            }
            _ => return Some(error(id, -32601, "Method not found")),
        };
        Some(response(id, result, modern))
    }
}

fn require(allowed: bool, permission: &str) -> Result<(), String> {
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "У зарегистрированного MCP-агента нет права: {permission}"
        ))
    }
}

fn agent_public(agent: &AuthenticatedAgent) -> Value {
    json!({"id":agent.id,"name":agent.name,"permissions":agent.permissions})
}

fn server_info() -> Value {
    json!({
        "name":"respanso-ai",
        "version":env!("CARGO_PKG_VERSION"),
        "description":"rEspanso local AI and protected trigger/script workspace tools"
    })
}

fn response(id: Value, mut result: Value, modern: bool) -> Value {
    if modern {
        if let Some(object) = result.as_object_mut() {
            let meta = object.entry("_meta").or_insert_with(|| json!({}));
            if let Some(meta_object) = meta.as_object_mut() {
                meta_object.insert(
                    "io.modelcontextprotocol/serverInfo".into(),
                    server_info(),
                );
            }
        }
    }
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn tool_success(value: Value, modern: bool) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    if modern {
        json!({"resultType":"complete","content":[{"type":"text","text":text}],"structuredContent":value,"isError":false})
    } else {
        json!({"content":[{"type":"text","text":text}],"structuredContent":value,"isError":false})
    }
}

fn tool_failure(message: String, modern: bool) -> Value {
    if modern {
        json!({"resultType":"complete","content":[{"type":"text","text":message}],"isError":true})
    } else {
        json!({"content":[{"type":"text","text":message}],"isError":true})
    }
}

fn text_tool_result(text: String, is_error: bool, modern: bool) -> Value {
    if modern {
        json!({"resultType":"complete","content":[{"type":"text","text":text}],"isError":is_error})
    } else {
        json!({"content":[{"type":"text","text":text}],"isError":is_error})
    }
}

fn object_args<'a>(
    args: &'a Value,
    allowed: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, String> {
    let object = args
        .as_object()
        .ok_or_else(|| "arguments должен быть JSON-объектом".to_owned())?;
    if object
        .keys()
        .any(|key| !allowed.contains(&key.as_str()))
    {
        return Err("Переданы неизвестные аргументы".into());
    }
    Ok(object)
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{key} должен быть непустой строкой"))
}

fn required_path<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    let value = required_string(object, key)?;
    if value.len() > 512 || value.chars().any(char::is_control) {
        return Err(format!("{key} содержит недопустимый или слишком длинный путь"));
    }
    Ok(value)
}

fn required_text<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} должен быть строкой"))
}

fn safe_audit_path(path: &str) -> String {
    if path.len() <= 512
        && !path.chars().any(char::is_control)
        && (path.starts_with("match/") || path.starts_with("scripts/"))
    {
        path.to_owned()
    } else {
        "<invalid-path>".into()
    }
}

fn request_protocol(request: &Value) -> Option<&str> {
    request["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str()
}

fn is_modern(request: &Value) -> bool {
    request_protocol(request) == Some(MODERN_PROTOCOL)
}

fn validate_modern_envelope(request: &Value) -> Result<(), String> {
    let meta = request["params"]["_meta"]
        .as_object()
        .ok_or_else(|| "Modern MCP request requires params._meta".to_owned())?;
    if meta
        .get("io.modelcontextprotocol/protocolVersion")
        .and_then(Value::as_str)
        != Some(MODERN_PROTOCOL)
    {
        return Err("Некорректная версия modern MCP".into());
    }
    if !meta
        .get("io.modelcontextprotocol/clientCapabilities")
        .is_some_and(Value::is_object)
    {
        return Err("Modern MCP request requires clientCapabilities".into());
    }
    if let Some(client_info) = meta.get("io.modelcontextprotocol/clientInfo") {
        if !client_info.is_object() {
            return Err("Modern MCP clientInfo должен быть объектом".into());
        }
    }
    Ok(())
}

fn discover_result(agent: Option<&AuthenticatedAgent>) -> Value {
    let mut meta = serde_json::Map::new();
    if let Some(agent) = agent {
        meta.insert("io.respanso/authenticatedAgent".into(), agent_public(agent));
    }
    json!({
        "resultType":"complete",
        "supportedVersions":[MODERN_PROTOCOL,LEGACY_PROTOCOL],
        "capabilities":{"tools":{}},
        "_meta":meta,
        "instructions":INSTRUCTIONS,
        "ttlMs":300000,
        "cacheScope":"private"
    })
}

fn tools_result(agent: Option<&AuthenticatedAgent>, modern: bool) -> Value {
    let mut tools = vec![
        json!({"name":"get_context","description":"Read masked local rewrite context and connection status; never credentials.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"prepare_text","description":"Locally mask common identifiers. No network. Human review still required.","inputSchema":{"type":"object","properties":{"text":{"type":"string","maxLength":24000}},"required":["text"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
    ];
    if let Some(agent) = agent {
        if agent.permissions.rewrite_ai {
            tools.push(json!({"name":"rewrite_text","description":"Send user-reviewed text to the configured AI provider. Requires registered-agent permission and local AI opt-in.","inputSchema":{"type":"object","properties":{"text":{"type":"string","maxLength":24000},"reviewed":{"type":"boolean","const":true}},"required":["text","reviewed"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":true}}));
        }
        if agent.permissions.read_workspace {
            tools.extend([
                json!({"name":"workspace_list","description":"List editable rEspanso trigger YAML and Rhai script files.","inputSchema":{"type":"object","properties":{"scope":{"type":"string","enum":["all","match","scripts"],"default":"all"}},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
                json!({"name":"workspace_read","description":"Read one UTF-8 file under match/ or scripts/. Returns an optimistic-concurrency hash.","inputSchema":{"type":"object","properties":{"path":{"type":"string","maxLength":512}},"required":["path"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
                json!({"name":"workspace_validate","description":"Validate proposed trigger YAML or Rhai source without writing.","inputSchema":{"type":"object","properties":{"path":{"type":"string","maxLength":512},"content":{"type":"string","maxLength":524288}},"required":["path","content"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
            ]);
        }
        if agent.permissions.write_workspace {
            tools.push(json!({"name":"workspace_write","description":"Create or replace one validated file under match/ or scripts/. Existing files require expected_hash, confirmed=true and write permission.","inputSchema":{"type":"object","properties":{"path":{"type":"string","maxLength":512},"content":{"type":"string","maxLength":524288},"expected_hash":{"type":"string","maxLength":64},"confirmed":{"type":"boolean","const":true}},"required":["path","content","confirmed"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":true,"idempotentHint":true,"openWorldHint":false}}));
        }
        if agent.permissions.delete_workspace {
            tools.push(json!({"name":"workspace_delete","description":"Move one active workspace file to the local MCP trash. Requires expected_hash, confirmed=true and delete permission.","inputSchema":{"type":"object","properties":{"path":{"type":"string","maxLength":512},"expected_hash":{"type":"string","maxLength":64},"confirmed":{"type":"boolean","const":true}},"required":["path","expected_hash","confirmed"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":true,"idempotentHint":false,"openWorldHint":false}}));
        }
    }
    if modern {
        json!({"resultType":"complete","tools":tools,"ttlMs":300000,"cacheScope":"private"})
    } else {
        json!({"tools":tools})
    }
}

pub fn serve(root: &Path, mut input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    let mut session = Session::from_env(root);
    loop {
        let mut line = String::new();
        let count = std::io::Read::take(&mut input, MAX_MCP_FRAME + 1).read_line(&mut line)?;
        if count == 0 {
            return Ok(());
        }
        if count as u64 > MAX_MCP_FRAME {
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
    use crate::agents::AgentPermissions;

    fn legacy_ready(mut session: Session, root: &Path) -> Session {
        let init = session
            .handle(
                root,
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":LEGACY_PROTOCOL,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
            )
            .unwrap();
        assert_eq!(init["result"]["protocolVersion"], LEGACY_PROTOCOL);
        let _ = session.handle(
            root,
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        );
        session
    }

    fn modern_request(id: i64, method: &str, params: Value) -> Value {
        let mut params = params.as_object().cloned().unwrap_or_default();
        params.insert(
            "_meta".into(),
            json!({
                "io.modelcontextprotocol/protocolVersion":MODERN_PROTOCOL,
                "io.modelcontextprotocol/clientCapabilities":{},
                "io.modelcontextprotocol/clientInfo":{"name":"test","version":"1"}
            }),
        );
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
    }

    #[test]
    fn legacy_ping_is_empty_and_modern_ping_is_not_supported() {
        let dir = tempdir::TempDir::new("mcp-ping").unwrap();
        let mut legacy = legacy_ready(Session::default(), dir.path());
        let ping = legacy
            .handle(
                dir.path(),
                json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
            )
            .unwrap();
        assert_eq!(ping["result"], json!({}));

        let mut modern = Session::default();
        let ping = modern
            .handle(dir.path(), modern_request(3, "ping", json!({})))
            .unwrap();
        assert_eq!(ping["error"]["code"], -32601);
    }

    #[test]
    fn modern_discovery_advertises_both_eras() {
        let dir = tempdir::TempDir::new("mcp-discover").unwrap();
        let mut session = Session::default();
        let result = session
            .handle(
                dir.path(),
                modern_request(1, "server/discover", json!({})),
            )
            .unwrap();
        assert_eq!(result["result"]["resultType"], "complete");
        assert!(result["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|version| version == MODERN_PROTOCOL));
        assert!(result["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|version| version == LEGACY_PROTOCOL));
        assert!(result["result"]["_meta"]["io.modelcontextprotocol/serverInfo"].is_object());
    }

    #[test]
    fn legacy_and_modern_tool_lists_have_era_correct_shapes() {
        let dir = tempdir::TempDir::new("mcp-tools-shape").unwrap();
        let mut legacy = legacy_ready(Session::default(), dir.path());
        let legacy_result = legacy
            .handle(
                dir.path(),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            )
            .unwrap();
        assert!(legacy_result["result"].get("resultType").is_none());

        let mut modern = Session::default();
        let modern_result = modern
            .handle(
                dir.path(),
                modern_request(3, "tools/list", json!({})),
            )
            .unwrap();
        assert_eq!(modern_result["result"]["resultType"], "complete");
    }

    #[test]
    fn unauthenticated_client_cannot_discover_workspace_or_rewrite_tools() {
        let dir = tempdir::TempDir::new("mcp-no-agent").unwrap();
        let mut session = legacy_ready(Session::default(), dir.path());
        let result = session
            .handle(
                dir.path(),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            )
            .unwrap();
        let tools = result["result"]["tools"].as_array().unwrap();
        assert!(!tools.iter().any(|tool| tool["name"] == "workspace_write"));
        assert!(!tools.iter().any(|tool| tool["name"] == "rewrite_text"));
    }

    #[test]
    fn permissions_are_revalidated_for_tool_discovery() {
        let dir = tempdir::TempDir::new("mcp-agent").unwrap();
        std::fs::create_dir_all(dir.path().join("match")).unwrap();
        let permissions = AgentPermissions {
            read_workspace: true,
            write_workspace: true,
            delete_workspace: false,
            rewrite_ai: false,
        };
        let pairing = agents::register(dir.path(), "agent", permissions).unwrap();
        let mut session = legacy_ready(
            Session::with_credentials(
                dir.path(),
                pairing.agent.id.clone(),
                pairing.token.clone(),
            ),
            dir.path(),
        );

        let before = session
            .handle(
                dir.path(),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            )
            .unwrap();
        assert!(before["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "workspace_write"));

        agents::set_enabled(dir.path(), &pairing.agent.id, false).unwrap();
        let after = session
            .handle(
                dir.path(),
                json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}),
            )
            .unwrap();
        assert!(!after["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "workspace_write"));
    }

    #[test]
    fn modern_requests_require_client_capabilities() {
        let dir = tempdir::TempDir::new("mcp-modern-meta").unwrap();
        let mut session = Session::default();
        let result = session
            .handle(
                dir.path(),
                json!({
                    "jsonrpc":"2.0",
                    "id":1,
                    "method":"server/discover",
                    "params":{"_meta":{"io.modelcontextprotocol/protocolVersion":MODERN_PROTOCOL}}
                }),
            )
            .unwrap();
        assert_eq!(result["error"]["code"], -32602);
    }
}
