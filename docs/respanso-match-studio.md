# rEspanso Match Studio

## Status

Design proposal for a local cross-platform GUI that manages rEspanso matches and YAML workspaces.

This work is intentionally separate from draft PR #1, which implements dynamic dialogs and selected-text match execution.

## Goal

Add a desktop GUI for managing rEspanso match definitions without forcing users to edit YAML manually.

The editor should treat multiple `.yml` and `.yaml` files as one logical workspace while preserving the physical file in which every rule is stored.

## Product concept

**rEspanso Match Studio** provides:

- CRUD operations for matches and global variables;
- a unified searchable view over multiple YAML files;
- safe movement and copying of rules between files;
- trigger and regular-expression validation;
- an interactive regexp builder, hints and examples;
- a match checker that explains which rule would fire for a test string;
- a raw YAML escape hatch for advanced fields;
- diagnostics before saving or reloading rEspanso.

## Architectural constraint

`espanso-config` currently loads match groups and imports into a runtime model. That model is optimized for reading and execution rather than source-preserving edits.

The GUI must not deserialize an entire YAML document and blindly serialize it again. Doing so could destroy comments, ordering, formatting, anchors, custom fields, unknown fields and hand-written structure.

Introduce a source-aware editing layer, conceptually:

```text
MatchWorkspace
  ├─ documents: Vec<YamlDocument>
  ├─ imports graph
  ├─ flattened rules index
  ├─ diagnostics
  └─ dirty documents

RuleSource
  ├─ document path
  ├─ stable rule identity for the current editing session
  ├─ YAML node/range
  └─ parsed runtime representation when valid
```

### Write strategy

1. Parse documents into a concrete/source-preserving syntax tree or perform narrowly scoped node edits.
2. Preserve untouched source text byte-for-byte where practical.
3. Write only changed files.
4. Use atomic writes: temporary file, flush, rename.
5. Create an optional timestamped backup before the first write in a session.
6. Run the same config validation used by rEspanso before committing the write.
7. Refuse save on destructive ambiguity and offer raw YAML editing instead.

## MVP scope

### Workspace and multi-YAML management

- Discover match files from the active rEspanso config root.
- Display directly included and recursively imported YAML files.
- Present one unified rule table with:
  - enabled or validity state;
  - trigger or regexp;
  - label or description;
  - action type;
  - source file;
  - diagnostics.
- Filter and search by trigger, regexp, label, replacement text, variable type and source file.
- Create, rename and delete workspace YAML files.
- Add and remove imports with circular-import detection.
- Move or copy selected rules between files.
- Support multi-select bulk operations:
  - enable or disable where supported;
  - move or copy;
  - delete;
  - add or remove common options;
  - export selected rules.

### Match CRUD form

Support at least:

- `trigger` and `triggers`;
- `regex`;
- `replace`;
- `label`;
- `search_terms`;
- word boundaries;
- case propagation and uppercase style;
- text format and injection mode;
- variables and common variable types;
- image actions;
- raw YAML fields for unsupported or advanced options.

The editor should expose two synchronized modes:

- **Structured** for common fields and validation;
- **YAML** for the exact rule fragment.

Changing modes must never silently discard fields unknown to the structured editor.

### Regex assistant

- Live compile validation using Rust `regex` semantics, matching the engine dependency.
- Highlight syntax errors and identify the approximate failing fragment.
- Explain common constructs in plain language.
- Builder presets for:
  - literal prefixes and suffixes;
  - digits and bounded digit counts;
  - words and whitespace;
  - alternatives;
  - optional fragments;
  - named capture groups;
  - start and end anchors;
  - Unicode letters where supported.
- Escape literal text automatically.
- Generate a regexp from a small set of positive examples, clearly marking the output as a draft.
- Maintain positive and negative test examples.
- Display named captures and values for the selected test input.
- Warn about unsupported features instead of suggesting PCRE-only constructs.

### Match checker and playground

For an arbitrary test string:

- list exact-trigger and regexp candidates;
- show the winning rule according to rEspanso precedence and ordering;
- explain why other candidates did not win;
- show source file and rule position;
- preview captures and deterministic variables;
- warn when full rendering requires shell, script or API side effects;
- provide a safe mode that never executes external commands.

The checker should reuse production matching logic where possible rather than implementing a second matcher.

### Validation and save flow

Before saving:

- validate YAML syntax;
- validate the edited rule schema;
- compile regex triggers;
- detect duplicate or conflicting triggers;
- detect import cycles and missing files;
- detect references to missing variables where possible;
- show a per-file change preview;
- write changed documents atomically;
- reload or restart the worker only after validation succeeds;
- retain the backup and offer recovery if reload fails.

## Suggested implementation boundaries

Create a dedicated crate:

```text
espanso-editor/
  src/
    app/
    workspace/
    yaml_edit/
    diagnostics/
    regex_assistant/
    match_playground/
    ipc/
```

Keep UI state and source-editing logic separate. The source and workspace layer must be testable without launching a GUI.

### Reusable library APIs

Prefer non-UI APIs over importing private runtime internals directly:

- source-aware match-group loading;
- validation and diagnostics service;
- match simulation service;
- config-root and active-file discovery;
- worker reload IPC command with a structured result.

### UI technology spike

Prefer a Rust-native implementation that can be packaged with existing Windows, macOS and Linux builds without requiring a network service.

The final toolkit should be selected only after a small spike covering:

- multiline YAML editing;
- large virtualized rule tables;
- native file dialogs;
- accessibility and keyboard navigation;
- Windows packaging size;
- compatibility with the current wxWidgets and native UI build constraints.

Do not couple the document or workspace layer to the selected toolkit.

## UX layout

```text
┌ Files / groups ─────┬ Unified rules ─────────────┬ Rule editor ────────────┐
│ base.yml            │ :hello   Text   base.yml   │ Structured | YAML       │
│ medical.yml         │ rx:...   Regex  med.yml    │ Trigger / action / vars │
│ _shared.yml         │ :date    Text   shared     │ diagnostics             │
├─────────────────────┴────────────────────────────┴─────────────────────────┤
│ Playground: input | matched rule | captures | rendered preview | warnings │
└────────────────────────────────────────────────────────────────────────────┘
```

Required keyboard workflow:

- `Ctrl+N`: new rule;
- `Ctrl+S`: validate and save;
- `Ctrl+F`: global search;
- duplicate rule shortcut;
- move focus between files, rule list and editor;
- undo and redo per editing session.

## Delivery plan

### Phase 0: technical spike

- choose a source-preserving YAML editing approach;
- load a real multi-file config into a unified index;
- modify one rule without changing unrelated text;
- validate through `espanso-config`;
- prove atomic save and rollback.

### Phase 1: read-only workspace

- file tree and import graph;
- unified rule list;
- search and filters;
- diagnostics;
- raw YAML viewer.

### Phase 2: CRUD MVP

- add, edit, duplicate and delete common text and regex rules;
- structured and YAML dual editor;
- safe save, backup and worker reload.

### Phase 3: regex assistant and playground

- builder and hints;
- positive and negative examples;
- captures;
- exact and regex candidate explanation;
- safe preview.

### Phase 4: multi-management

- bulk operations;
- rule moves across YAML documents;
- import editor;
- conflict resolution and change preview.

## Acceptance criteria

- [ ] Open an existing rEspanso config containing multiple imported YAML files.
- [ ] Show one unified list while retaining the source file for every rule.
- [ ] Create, update, duplicate and delete common trigger and regex rules.
- [ ] Moving a rule between files updates only the two affected files.
- [ ] Unrelated comments and formatting remain unchanged after editing another rule.
- [ ] Invalid YAML or invalid regex cannot be applied to the running worker.
- [ ] Match checker agrees with production matching tests for trigger and regex precedence.
- [ ] No external command, script or API is executed during ordinary preview.
- [ ] Saving is atomic and a recoverable backup is available.
- [ ] Windows build and keyboard workflow are covered first while Linux and macOS remain supported by the architecture.
- [ ] Unit tests cover imports, cycles, duplicate triggers, regex errors, source preservation, atomic writes and rollback.

## Non-goals for the first release

- full visual authoring of every third-party or custom variable type;
- cloud synchronization;
- collaborative editing;
- arbitrary script execution in the preview pane;
- replacing YAML as the canonical storage format.

## Branch and PR strategy

Keep implementation separate from draft PR #1 (`dev-in`). Suggested increments:

1. `feature/editor-workspace-core`
2. `feature/editor-readonly-ui`
3. `feature/editor-crud`
4. `feature/editor-regex-playground`
5. `feature/editor-multi-yaml`

Each PR should remain independently testable and avoid mixing engine behavior changes with GUI implementation.
