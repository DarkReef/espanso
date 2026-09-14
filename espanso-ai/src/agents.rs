//! Local registry and authentication for MCP agents.
//! Plaintext agent tokens are shown only once and are never persisted.
use serde::{Deserialize, Serialize};
use std::{fs, io::Write as _, path::Path};
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
    fs::create_dir_all(root)
        .map_err(|error| format!("Не удалось создать каталог конфигурации: {error}"))?;
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
    let mut material = Vec::with_capacity(id.len() + token.len() + 32);
    material.extend_from_slice(b"respanso-mcp-agent-v1\0");
    material.extend_from_slice(id.as_bytes());
    material.push(0);
    material.extend_from_slice(token.as_bytes());
    sha256_hex(&material)
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (a, b) in left.bytes().zip(right.bytes()) {
        difference |= a ^ b;
    }
    difference == 0
}

fn generate_token() -> String {
    // UUID v4 is generated from OS randomness by the uuid crate.
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
    if !constant_time_eq(&record.token_hash, &candidate) {
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

// Small dependency-free SHA-256 implementation used only for local token verifiers.
fn sha256_hex(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
        0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
        0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut data = input.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    let mut h = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];

    for chunk in data.chunks_exact(64) {
        let mut w = [0_u32; 64];
        for (index, word) in w.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for index in 0..64 {
            let big1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(big1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let big0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = big0.wrapping_add(majority);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut output = String::with_capacity(64);
    for word in h {
        use std::fmt::Write as _;
        let _ = write!(output, "{word:08x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

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
