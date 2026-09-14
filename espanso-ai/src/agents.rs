//! Local registry and authentication for MCP agents.
//! Plaintext agent tokens are shown only once and are never persisted.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Write as _, path::Path};
use subtle::ConstantTimeEq;
use uuid::Uuid;

const REGISTRY_VERSION: u32 = 1;
const REGISTRY_FILE: &str = "mcp-agents.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AgentPermissions {
    pub read_workspace: bool,
    pub write_workspace: bool,
    pub delete_workspace: bool,
    pub rewrite_ai: bool,
}

impl Default for AgentPermissions {
    fn default() -> Self {
        Self {
            read_workspace: true,
            write_workspace: true,
            delete_workspace: false,
            rewrite_ai: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentRecord {
    id: String,
    name: String,
    enabled: bool,
    token_hash: String,
    permissions: AgentPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Registry {
    version: u32,
    agents: Vec<AgentRecord>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            agents: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub permissions: AgentPermissions,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedAgent {
    pub id: String,
    pub name: String,
    pub permissions: AgentPermissions,
}

#[derive(Debug, Clone)]
pub struct PairingSecret {
    pub agent: AgentSummary,
    pub token: String,
}

fn registry_path(root: &Path) -> std::path::PathBuf {
    root.join(REGISTRY_FILE)
}

fn load_registry(root: &Path) -> Result<Registry, String> {
    match fs::read(registry_path(root)) {
        Ok(bytes) => {
            let registry: Registry = serde_json::from_slice(&bytes)
                .map_err(|_| "Некорректный mcp-agents.json".to_owned())?;
            if registry.version != REGISTRY_VERSION {
                return Err(format!(
                    "Неподдерживаемая версия mcp-agents.json: {}",
                    registry.version
                ));
            }
            Ok(registry)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Registry::default()),
        Err(error) => Err(format!("Не удалось прочитать реестр MCP-агентов: {error}")),
    }
}

fn save_registry(root: &Path, registry: &Registry) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| format!("Не удалось создать каталог конфигурации: {error}"))?;
    let bytes = serde_json::to_vec_pretty(registry)
        .map_err(|_| "Не удалось сериализовать реестр MCP-агентов".to_owned())?;
    let mut temp = tempfile::NamedTempFile::new_in(root)
        .map_err(|error| format!("Не удалось создать временный файл реестра: {error}"))?;
    temp.write_all(&bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|error| format!("Не удалось записать реестр MCP-агентов: {error}"))?;
    temp.persist(registry_path(root))
        .map_err(|error| format!("Не удалось сохранить реестр MCP-агентов: {}", error.error))?;
    Ok(())
}

fn token_hash(id: &str, token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"respanso-mcp-agent-v1\0");
    hasher.update(id.as_bytes());
    hasher.update(b"\0");
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn generate_token() -> String {
    // UUID v4 is backed by the OS CSPRNG through the uuid crate.
    format!(
        "rma_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn summary(record: &AgentRecord) -> AgentSummary {
    AgentSummary {
        id: record.id.clone(),
        name: record.name.clone(),
        enabled: record.enabled,
        permissions: record.permissions,
    }
}

pub fn list(root: &Path) -> Result<Vec<AgentSummary>, String> {
    Ok(load_registry(root)?.agents.iter().map(summary).collect())
}

pub fn register(
    root: &Path,
    name: &str,
    permissions: AgentPermissions,
) -> Result<PairingSecret, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("Имя MCP-агента должно содержать от 1 до 80 символов".into());
    }
    let mut registry = load_registry(root)?;
    let id = Uuid::new_v4().to_string();
    let token = generate_token();
    let record = AgentRecord {
        id: id.clone(),
        name: name.to_owned(),
        enabled: true,
        token_hash: token_hash(&id, &token),
        permissions,
    };
    let agent = summary(&record);
    registry.agents.push(record);
    save_registry(root, &registry)?;
    Ok(PairingSecret { agent, token })
}

pub fn set_enabled(root: &Path, id: &str, enabled: bool) -> Result<(), String> {
    let mut registry = load_registry(root)?;
    let record = registry
        .agents
        .iter_mut()
        .find(|agent| agent.id == id)
        .ok_or_else(|| "MCP-агент не найден".to_owned())?;
    record.enabled = enabled;
    save_registry(root, &registry)
}

pub fn set_permissions(
    root: &Path,
    id: &str,
    permissions: AgentPermissions,
) -> Result<(), String> {
    let mut registry = load_registry(root)?;
    let record = registry
        .agents
        .iter_mut()
        .find(|agent| agent.id == id)
        .ok_or_else(|| "MCP-агент не найден".to_owned())?;
    record.permissions = permissions;
    save_registry(root, &registry)
}

pub fn rotate_token(root: &Path, id: &str) -> Result<String, String> {
    let mut registry = load_registry(root)?;
    let record = registry
        .agents
        .iter_mut()
        .find(|agent| agent.id == id)
        .ok_or_else(|| "MCP-агент не найден".to_owned())?;
    let token = generate_token();
    record.token_hash = token_hash(id, &token);
    record.enabled = true;
    save_registry(root, &registry)?;
    Ok(token)
}

pub fn remove(root: &Path, id: &str) -> Result<(), String> {
    let mut registry = load_registry(root)?;
    let before = registry.agents.len();
    registry.agents.retain(|agent| agent.id != id);
    if registry.agents.len() == before {
        return Err("MCP-агент не найден".into());
    }
    save_registry(root, &registry)
}

pub fn authenticate(root: &Path, id: &str, token: &str) -> Result<AuthenticatedAgent, String> {
    if id.is_empty() || token.is_empty() {
        return Err("MCP-агент не аутентифицирован".into());
    }
    let registry = load_registry(root)?;
    let record = registry
        .agents
        .iter()
        .find(|agent| agent.id == id)
        .ok_or_else(|| "Неизвестный MCP-агент".to_owned())?;
    if !record.enabled {
        return Err("MCP-агент отключён".into());
    }
    let candidate = token_hash(id, token);
    let matches = record.token_hash.len() == candidate.len()
        && record
            .token_hash
            .as_bytes()
            .ct_eq(candidate.as_bytes())
            .into();
    if !matches {
        return Err("Неверный токен MCP-агента".into());
    }
    Ok(AuthenticatedAgent {
        id: record.id.clone(),
        name: record.name.clone(),
        permissions: record.permissions,
    })
}

pub fn authenticate_from_env(root: &Path) -> Result<AuthenticatedAgent, String> {
    let id = std::env::var("RESPANSO_MCP_AGENT_ID")
        .map_err(|_| "Не задан RESPANSO_MCP_AGENT_ID".to_owned())?;
    let token = std::env::var("RESPANSO_MCP_TOKEN")
        .map_err(|_| "Не задан RESPANSO_MCP_TOKEN".to_owned())?;
    authenticate(root, id.trim(), token.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_not_persisted_and_authentication_is_scoped() {
        let dir = tempdir::TempDir::new("mcp-agents").unwrap();
        let pairing = register(dir.path(), "test agent", AgentPermissions::default()).unwrap();
        let raw = fs::read_to_string(registry_path(dir.path())).unwrap();
        assert!(!raw.contains(&pairing.token));
        assert!(raw.contains("token_hash"));
        let auth = authenticate(dir.path(), &pairing.agent.id, &pairing.token).unwrap();
        assert_eq!(auth.id, pairing.agent.id);
        assert!(authenticate(dir.path(), &pairing.agent.id, "wrong").is_err());
    }

    #[test]
    fn disabled_agent_is_rejected_and_rotation_invalidates_old_token() {
        let dir = tempdir::TempDir::new("mcp-agents-disable").unwrap();
        let pairing = register(dir.path(), "test", AgentPermissions::default()).unwrap();
        set_enabled(dir.path(), &pairing.agent.id, false).unwrap();
        assert!(authenticate(dir.path(), &pairing.agent.id, &pairing.token).is_err());
        let new_token = rotate_token(dir.path(), &pairing.agent.id).unwrap();
        assert!(authenticate(dir.path(), &pairing.agent.id, &pairing.token).is_err());
        assert!(authenticate(dir.path(), &pairing.agent.id, &new_token).is_ok());
    }
}
