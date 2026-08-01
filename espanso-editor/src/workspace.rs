use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("match directory does not exist: {0}")]
    MissingMatchDirectory(PathBuf),
    #[error("match file is not part of the workspace: {0}")]
    UnknownFile(PathBuf),
    #[error("match no longer exists: {0:?}")]
    UnknownRule(RuleId),
    #[error("invalid YAML in {path}: {message}")]
    InvalidYaml { path: PathBuf, message: String },
    #[error("the raw YAML block must contain exactly one match")]
    InvalidRawRule,
    #[error("file changed outside Match Studio and was not overwritten: {0}")]
    ExternalModification(PathBuf),
    #[error("unable to save {path}: {message}")]
    SaveFailed { path: PathBuf, message: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, WorkspaceError>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId {
    pub file: PathBuf,
    pub ordinal: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Trigger,
    Regex,
}

impl MatchKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Trigger => "Trigger",
            Self::Regex => "RegExp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDraft {
    pub kind: MatchKind,
    pub triggers: Vec<String>,
    pub regex: String,
    pub replace: String,
    pub label: String,
    pub disabled: bool,
}

impl Default for RuleDraft {
    fn default() -> Self {
        Self {
            kind: MatchKind::Trigger,
            triggers: vec![":new".to_owned()],
            regex: String::new(),
            replace: String::new(),
            label: String::new(),
            disabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuleRecord {
    pub id: RuleId,
    pub draft: RuleDraft,
    pub raw: String,
    pub replacement_preview: String,
}

impl RuleRecord {
    pub fn display_name(&self) -> String {
        let cause = match self.draft.kind {
            MatchKind::Trigger => self
                .draft
                .triggers
                .first()
                .cloned()
                .unwrap_or_else(|| "<empty trigger>".to_owned()),
            MatchKind::Regex => {
                if self.draft.regex.is_empty() {
                    "<empty regexp>".to_owned()
                } else {
                    self.draft.regex.clone()
                }
            }
        };
        let label = self.draft.label.trim();
        if label.is_empty() {
            cause
        } else {
            format!("{label} · {cause}")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub file: Option<PathBuf>,
    pub rule: Option<RuleId>,
}

#[derive(Debug, Clone)]
pub struct PlaygroundResult {
    pub id: RuleId,
    pub display_name: String,
    pub matched_text: String,
    pub captures: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct RegexExampleResult {
    pub input: String,
    pub matched: bool,
    pub matched_text: String,
    pub captures: BTreeMap<String, String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegexBuilderSpec {
    pub prefix: String,
    pub capture_name: String,
    pub capture_pattern: String,
    pub suffix: String,
    pub anchor_start: bool,
    pub anchor_end: bool,
}

impl Default for RegexBuilderSpec {
    fn default() -> Self {
        Self {
            prefix: ":item_".to_owned(),
            capture_name: "value".to_owned(),
            capture_pattern: r"\d+".to_owned(),
            suffix: String::new(),
            anchor_start: false,
            anchor_end: true,
        }
    }
}

#[derive(Debug, Clone)]
struct RuleSpan {
    start: usize,
    end: usize,
    indent: usize,
}

#[derive(Debug, Clone)]
struct ParsedDocument {
    matches_header_end: Option<usize>,
    section_end: usize,
    rule_indent: usize,
    spans: Vec<RuleSpan>,
}

#[derive(Debug, Clone)]
struct MatchDocument {
    path: PathBuf,
    original: String,
    working: String,
    original_hash: u64,
    parsed: ParsedDocument,
    parse_error: Option<String>,
    imports: Vec<String>,
}

impl MatchDocument {
    fn load(path: PathBuf) -> Result<Self> {
        let original = fs::read_to_string(&path)?;
        let original_hash = hash_text(&original);
        Ok(Self::from_content(
            path,
            original.clone(),
            original,
            original_hash,
        ))
    }

    fn from_content(path: PathBuf, original: String, working: String, original_hash: u64) -> Self {
        let parsed = parse_document(&working);
        let parsed_shape = serde_norway::from_str::<MatchFileShape>(&working);
        let (parse_error, imports) = match parsed_shape {
            Ok(shape) => (None, shape.imports),
            Err(error) => (Some(error.to_string()), Vec::new()),
        };
        Self {
            path,
            original,
            working,
            original_hash,
            parsed,
            parse_error,
            imports,
        }
    }

    fn refresh(&mut self) {
        self.parsed = parse_document(&self.working);
        match serde_norway::from_str::<MatchFileShape>(&self.working) {
            Ok(shape) => {
                self.parse_error = None;
                self.imports = shape.imports;
            }
            Err(error) => {
                self.parse_error = Some(error.to_string());
                self.imports.clear();
            }
        }
    }

    fn is_dirty(&self) -> bool {
        self.working != self.original
    }

    fn rule_raw(&self, ordinal: usize) -> Option<&str> {
        let span = self.parsed.spans.get(ordinal)?;
        self.working.get(span.start..span.end)
    }

    fn replace_range(&mut self, start: usize, end: usize, replacement: &str) {
        self.working.replace_range(start..end, replacement);
        self.refresh();
    }
}

#[derive(Debug)]
pub struct MatchWorkspace {
    config_root: PathBuf,
    match_root: PathBuf,
    documents: Vec<MatchDocument>,
}

impl MatchWorkspace {
    pub fn load(config_root: impl Into<PathBuf>) -> Result<Self> {
        let config_root = config_root.into();
        let match_root = config_root.join("match");
        if !match_root.is_dir() {
            return Err(WorkspaceError::MissingMatchDirectory(match_root));
        }

        let mut paths = WalkDir::new(&match_root)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(walkdir::DirEntry::into_path)
            .filter(|path| is_yaml_path(path))
            .collect::<Vec<_>>();
        paths.sort();

        let mut documents = Vec::with_capacity(paths.len());
        for path in paths {
            documents.push(MatchDocument::load(path)?);
        }

        Ok(Self {
            config_root,
            match_root,
            documents,
        })
    }

    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    pub fn match_root(&self) -> &Path {
        &self.match_root
    }

    pub fn reload(&mut self) -> Result<()> {
        *self = Self::load(self.config_root.clone())?;
        Ok(())
    }

    pub fn files(&self) -> Vec<PathBuf> {
        self.documents.iter().map(|doc| doc.path.clone()).collect()
    }

    pub fn relative_path(&self, path: &Path) -> PathBuf {
        path.strip_prefix(&self.config_root)
            .unwrap_or(path)
            .to_path_buf()
    }

    pub fn rules(&self) -> Vec<RuleRecord> {
        let mut records = Vec::new();
        for document in &self.documents {
            for (ordinal, span) in document.parsed.spans.iter().enumerate() {
                let raw = document
                    .working
                    .get(span.start..span.end)
                    .unwrap_or_default()
                    .to_owned();
                let Some(draft) = parse_rule_draft(&raw) else {
                    continue;
                };
                records.push(RuleRecord {
                    id: RuleId {
                        file: document.path.clone(),
                        ordinal,
                    },
                    replacement_preview: preview(&draft.replace, 90),
                    draft,
                    raw,
                });
            }
        }
        records
    }

    pub fn rule(&self, id: &RuleId) -> Result<RuleRecord> {
        let document = self.document(&id.file)?;
        let raw = document
            .rule_raw(id.ordinal)
            .ok_or_else(|| WorkspaceError::UnknownRule(id.clone()))?
            .to_owned();
        let draft = parse_rule_draft(&raw).ok_or(WorkspaceError::InvalidRawRule)?;
        Ok(RuleRecord {
            id: id.clone(),
            replacement_preview: preview(&draft.replace, 90),
            draft,
            raw,
        })
    }

    pub fn raw_file(&self, path: &Path) -> Result<&str> {
        Ok(&self.document(path)?.working)
    }

    pub fn set_raw_file(&mut self, path: &Path, content: String) -> Result<()> {
        let document = self.document_mut(path)?;
        serde_norway::from_str::<MatchFileShape>(&content).map_err(|error| {
            WorkspaceError::InvalidYaml {
                path: path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        document.working = content;
        document.refresh();
        Ok(())
    }

    pub fn create_rule(&mut self, file: &Path, draft: &RuleDraft) -> Result<RuleId> {
        let document = self.document_mut(file)?;
        let indent = document.parsed.rule_indent.max(2);
        let mut block = render_rule(draft, indent, &[]);
        if !block.ends_with('\n') {
            block.push('\n');
        }

        let insertion = if document.parsed.matches_header_end.is_some() {
            document.parsed.section_end
        } else {
            document.working.len()
        };

        let ordinal = document.parsed.spans.len();
        let replacement = if document.parsed.matches_header_end.is_some() {
            prefix_newline_if_needed(&document.working, insertion, &block)
        } else {
            let prefix = if document.working.is_empty() || document.working.ends_with('\n') {
                String::new()
            } else {
                "\n".to_owned()
            };
            format!("{prefix}matches:\n{block}")
        };
        document.replace_range(insertion, insertion, &replacement);
        Ok(RuleId {
            file: file.to_path_buf(),
            ordinal,
        })
    }

    pub fn update_rule(&mut self, id: &RuleId, draft: &RuleDraft) -> Result<()> {
        let document = self.document_mut(&id.file)?;
        let span = document
            .parsed
            .spans
            .get(id.ordinal)
            .cloned()
            .ok_or_else(|| WorkspaceError::UnknownRule(id.clone()))?;
        let raw = document
            .working
            .get(span.start..span.end)
            .unwrap_or_default();
        let preserved = preserved_chunks(raw, span.indent);
        let replacement = render_rule(draft, span.indent, &preserved);
        document.replace_range(span.start, span.end, &replacement);
        Ok(())
    }

    pub fn update_rule_raw(&mut self, id: &RuleId, raw: &str) -> Result<()> {
        let document = self.document_mut(&id.file)?;
        let span = document
            .parsed
            .spans
            .get(id.ordinal)
            .cloned()
            .ok_or_else(|| WorkspaceError::UnknownRule(id.clone()))?;
        validate_single_rule(raw)?;
        let replacement = reindent_rule(raw, span.indent);
        document.replace_range(span.start, span.end, &replacement);
        Ok(())
    }

    pub fn duplicate_rule(&mut self, id: &RuleId) -> Result<RuleId> {
        let document = self.document_mut(&id.file)?;
        let span = document
            .parsed
            .spans
            .get(id.ordinal)
            .cloned()
            .ok_or_else(|| WorkspaceError::UnknownRule(id.clone()))?;
        let raw = document
            .working
            .get(span.start..span.end)
            .unwrap_or_default()
            .to_owned();
        document.replace_range(span.end, span.end, &raw);
        Ok(RuleId {
            file: id.file.clone(),
            ordinal: id.ordinal + 1,
        })
    }

    pub fn delete_rule(&mut self, id: &RuleId) -> Result<()> {
        let document = self.document_mut(&id.file)?;
        let span = document
            .parsed
            .spans
            .get(id.ordinal)
            .cloned()
            .ok_or_else(|| WorkspaceError::UnknownRule(id.clone()))?;
        document.replace_range(span.start, span.end, "");
        Ok(())
    }

    pub fn move_rule(
        &mut self,
        id: &RuleId,
        target_file: &Path,
        target_ordinal: Option<usize>,
    ) -> Result<RuleId> {
        let raw = self.rule(id)?.raw;
        self.delete_rule(id)?;

        let target = self.document_mut(target_file)?;
        let indent = target.parsed.rule_indent.max(2);
        let raw = reindent_rule(&raw, indent);
        let ordinal = target_ordinal
            .unwrap_or(target.parsed.spans.len())
            .min(target.parsed.spans.len());
        let insertion = target
            .parsed
            .spans
            .get(ordinal)
            .map_or(target.parsed.section_end, |span| span.start);
        let replacement = if target.parsed.matches_header_end.is_some() {
            prefix_newline_if_needed(&target.working, insertion, &raw)
        } else {
            let prefix = if target.working.is_empty() || target.working.ends_with('\n') {
                String::new()
            } else {
                "\n".to_owned()
            };
            format!("{prefix}matches:\n{raw}")
        };
        target.replace_range(insertion, insertion, &replacement);
        Ok(RuleId {
            file: target_file.to_path_buf(),
            ordinal,
        })
    }

    pub fn dirty_files(&self) -> Vec<PathBuf> {
        self.documents
            .iter()
            .filter(|doc| doc.is_dirty())
            .map(|doc| doc.path.clone())
            .collect()
    }

    pub fn save_all(&mut self) -> Result<Vec<PathBuf>> {
        let dirty = self
            .documents
            .iter()
            .enumerate()
            .filter(|(_, doc)| doc.is_dirty())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        for index in &dirty {
            let document = &self.documents[*index];
            let current = fs::read_to_string(&document.path)?;
            if hash_text(&current) != document.original_hash {
                return Err(WorkspaceError::ExternalModification(document.path.clone()));
            }
            if let Some(error) = &document.parse_error {
                return Err(WorkspaceError::InvalidYaml {
                    path: document.path.clone(),
                    message: error.clone(),
                });
            }
        }

        let originals = dirty
            .iter()
            .map(|index| {
                let document = &self.documents[*index];
                (*index, document.original.clone(), document.original_hash)
            })
            .collect::<Vec<_>>();
        let mut attempted = Vec::new();
        let mut saved = Vec::new();

        for index in dirty {
            attempted.push(index);
            let save_result = {
                let document = &mut self.documents[index];
                save_document(document)
            };
            if let Err(error) = save_result {
                let mut rollback_errors = Vec::new();
                for rollback_index in attempted.iter().copied().rev() {
                    let document = &mut self.documents[rollback_index];
                    let file_name = document
                        .path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or("matches.yml");
                    let backup = document
                        .path
                        .with_file_name(format!("{file_name}.respanso.bak"));
                    let temporary = document
                        .path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(format!(".{file_name}.respanso.tmp"));
                    if backup.is_file() {
                        if let Err(restore_error) = fs::copy(&backup, &document.path) {
                            rollback_errors
                                .push(format!("{}: {restore_error}", document.path.display()));
                        }
                    }
                    let _ = fs::remove_file(temporary);
                    if let Some((_, original, original_hash)) = originals
                        .iter()
                        .find(|(item, _, _)| *item == rollback_index)
                    {
                        document.original.clone_from(original);
                        document.original_hash = *original_hash;
                    }
                }
                if rollback_errors.is_empty() {
                    return Err(error);
                }
                return Err(WorkspaceError::SaveFailed {
                    path: self.documents[index].path.clone(),
                    message: format!(
                        "{error}; rollback also failed for {}",
                        rollback_errors.join(", ")
                    ),
                });
            }
            saved.push(self.documents[index].path.clone());
        }
        Ok(saved)
    }

    pub fn working_snapshot(&self) -> Vec<(PathBuf, String)> {
        self.documents
            .iter()
            .map(|document| (document.path.clone(), document.working.clone()))
            .collect()
    }

    pub fn restore_working_snapshot(&mut self, snapshot: &[(PathBuf, String)]) {
        for (path, content) in snapshot {
            if let Some(document) = self
                .documents
                .iter_mut()
                .find(|document| document.path == *path)
            {
                document.working.clone_from(content);
                document.refresh();
            }
        }
    }

    pub fn rename_placeholder_in_replacements(
        &mut self,
        old_name: &str,
        new_name: &str,
    ) -> Result<usize> {
        if old_name == new_name {
            return Ok(0);
        }
        let old_placeholder = format!("{{{{{old_name}}}}}");
        let new_placeholder = format!("{{{{{new_name}}}}}");
        let ids = self
            .rules()
            .into_iter()
            .filter(|rule| rule.draft.replace.contains(&old_placeholder))
            .map(|rule| rule.id)
            .collect::<Vec<_>>();
        let mut changed = 0;
        for id in ids {
            let mut rule = self.rule(&id)?;
            rule.draft.replace = rule
                .draft
                .replace
                .replace(&old_placeholder, &new_placeholder);
            self.update_rule(&id, &rule.draft)?;
            changed += 1;
        }
        Ok(changed)
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for document in &self.documents {
            if let Some(error) = &document.parse_error {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: format!("YAML parse error: {error}"),
                    file: Some(document.path.clone()),
                    rule: None,
                });
            }
        }

        for document in &self.documents {
            for (ordinal, span) in document.parsed.spans.iter().enumerate() {
                let raw = document
                    .working
                    .get(span.start..span.end)
                    .unwrap_or_default();
                if parse_rule_draft(raw).is_none() {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Error,
                        message: "Invalid rule block".to_owned(),
                        file: Some(document.path.clone()),
                        rule: Some(RuleId {
                            file: document.path.clone(),
                            ordinal,
                        }),
                    });
                }
            }
        }

        let rules = self.rules();
        for rule in &rules {
            match rule.draft.kind {
                MatchKind::Trigger => {
                    if rule.draft.triggers.is_empty()
                        || rule.draft.triggers.iter().any(String::is_empty)
                    {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            message: "Empty trigger".to_owned(),
                            file: Some(rule.id.file.clone()),
                            rule: Some(rule.id.clone()),
                        });
                    }
                    // Identical triggers intentionally open the selection window.
                }
                MatchKind::Regex => {
                    if rule.draft.regex.is_empty() {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            message: "Empty regexp".to_owned(),
                            file: Some(rule.id.file.clone()),
                            rule: Some(rule.id.clone()),
                        });
                    } else if let Err(error) = Regex::new(&rule.draft.regex) {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            message: format!("Invalid regexp: {error}"),
                            file: Some(rule.id.file.clone()),
                            rule: Some(rule.id.clone()),
                        });
                    }
                    // Identical RegExp causes are also valid selection alternatives.
                }
            }
        }

        diagnostics.extend(self.import_diagnostics());
        diagnostics
    }

    pub fn playground(&self, input: &str) -> Vec<PlaygroundResult> {
        let mut results = Vec::new();
        for rule in self.rules() {
            if rule.draft.disabled {
                continue;
            }
            match rule.draft.kind {
                MatchKind::Trigger => {
                    for trigger in &rule.draft.triggers {
                        if !trigger.is_empty() && input.ends_with(trigger) {
                            results.push(PlaygroundResult {
                                id: rule.id.clone(),
                                display_name: rule.display_name(),
                                matched_text: trigger.clone(),
                                captures: BTreeMap::new(),
                            });
                        }
                    }
                }
                MatchKind::Regex => {
                    let Ok(regex) = Regex::new(&rule.draft.regex) else {
                        continue;
                    };
                    if let Some(captures) = regex.captures(input) {
                        let Some(whole) = captures.get(0) else {
                            continue;
                        };
                        let mut values = BTreeMap::new();
                        for (index, name) in regex.capture_names().enumerate() {
                            if index == 0 {
                                continue;
                            }
                            if let Some(value) = captures.get(index) {
                                values.insert(
                                    name.map_or_else(|| index.to_string(), str::to_owned),
                                    value.as_str().to_owned(),
                                );
                            }
                        }
                        results.push(PlaygroundResult {
                            id: rule.id.clone(),
                            display_name: rule.display_name(),
                            matched_text: whole.as_str().to_owned(),
                            captures: values,
                        });
                    }
                }
            }
        }
        results
    }

    pub fn validate_regex(pattern: &str) -> std::result::Result<(), String> {
        Regex::new(pattern)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn regex_examples(pattern: &str, examples: &[String]) -> Vec<RegexExampleResult> {
        let regex = match Regex::new(pattern) {
            Ok(regex) => regex,
            Err(error) => {
                return examples
                    .iter()
                    .map(|input| RegexExampleResult {
                        input: input.clone(),
                        matched: false,
                        matched_text: String::new(),
                        captures: BTreeMap::new(),
                        error: Some(error.to_string()),
                    })
                    .collect();
            }
        };

        examples
            .iter()
            .map(|input| {
                let Some(captures) = regex.captures(input) else {
                    return RegexExampleResult {
                        input: input.clone(),
                        matched: false,
                        matched_text: String::new(),
                        captures: BTreeMap::new(),
                        error: None,
                    };
                };
                let mut values = BTreeMap::new();
                for (index, name) in regex.capture_names().enumerate() {
                    if index == 0 {
                        continue;
                    }
                    if let Some(value) = captures.get(index) {
                        values.insert(
                            name.map_or_else(|| index.to_string(), str::to_owned),
                            value.as_str().to_owned(),
                        );
                    }
                }
                RegexExampleResult {
                    input: input.clone(),
                    matched: true,
                    matched_text: captures
                        .get(0)
                        .map_or_else(String::new, |value| value.as_str().to_owned()),
                    captures: values,
                    error: None,
                }
            })
            .collect()
    }

    pub fn build_regex(spec: &RegexBuilderSpec) -> std::result::Result<String, String> {
        if !spec.capture_name.is_empty()
            && !Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$")
                .expect("static regexp must compile")
                .is_match(&spec.capture_name)
        {
            return Err("Capture name must be a valid identifier".to_owned());
        }
        Regex::new(&spec.capture_pattern).map_err(|error| error.to_string())?;

        let mut result = String::new();
        if spec.anchor_start {
            result.push('^');
        }
        result.push_str(&regex::escape(&spec.prefix));
        if spec.capture_name.is_empty() {
            let _ = write!(result, "({})", spec.capture_pattern);
        } else {
            let _ = write!(
                result,
                "(?P<{}>{})",
                spec.capture_name, spec.capture_pattern
            );
        }
        result.push_str(&regex::escape(&spec.suffix));
        if spec.anchor_end {
            result.push('$');
        }
        Ok(result)
    }

    fn document(&self, path: &Path) -> Result<&MatchDocument> {
        self.documents
            .iter()
            .find(|doc| doc.path == path)
            .ok_or_else(|| WorkspaceError::UnknownFile(path.to_path_buf()))
    }

    fn document_mut(&mut self, path: &Path) -> Result<&mut MatchDocument> {
        self.documents
            .iter_mut()
            .find(|doc| doc.path == path)
            .ok_or_else(|| WorkspaceError::UnknownFile(path.to_path_buf()))
    }

    fn import_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let known = self
            .documents
            .iter()
            .map(|doc| normalize_path(&doc.path))
            .collect::<HashSet<_>>();
        let mut graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

        for document in &self.documents {
            let source = normalize_path(&document.path);
            let mut targets = Vec::new();
            for import in &document.imports {
                let target = resolve_import(&document.path, import);
                if !known.contains(&target) {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Warning,
                        message: format!("Missing import: {import}"),
                        file: Some(document.path.clone()),
                        rule: None,
                    });
                }
                targets.push(target);
            }
            graph.insert(source, targets);
        }

        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();
        let mut reported = BTreeSet::new();
        for node in graph.keys() {
            detect_cycles(
                node,
                &graph,
                &mut visiting,
                &mut visited,
                &mut Vec::new(),
                &mut reported,
            );
        }
        for cycle in reported {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Error,
                message: format!("Import cycle: {cycle}"),
                file: None,
                rule: None,
            });
        }
        diagnostics
    }
}

#[derive(Debug, Default, Deserialize)]
struct MatchFileShape {
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default)]
    matches: Vec<LooseRule>,
}

#[derive(Debug, Default, Deserialize)]
struct LooseRule {
    trigger: Option<String>,
    #[serde(default)]
    triggers: Vec<String>,
    regex: Option<String>,
    replace: Option<String>,
    label: Option<String>,
    #[serde(default)]
    disabled: bool,
}

fn parse_rule_draft(raw: &str) -> Option<RuleDraft> {
    let wrapper = format!("matches:\n{}", reindent_rule(raw, 2));
    let shape = serde_norway::from_str::<MatchFileShape>(&wrapper).ok()?;
    let rule = shape.matches.into_iter().next()?;
    let kind = if rule.regex.is_some() {
        MatchKind::Regex
    } else {
        MatchKind::Trigger
    };
    let triggers = if let Some(trigger) = rule.trigger {
        vec![trigger]
    } else {
        rule.triggers
    };
    Some(RuleDraft {
        kind,
        triggers,
        regex: rule.regex.unwrap_or_default(),
        replace: rule.replace.unwrap_or_default(),
        label: rule.label.unwrap_or_default(),
        disabled: rule.disabled,
    })
}

fn validate_single_rule(raw: &str) -> Result<()> {
    let wrapper = format!("matches:\n{}", reindent_rule(raw, 2));
    let shape = serde_norway::from_str::<MatchFileShape>(&wrapper)
        .map_err(|_| WorkspaceError::InvalidRawRule)?;
    if shape.matches.len() == 1 {
        Ok(())
    } else {
        Err(WorkspaceError::InvalidRawRule)
    }
}

fn parse_document(content: &str) -> ParsedDocument {
    let lines = line_ranges(content);
    let mut matches_header_end = None;
    let mut matches_indent = 0;
    let mut section_end = content.len();

    for (index, (start, end)) in lines.iter().copied().enumerate() {
        let line = &content[start..end];
        let trimmed = trim_line(line);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = indentation(line);
        if is_key_line(trimmed, "matches") {
            matches_header_end = Some(end);
            matches_indent = indent;
            for (next_start, _) in lines.iter().copied().skip(index + 1) {
                let next_line = &content[next_start..];
                let next_line = next_line.lines().next().unwrap_or_default();
                let next_trimmed = next_line.trim();
                if next_trimmed.is_empty() || next_trimmed.starts_with('#') {
                    continue;
                }
                if indentation(next_line) <= matches_indent {
                    section_end = next_start;
                    break;
                }
            }
            break;
        }
    }

    let Some(header_end) = matches_header_end else {
        return ParsedDocument {
            matches_header_end: None,
            section_end: content.len(),
            rule_indent: 2,
            spans: Vec::new(),
        };
    };

    let mut starts = Vec::new();
    let mut rule_indent = None;
    for (start, end) in lines.iter().copied() {
        if start < header_end || start >= section_end {
            continue;
        }
        let line = &content[start..end];
        let trimmed = trim_line(line);
        if trimmed.starts_with("- ") || trimmed == "-" {
            let indent = indentation(line);
            if indent > matches_indent {
                if rule_indent.is_none() {
                    rule_indent = Some(indent);
                }
                if Some(indent) == rule_indent {
                    starts.push(start);
                }
            }
        }
    }

    let mut spans = Vec::new();
    for (index, start) in starts.iter().copied().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(section_end);
        spans.push(RuleSpan {
            start,
            end,
            indent: rule_indent.unwrap_or(2),
        });
    }

    ParsedDocument {
        matches_header_end: Some(header_end),
        section_end,
        rule_indent: rule_indent.unwrap_or(matches_indent + 2),
        spans,
    }
}

fn line_ranges(content: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for segment in content.split_inclusive('\n') {
        let end = start + segment.len();
        ranges.push((start, end));
        start = end;
    }
    if start < content.len() {
        ranges.push((start, content.len()));
    }
    ranges
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn trim_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n']).trim_start()
}

fn is_key_line(trimmed: &str, key: &str) -> bool {
    trimmed
        .strip_prefix(key)
        .is_some_and(|rest| rest.trim_start().starts_with(':'))
}

fn render_rule(draft: &RuleDraft, indent: usize, preserved: &[String]) -> String {
    let first_indent = " ".repeat(indent);
    let child_indent = " ".repeat(indent + 2);
    let mut fields = Vec::new();

    if !draft.label.trim().is_empty() {
        fields.push(render_scalar("label", draft.label.trim(), &child_indent));
    }
    match draft.kind {
        MatchKind::Trigger => {
            if draft.triggers.len() <= 1 {
                fields.push(render_scalar(
                    "trigger",
                    draft.triggers.first().map_or("", String::as_str),
                    &child_indent,
                ));
            } else {
                let encoded =
                    serde_json::to_string(&draft.triggers).unwrap_or_else(|_| "[]".to_owned());
                fields.push(format!("{child_indent}triggers: {encoded}\n"));
            }
        }
        MatchKind::Regex => fields.push(render_scalar("regex", &draft.regex, &child_indent)),
    }
    fields.push(render_text("replace", &draft.replace, &child_indent));
    if draft.disabled {
        fields.push(format!("{child_indent}disabled: true\n"));
    }
    fields.extend(preserved.iter().cloned());

    if fields.is_empty() {
        return format!("{first_indent}- trigger: \"\"\n");
    }

    let first = fields.remove(0);
    let first = first
        .strip_prefix(&child_indent)
        .unwrap_or(first.as_str())
        .to_owned();
    let mut result = format!("{first_indent}- {first}");
    for field in fields {
        result.push_str(&field);
    }
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn render_scalar(key: &str, value: &str, indent: &str) -> String {
    let encoded = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned());
    format!("{indent}{key}: {encoded}\n")
}

fn render_text(key: &str, value: &str, indent: &str) -> String {
    if value.contains('\n') {
        let body_indent = format!("{indent}  ");
        let mut result = format!("{indent}{key}: |-\n");
        for line in value.split('\n') {
            result.push_str(&body_indent);
            result.push_str(line);
            result.push('\n');
        }
        result
    } else {
        render_scalar(key, value, indent)
    }
}

fn preserved_chunks(raw: &str, rule_indent: usize) -> Vec<String> {
    let known = [
        "label", "trigger", "triggers", "regex", "replace", "disabled",
    ];
    let child_indent = rule_indent + 2;
    let lines = raw.split_inclusive('\n').collect::<Vec<_>>();
    let mut chunk_starts = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some(key) = top_level_rule_key(line, rule_indent, child_indent) {
            chunk_starts.push((index, key));
        }
    }

    let mut preserved = Vec::new();
    for (position, (start, key)) in chunk_starts.iter().enumerate() {
        let end = chunk_starts
            .get(position + 1)
            .map_or(lines.len(), |(index, _)| *index);
        let chunk = &lines[*start..end];
        if known.contains(&key.as_str()) {
            for line in chunk {
                if line.trim_start().starts_with('#') {
                    preserved.push(normalize_continuation_line(line, child_indent));
                }
            }
        } else {
            let mut rendered = String::new();
            for (line_index, line) in chunk.iter().enumerate() {
                if line_index == 0 {
                    rendered.push_str(&normalize_unknown_first_line(
                        line,
                        rule_indent,
                        child_indent,
                    ));
                } else {
                    rendered.push_str(line);
                }
            }
            preserved.push(rendered);
        }
    }
    preserved
}

fn top_level_rule_key(line: &str, rule_indent: usize, child_indent: usize) -> Option<String> {
    let indent = indentation(line);
    let trimmed = trim_line(line);
    let candidate = if indent == rule_indent && trimmed.starts_with("- ") {
        trimmed.trim_start_matches("- ")
    } else if indent == child_indent && !trimmed.starts_with('-') && !trimmed.starts_with('#') {
        trimmed
    } else {
        return None;
    };
    let key = candidate.split_once(':')?.0.trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_owned())
    }
}

fn normalize_unknown_first_line(line: &str, rule_indent: usize, child_indent: usize) -> String {
    let trimmed_end = line.trim_end_matches(['\r', '\n']);
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    if indentation(line) == rule_indent {
        let body = trimmed_end.trim_start().trim_start_matches("- ");
        format!("{}{body}{newline}", " ".repeat(child_indent))
    } else {
        line.to_owned()
    }
}

fn normalize_continuation_line(line: &str, child_indent: usize) -> String {
    format!("{}{}", " ".repeat(child_indent), line.trim_start())
}

fn reindent_rule(raw: &str, target_indent: usize) -> String {
    let raw = raw.trim_matches(|character| character == '\r' || character == '\n');
    let min_indent = raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(indentation)
        .min()
        .unwrap_or(0);
    let prefix = " ".repeat(target_indent);
    let mut result = String::new();
    for line in raw.lines() {
        result.push_str(&prefix);
        result.push_str(line.get(min_indent..).unwrap_or(line));
        result.push('\n');
    }
    result
}

fn prefix_newline_if_needed(content: &str, insertion: usize, block: &str) -> String {
    if insertion > 0 && !content[..insertion].ends_with('\n') {
        format!("\n{block}")
    } else {
        block.to_owned()
    }
}

fn save_document(document: &mut MatchDocument) -> Result<()> {
    let parent = document
        .path
        .parent()
        .ok_or_else(|| WorkspaceError::SaveFailed {
            path: document.path.clone(),
            message: "missing parent directory".to_owned(),
        })?;
    let file_name = document
        .path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("matches.yml");
    let backup = document
        .path
        .with_file_name(format!("{file_name}.respanso.bak"));
    let temporary = parent.join(format!(".{file_name}.respanso.tmp"));

    fs::copy(&document.path, &backup).map_err(|error| WorkspaceError::SaveFailed {
        path: document.path.clone(),
        message: format!("unable to create backup: {error}"),
    })?;

    let mut file = fs::File::create(&temporary).map_err(|error| WorkspaceError::SaveFailed {
        path: document.path.clone(),
        message: format!("unable to create temporary file: {error}"),
    })?;
    file.write_all(document.working.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| WorkspaceError::SaveFailed {
            path: document.path.clone(),
            message: format!("unable to write temporary file: {error}"),
        })?;

    if let Err(first_error) = fs::rename(&temporary, &document.path) {
        fs::remove_file(&document.path).map_err(|error| WorkspaceError::SaveFailed {
            path: document.path.clone(),
            message: format!("rename failed ({first_error}); unable to replace target: {error}"),
        })?;
        fs::rename(&temporary, &document.path).map_err(|error| WorkspaceError::SaveFailed {
            path: document.path.clone(),
            message: format!("unable to move temporary file into place: {error}"),
        })?;
    }

    document.original = document.working.clone();
    document.original_hash = hash_text(&document.original);
    Ok(())
}

fn is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        })
}

fn preview(value: &str, limit: usize) -> String {
    let flattened = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.chars().count() <= limit {
        flattened
    } else {
        let mut result = flattened.chars().take(limit).collect::<String>();
        result.push('…');
        result
    }
}

fn hash_text(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn resolve_import(source: &Path, import: &str) -> PathBuf {
    let import_path = PathBuf::from(import);
    if import_path.is_absolute() {
        normalize_path(&import_path)
    } else {
        normalize_path(
            &source
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(import_path),
        )
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn detect_cycles(
    node: &PathBuf,
    graph: &HashMap<PathBuf, Vec<PathBuf>>,
    visiting: &mut HashSet<PathBuf>,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
    reported: &mut BTreeSet<String>,
) {
    if visited.contains(node) {
        return;
    }
    if !visiting.insert(node.clone()) {
        if let Some(position) = stack.iter().position(|item| item == node) {
            let mut cycle = stack[position..]
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>();
            cycle.push(node.display().to_string());
            reported.insert(cycle.join(" -> "));
        }
        return;
    }

    stack.push(node.clone());
    if let Some(children) = graph.get(node) {
        for child in children {
            if graph.contains_key(child) {
                detect_cycles(child, graph, visiting, visited, stack, reported);
            }
        }
    }
    stack.pop();
    visiting.remove(node);
    visited.insert(node.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    fn fixture() -> (TempDir, MatchWorkspace, PathBuf, PathBuf) {
        let temp = TempDir::new("respanso-editor").expect("temp dir");
        let match_dir = temp.path().join("match");
        fs::create_dir_all(&match_dir).expect("match dir");
        let base = match_dir.join("base.yml");
        let extra = match_dir.join("extra.yaml");
        fs::write(
            &base,
            r#"# base comment
imports:
  - "extra.yaml"
matches:
  - trigger: ":hello"
    replace: "Hello"
    word: true
  - regex: ":id_(?P<id>\\d+)"
    replace: "{{id}}"
"#,
        )
        .expect("write base");
        fs::write(
            &extra,
            r#"matches:
  - trigger: ":bye"
    replace: "Bye"
"#,
        )
        .expect("write extra");
        let workspace = MatchWorkspace::load(temp.path()).expect("workspace");
        (temp, workspace, base, extra)
    }

    #[test]
    fn loads_multiple_yaml_and_indexes_rules() {
        let (_temp, workspace, _base, _extra) = fixture();
        assert_eq!(workspace.files().len(), 2);
        assert_eq!(workspace.rules().len(), 3);
    }

    #[test]
    fn creates_rule_in_selected_file() {
        let (_temp, mut workspace, _base, extra) = fixture();
        let id = workspace
            .create_rule(
                &extra,
                &RuleDraft {
                    triggers: vec![":new".to_owned()],
                    replace: "New".to_owned(),
                    ..RuleDraft::default()
                },
            )
            .expect("create");
        assert_eq!(id.ordinal, 1);
        assert!(workspace.raw_file(&extra).expect("raw").contains(":new"));
    }

    #[test]
    fn updates_rule_and_preserves_unknown_fields() {
        let (_temp, mut workspace, base, _extra) = fixture();
        let id = RuleId {
            file: base.clone(),
            ordinal: 0,
        };
        let mut draft = workspace.rule(&id).expect("rule").draft;
        draft.replace = "Changed".to_owned();
        workspace.update_rule(&id, &draft).expect("update");
        let raw = workspace.raw_file(&base).expect("raw");
        assert!(raw.contains("Changed"));
        assert!(raw.contains("word: true"));
        assert!(raw.contains("# base comment"));
    }

    #[test]
    fn raw_rule_mode_keeps_custom_fields() {
        let (_temp, mut workspace, base, _extra) = fixture();
        let id = RuleId {
            file: base.clone(),
            ordinal: 0,
        };
        workspace
            .update_rule_raw(
                &id,
                "  - trigger: \":raw\"\n    replace: \"Raw\"\n    custom: 42\n",
            )
            .expect("raw update");
        assert!(workspace
            .raw_file(&base)
            .expect("raw")
            .contains("custom: 42"));
    }

    #[test]
    fn duplicates_and_deletes_rule() {
        let (_temp, mut workspace, base, _extra) = fixture();
        let id = RuleId {
            file: base,
            ordinal: 0,
        };
        let duplicate = workspace.duplicate_rule(&id).expect("duplicate");
        assert_eq!(workspace.rules().len(), 4);
        workspace.delete_rule(&duplicate).expect("delete");
        assert_eq!(workspace.rules().len(), 3);
    }

    #[test]
    fn moves_rule_between_files() {
        let (_temp, mut workspace, base, extra) = fixture();
        let id = RuleId {
            file: base,
            ordinal: 0,
        };
        let moved = workspace
            .move_rule(&id, &extra, None)
            .expect("move between files");
        assert_eq!(moved.file, extra);
        assert_eq!(workspace.rules().len(), 3);
    }

    #[test]
    fn save_creates_backup() {
        let (_temp, mut workspace, base, _extra) = fixture();
        let id = RuleId {
            file: base.clone(),
            ordinal: 0,
        };
        let mut draft = workspace.rule(&id).expect("rule").draft;
        draft.replace = "Saved".to_owned();
        workspace.update_rule(&id, &draft).expect("update");
        workspace.save_all().expect("save");
        assert!(base.with_file_name("base.yml.respanso.bak").exists());
        assert!(fs::read_to_string(base).expect("read").contains("Saved"));
    }

    #[test]
    fn refuses_external_overwrite() {
        let (_temp, mut workspace, base, _extra) = fixture();
        let id = RuleId {
            file: base.clone(),
            ordinal: 0,
        };
        let mut draft = workspace.rule(&id).expect("rule").draft;
        draft.replace = "Editor".to_owned();
        workspace.update_rule(&id, &draft).expect("update");
        fs::write(&base, "matches: []\n# external\n").expect("external write");
        assert!(matches!(
            workspace.save_all(),
            Err(WorkspaceError::ExternalModification(path)) if path == base
        ));
    }

    #[test]
    fn validates_and_builds_regex() {
        let spec = RegexBuilderSpec::default();
        let pattern = MatchWorkspace::build_regex(&spec).expect("build regex");
        assert!(Regex::new(&pattern).is_ok());
        assert!(pattern.contains("?P<value>"));
    }

    #[test]
    fn regex_examples_show_named_captures() {
        let examples = MatchWorkspace::regex_examples(
            r":id_(?P<id>\d+)",
            &[":id_42".to_owned(), "none".to_owned()],
        );
        assert!(examples[0].matched);
        assert_eq!(examples[0].captures.get("id"), Some(&"42".to_owned()));
        assert!(!examples[1].matched);
    }

    #[test]
    fn playground_does_not_execute_matches() {
        let (_temp, workspace, _base, _extra) = fixture();
        let results = workspace.playground("prefix :hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, ":hello");
    }

    #[test]
    fn trigger_and_regexp_duplicates_are_valid_selection_alternatives() {
        let (_temp, mut workspace, _base, extra) = fixture();
        workspace
            .create_rule(
                &extra,
                &RuleDraft {
                    triggers: vec![":hello".to_owned()],
                    replace: "Selection option".to_owned(),
                    ..RuleDraft::default()
                },
            )
            .expect("create simple duplicate");
        workspace
            .create_rule(
                &extra,
                &RuleDraft {
                    kind: MatchKind::Regex,
                    regex: r":id_(?P<id>\d+)".to_owned(),
                    replace: "Duplicate regexp".to_owned(),
                    ..RuleDraft::default()
                },
            )
            .expect("create regexp duplicate");

        assert!(!workspace
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Duplicate match cause")));
    }

    #[test]
    fn renames_placeholders_in_all_rule_replacements() {
        let (_temp, mut workspace, base, _extra) = fixture();
        let changed = workspace
            .rename_placeholder_in_replacements("id", "patient_id")
            .expect("rename placeholders");
        assert_eq!(changed, 1);
        assert!(workspace
            .raw_file(&base)
            .expect("raw")
            .contains("{{patient_id}}"));
    }

    #[test]
    fn invalid_rule_block_is_diagnostic_and_not_returned_as_default_rule() {
        let temp = TempDir::new("respanso-invalid-rule").expect("temp dir");
        let match_dir = temp.path().join("match");
        fs::create_dir_all(&match_dir).expect("match dir");
        let base = match_dir.join("base.yml");
        fs::write(&base, "matches:\n  - just-a-scalar\n").expect("write invalid rule");
        let workspace = MatchWorkspace::load(temp.path()).expect("workspace");
        assert!(workspace.rules().is_empty());
        assert!(workspace
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message == "Invalid rule block"));
    }

    #[test]
    fn import_cycles_are_reported() {
        let (temp, _workspace, _base, extra) = fixture();
        fs::write(
            &extra,
            "imports:\n  - \"base.yml\"\nmatches:\n  - trigger: \":bye\"\n    replace: \"Bye\"\n",
        )
        .expect("write cycle");
        let workspace = MatchWorkspace::load(temp.path()).expect("reload");
        assert!(workspace
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Import cycle")));
    }
}
