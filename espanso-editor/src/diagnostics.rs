use crate::{
    global_variables,
    workspace::{Diagnostic, DiagnosticLevel, MatchKind, MatchWorkspace, RuleId},
};
use rhai::Engine;
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticState {
    New,
    Active,
    PendingResolved,
}

#[derive(Debug, Clone)]
pub struct DiagnosticInstance {
    pub id: u64,
    pub level: DiagnosticLevel,
    pub message: String,
    pub file: Option<PathBuf>,
    pub rule: Option<RuleId>,
    pub state: DiagnosticState,
    pub first_seen_generation: u64,
    pub last_seen_generation: u64,
    pub occurrence_count: u64,
    anchor: String,
}

#[derive(Debug, Default)]
pub struct DiagnosticManager {
    generation: u64,
    instances: BTreeMap<String, DiagnosticInstance>,
}

impl DiagnosticManager {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn active_count(&self) -> usize {
        self.instances
            .values()
            .filter(|instance| instance.state != DiagnosticState::PendingResolved)
            .count()
    }

    pub fn instances(&self) -> Vec<DiagnosticInstance> {
        let mut instances = self.instances.values().cloned().collect::<Vec<_>>();
        instances.sort_by(|left, right| {
            level_rank(left.level)
                .cmp(&level_rank(right.level))
                .then_with(|| left.file.cmp(&right.file))
                .then_with(|| left.anchor.cmp(&right.anchor))
        });
        instances
    }

    pub fn reconcile(
        &mut self,
        mut diagnostics: Vec<Diagnostic>,
        config_root: &Path,
        workspace: Option<&MatchWorkspace>,
    ) {
        self.generation = self.generation.saturating_add(1);
        diagnostics.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then_with(|| left.rule.cmp(&right.rule))
                .then_with(|| left.message.cmp(&right.message))
        });

        let mut seen = HashSet::new();
        let mut duplicate_counts = BTreeMap::<String, usize>::new();
        for diagnostic in diagnostics {
            let base_anchor = diagnostic_anchor(&diagnostic, config_root, workspace);
            let duplicate = duplicate_counts.entry(base_anchor.clone()).or_default();
            let anchor = if *duplicate == 0 {
                base_anchor
            } else {
                format!("{base_anchor}#{}", *duplicate)
            };
            *duplicate += 1;
            seen.insert(anchor.clone());

            if let Some(instance) = self.instances.get_mut(&anchor) {
                instance.level = diagnostic.level;
                instance.message = diagnostic.message;
                instance.file = diagnostic.file;
                instance.rule = diagnostic.rule;
                instance.last_seen_generation = self.generation;
                instance.occurrence_count = instance.occurrence_count.saturating_add(1);
                instance.state = DiagnosticState::Active;
            } else {
                self.instances.insert(
                    anchor.clone(),
                    DiagnosticInstance {
                        id: stable_hash(&anchor),
                        level: diagnostic.level,
                        message: diagnostic.message,
                        file: diagnostic.file,
                        rule: diagnostic.rule,
                        state: DiagnosticState::New,
                        first_seen_generation: self.generation,
                        last_seen_generation: self.generation,
                        occurrence_count: 1,
                        anchor,
                    },
                );
            }
        }

        let stale = self
            .instances
            .keys()
            .filter(|anchor| !seen.contains(*anchor))
            .cloned()
            .collect::<Vec<_>>();
        for anchor in stale {
            let should_remove = self
                .instances
                .get(&anchor)
                .is_some_and(|instance| instance.state == DiagnosticState::PendingResolved);
            if should_remove {
                self.instances.remove(&anchor);
            } else if let Some(instance) = self.instances.get_mut(&anchor) {
                instance.state = DiagnosticState::PendingResolved;
            }
        }
    }
}

pub fn collect_project_diagnostics(
    config_root: &Path,
    workspace: Option<&MatchWorkspace>,
) -> Vec<Diagnostic> {
    let mut diagnostics = workspace.map_or_else(Vec::new, MatchWorkspace::diagnostics);
    diagnostics.extend(validate_global_variables(workspace));
    diagnostics.extend(validate_config_files(config_root));
    diagnostics.extend(validate_rhai_files(config_root));
    diagnostics
}

fn validate_global_variables(workspace: Option<&MatchWorkspace>) -> Vec<Diagnostic> {
    let Some(workspace) = workspace else {
        return Vec::new();
    };
    let mut by_name = BTreeMap::<String, Vec<PathBuf>>::new();
    let mut diagnostics = Vec::new();
    for file in workspace.files() {
        let Ok(content) = workspace.raw_file(&file) else {
            continue;
        };
        match global_variables::list_global_variables(&file, content) {
            Ok(records) => {
                for record in records {
                    by_name
                        .entry(record.definition.name)
                        .or_default()
                        .push(record.file);
                }
            }
            Err(error) => diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Error,
                message: format!("Global variable parse error: {error}"),
                file: Some(file),
                rule: None,
            }),
        }
    }
    for (name, files) in by_name {
        if files.len() > 1 {
            for file in files {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Warning,
                    message: format!("Duplicate global variable: {name}"),
                    file: Some(file),
                    rule: None,
                });
            }
        }
    }
    diagnostics
}

fn validate_config_files(config_root: &Path) -> Vec<Diagnostic> {
    let root = config_root.join("config");
    let mut diagnostics = Vec::new();
    if !root.is_dir() {
        return diagnostics;
    }

    for path in supported_files(&root, "yaml") {
        match fs::read_to_string(&path) {
            Ok(content) => {
                if let Err(error) = serde_norway::from_str::<serde_json::Value>(&content) {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Error,
                        message: format!("Config YAML parse error: {error}"),
                        file: Some(path),
                        rule: None,
                    });
                }
            }
            Err(error) => diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Error,
                message: format!("Config file read error: {error}"),
                file: Some(path),
                rule: None,
            }),
        }
    }
    diagnostics
}

fn validate_rhai_files(config_root: &Path) -> Vec<Diagnostic> {
    let root = config_root.join("scripts");
    let mut diagnostics = Vec::new();
    if !root.is_dir() {
        return diagnostics;
    }
    let engine = Engine::new();

    for path in supported_files(&root, "rhai") {
        match fs::read_to_string(&path) {
            Ok(content) => {
                if let Err(error) = engine.compile(&content) {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Error,
                        message: format!("Rhai compile error: {error}"),
                        file: Some(path),
                        rule: None,
                    });
                }
            }
            Err(error) => diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Error,
                message: format!("Rhai file read error: {error}"),
                file: Some(path),
                rule: None,
            }),
        }
    }
    diagnostics
}

fn supported_files(root: &Path, kind: &str) -> Vec<PathBuf> {
    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|path| {
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            match kind {
                "yaml" => {
                    extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
                }
                "rhai" => extension.eq_ignore_ascii_case("rhai"),
                _ => false,
            }
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn diagnostic_anchor(
    diagnostic: &Diagnostic,
    config_root: &Path,
    workspace: Option<&MatchWorkspace>,
) -> String {
    let file = diagnostic.file.as_ref().map_or_else(
        || "<project>".to_owned(),
        |path| {
            path.strip_prefix(config_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/")
                .to_lowercase()
        },
    );
    let kind = diagnostic_kind(&diagnostic.message);
    let rule = diagnostic.rule.as_ref().map_or_else(
        || "<file>".to_owned(),
        |rule_id| {
            workspace
                .and_then(|workspace| workspace.rule(rule_id).ok())
                .map_or_else(
                    || format!("ordinal:{}", rule_id.ordinal),
                    |record| match record.draft.kind {
                        MatchKind::Trigger => {
                            let mut triggers = record.draft.triggers;
                            triggers.sort();
                            format!("trigger:{}", triggers.join("|"))
                        }
                        MatchKind::Regex => format!("regex:{}", record.draft.regex),
                    },
                )
        },
    );
    format!("{file}|{rule}|{kind}")
}

fn diagnostic_kind(message: &str) -> String {
    let lower = message.to_lowercase();
    for (needle, code) in [
        ("config yaml parse error", "config-yaml-parse"),
        ("yaml parse error", "yaml-parse"),
        ("config file read error", "config-read"),
        ("rhai compile error", "rhai-compile"),
        ("rhai file read error", "rhai-read"),
        ("empty trigger", "empty-trigger"),
        ("empty regexp", "empty-regexp"),
        ("invalid regexp", "invalid-regexp"),
        ("invalid rule block", "invalid-rule-block"),
        ("duplicate global variable", "duplicate-global-variable"),
        ("global variable parse error", "global-variable-parse"),
        ("missing import", "missing-import"),
        ("not imported", "not-imported"),
    ] {
        if lower.contains(needle) {
            if code == "duplicate-global-variable"
                || code == "missing-import"
                || code == "not-imported"
            {
                return format!("{code}:{}", normalize_message(message));
            }
            return code.to_owned();
        }
    }

    message
        .split(':')
        .next()
        .map_or_else(|| "diagnostic".to_owned(), normalize_message)
}

fn normalize_message(message: &str) -> String {
    message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn level_rank(level: DiagnosticLevel) -> u8 {
    match level {
        DiagnosticLevel::Error => 0,
        DiagnosticLevel::Warning => 1,
        DiagnosticLevel::Info => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(message: &str) -> Diagnostic {
        Diagnostic {
            level: DiagnosticLevel::Error,
            message: message.to_owned(),
            file: Some(PathBuf::from("match/base.yml")),
            rule: None,
        }
    }

    #[test]
    fn keeps_instance_identity_when_error_details_change() {
        let root = Path::new(".");
        let mut manager = DiagnosticManager::default();
        manager.reconcile(
            vec![diagnostic("YAML parse error: first detail")],
            root,
            None,
        );
        let first = manager.instances()[0].id;
        manager.reconcile(
            vec![diagnostic("YAML parse error: another detail")],
            root,
            None,
        );
        let second = manager.instances()[0].id;
        assert_eq!(first, second);
        assert_eq!(manager.instances()[0].occurrence_count, 2);
    }

    #[test]
    fn resolves_only_after_two_absent_generations() {
        let root = Path::new(".");
        let mut manager = DiagnosticManager::default();
        manager.reconcile(vec![diagnostic("Empty trigger")], root, None);
        manager.reconcile(Vec::new(), root, None);
        assert_eq!(
            manager.instances()[0].state,
            DiagnosticState::PendingResolved
        );
        manager.reconcile(Vec::new(), root, None);
        assert!(manager.instances().is_empty());
    }

    #[test]
    fn validates_config_and_rhai_files() {
        let directory = tempdir::TempDir::new("respanso-project-diagnostics").unwrap();
        fs::create_dir_all(directory.path().join("config")).unwrap();
        fs::create_dir_all(directory.path().join("scripts")).unwrap();
        fs::write(directory.path().join("config/default.yml"), "broken: [\n").unwrap();
        fs::write(directory.path().join("scripts/bad.rhai"), "let x = ;").unwrap();

        let diagnostics = collect_project_diagnostics(directory.path(), None);
        assert_eq!(diagnostics.len(), 2);
    }
}
