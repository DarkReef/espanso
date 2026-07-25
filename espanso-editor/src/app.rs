use crate::workspace::{
    DiagnosticLevel, MatchKind, MatchWorkspace, PlaygroundResult, RegexBuilderSpec,
    RegexExampleResult, RuleDraft, RuleId,
};
use eframe::egui;
use std::path::{Path, PathBuf};

pub fn run(config_root: PathBuf) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([1000.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rEspanso Match Studio",
        options,
        Box::new(move |_creation_context| Ok(Box::new(MatchStudioApp::new(config_root)))),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    Structured,
    Raw,
}

pub struct MatchStudioApp {
    config_root: PathBuf,
    workspace: Option<MatchWorkspace>,
    load_error: Option<String>,
    filter: String,
    file_filter: Option<PathBuf>,
    selected: Option<RuleId>,
    draft: RuleDraft,
    raw_rule: String,
    mode: EditorMode,
    status: String,
    move_target: Option<PathBuf>,
    playground_input: String,
    playground_results: Vec<PlaygroundResult>,
    regex_examples_text: String,
    regex_results: Vec<RegexExampleResult>,
    builder: RegexBuilderSpec,
    builder_error: Option<String>,
    show_diagnostics: bool,
}

impl MatchStudioApp {
    fn new(config_root: PathBuf) -> Self {
        let (workspace, load_error) = match MatchWorkspace::load(config_root.clone()) {
            Ok(workspace) => (Some(workspace), None),
            Err(error) => (None, Some(error.to_string())),
        };
        Self {
            config_root,
            workspace,
            load_error,
            filter: String::new(),
            file_filter: None,
            selected: None,
            draft: RuleDraft::default(),
            raw_rule: String::new(),
            mode: EditorMode::Structured,
            status: String::new(),
            move_target: None,
            playground_input: String::new(),
            playground_results: Vec::new(),
            regex_examples_text: ":item_42\n:item_alpha".to_owned(),
            regex_results: Vec::new(),
            builder: RegexBuilderSpec::default(),
            builder_error: None,
            show_diagnostics: true,
        }
    }

    fn reload(&mut self) {
        match MatchWorkspace::load(self.config_root.clone()) {
            Ok(workspace) => {
                self.workspace = Some(workspace);
                self.load_error = None;
                self.selected = None;
                self.raw_rule.clear();
                self.status = "Workspace reloaded".to_owned();
            }
            Err(error) => {
                self.load_error = Some(error.to_string());
                self.status = format!("Reload failed: {error}");
            }
        }
    }

    fn select_rule(&mut self, id: RuleId) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        match workspace.rule(&id) {
            Ok(rule) => {
                self.draft = rule.draft;
                self.raw_rule = rule.raw;
                self.move_target = Some(id.file.clone());
                self.selected = Some(id);
                self.refresh_regex_examples();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn refresh_selected(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.select_rule(id);
    }

    fn save_all(&mut self) {
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        match workspace.save_all() {
            Ok(saved) if saved.is_empty() => self.status = "Nothing to save".to_owned(),
            Ok(saved) => self.status = format!("Saved {} file(s) with backups", saved.len()),
            Err(error) => self.status = format!("Save blocked: {error}"),
        }
    }

    fn create_rule(&mut self) {
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        let target = self
            .file_filter
            .clone()
            .or_else(|| workspace.files().into_iter().next());
        let Some(target) = target else {
            self.status = "No YAML file is available".to_owned();
            return;
        };
        match workspace.create_rule(&target, &RuleDraft::default()) {
            Ok(id) => {
                self.status = "New rule created in memory; save to write it".to_owned();
                self.select_rule(id);
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn duplicate_selected(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        match workspace.duplicate_rule(&id) {
            Ok(new_id) => {
                self.status = "Rule duplicated".to_owned();
                self.select_rule(new_id);
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn delete_selected(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        match workspace.delete_rule(&id) {
            Ok(()) => {
                self.selected = None;
                self.raw_rule.clear();
                self.status = "Rule deleted in memory; save to write it".to_owned();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn apply_structured(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        match workspace.update_rule(&id, &self.draft) {
            Ok(()) => {
                self.status = "Structured changes applied in memory".to_owned();
                self.refresh_selected();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn apply_raw(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        match workspace.update_rule_raw(&id, &self.raw_rule) {
            Ok(()) => {
                self.status = "Raw YAML applied in memory".to_owned();
                self.refresh_selected();
            }
            Err(error) => self.status = format!("Raw YAML rejected: {error}"),
        }
    }

    fn move_selected(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(target) = self.move_target.clone() else {
            return;
        };
        if target == id.file {
            self.status = "Choose another YAML file to move the rule".to_owned();
            return;
        }
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        match workspace.move_rule(&id, &target, None) {
            Ok(new_id) => {
                self.file_filter = Some(target);
                self.status = "Rule moved between YAML files".to_owned();
                self.select_rule(new_id);
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn refresh_playground(&mut self) {
        self.playground_results = self
            .workspace
            .as_ref()
            .map_or_else(Vec::new, |workspace| workspace.playground(&self.playground_input));
    }

    fn refresh_regex_examples(&mut self) {
        let examples = self
            .regex_examples_text
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        self.regex_results = MatchWorkspace::regex_examples(&self.draft.regex, &examples);
    }

    fn apply_regex_builder(&mut self) {
        match MatchWorkspace::build_regex(&self.builder) {
            Ok(pattern) => {
                self.draft.kind = MatchKind::Regex;
                self.draft.regex = pattern;
                self.builder_error = None;
                self.refresh_regex_examples();
            }
            Err(error) => self.builder_error = Some(error),
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("rEspanso Match Studio");
                ui.separator();
                if ui.button("Reload").clicked() {
                    self.reload();
                }
                if ui.button("Save all").clicked() {
                    self.save_all();
                }
                if ui.button("New").clicked() {
                    self.create_rule();
                }
                let has_selection = self.selected.is_some();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Duplicate"))
                    .clicked()
                {
                    self.duplicate_selected();
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("Delete"))
                    .clicked()
                {
                    self.delete_selected();
                }
                ui.separator();
                ui.checkbox(&mut self.show_diagnostics, "Diagnostics");
                if let Some(workspace) = &self.workspace {
                    let dirty = workspace.dirty_files().len();
                    if dirty > 0 {
                        ui.strong(format!("Unsaved files: {dirty}"));
                    }
                }
            });
        });
    }

    fn rules_panel(&mut self, ctx: &egui::Context) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let files = workspace.files();
        let rules = workspace.rules();
        let config_root = workspace.config_root().to_path_buf();

        egui::SidePanel::left("rules")
            .resizable(true)
            .default_width(330.0)
            .show(ctx, |ui| {
                ui.heading("Rules");
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .hint_text("Filter trigger, label, replacement…"),
                );
                egui::ComboBox::from_id_salt("file_filter")
                    .selected_text(
                        self.file_filter
                            .as_ref()
                            .map_or_else(|| "All YAML files".to_owned(), |path| {
                                relative_display(&config_root, path)
                            }),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.file_filter, None, "All YAML files");
                        for file in &files {
                            ui.selectable_value(
                                &mut self.file_filter,
                                Some(file.clone()),
                                relative_display(&config_root, file),
                            );
                        }
                    });
                ui.separator();

                let filter = self.filter.to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for rule in rules {
                        if self
                            .file_filter
                            .as_ref()
                            .is_some_and(|file| file != &rule.id.file)
                        {
                            continue;
                        }
                        let haystack = format!(
                            "{} {} {} {}",
                            rule.display_name(),
                            rule.replacement_preview,
                            rule.draft.regex,
                            rule.draft.triggers.join(" ")
                        )
                        .to_lowercase();
                        if !filter.is_empty() && !haystack.contains(&filter) {
                            continue;
                        }
                        let selected = self.selected.as_ref() == Some(&rule.id);
                        let label = format!(
                            "{}  {}\n{}",
                            if rule.draft.kind == MatchKind::Regex {
                                "Rx"
                            } else {
                                "T"
                            },
                            rule.display_name(),
                            relative_display(&config_root, &rule.id.file)
                        );
                        if ui.selectable_label(selected, label).clicked() {
                            self.select_rule(rule.id);
                        }
                        if !rule.replacement_preview.is_empty() {
                            ui.small(rule.replacement_preview.as_str());
                        }
                        ui.separator();
                    }
                });
            });
    }

    fn central_editor(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(error) = &self.load_error {
                ui.heading("Unable to load Match Studio");
                ui.label(error.as_str());
                ui.label(format!("Configuration root: {}", self.config_root.display()));
                return;
            }
            if self.selected.is_none() {
                ui.vertical_centered(|ui| {
                    ui.heading("Select a rule or create a new one");
                    ui.label("The left panel combines matches from every .yml and .yaml file.");
                    ui.label("The playground only evaluates causes; it never executes scripts or APIs.");
                });
                self.playground_ui(ui);
                return;
            }

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, EditorMode::Structured, "Structured");
                ui.selectable_value(&mut self.mode, EditorMode::Raw, "Raw YAML");
                ui.separator();
                if let Some(id) = &self.selected {
                    ui.monospace(relative_display(&self.config_root, &id.file));
                }
            });
            ui.separator();

            match self.mode {
                EditorMode::Structured => self.structured_editor(ui),
                EditorMode::Raw => self.raw_editor(ui),
            }

            ui.separator();
            self.move_ui(ui);
            ui.separator();
            self.playground_ui(ui);
        });
    }

    fn structured_editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Type");
            ui.selectable_value(&mut self.draft.kind, MatchKind::Trigger, "Trigger");
            ui.selectable_value(&mut self.draft.kind, MatchKind::Regex, "RegExp");
            ui.checkbox(&mut self.draft.disabled, "Disabled");
        });
        ui.horizontal(|ui| {
            ui.label("Label");
            ui.text_edit_singleline(&mut self.draft.label);
        });

        match self.draft.kind {
            MatchKind::Trigger => {
                let mut triggers = self.draft.triggers.join(", ");
                ui.horizontal(|ui| {
                    ui.label("Triggers");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut triggers)
                                .desired_width(f32::INFINITY)
                                .hint_text(":hello, :hi"),
                        )
                        .changed()
                    {
                        self.draft.triggers = triggers
                            .split(',')
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(str::to_owned)
                            .collect();
                    }
                });
            }
            MatchKind::Regex => self.regex_editor(ui),
        }

        ui.label("Replacement");
        ui.add(
            egui::TextEdit::multiline(&mut self.draft.replace)
                .desired_rows(10)
                .desired_width(f32::INFINITY),
        );
        if ui.button("Apply structured changes").clicked() {
            self.apply_structured();
        }
    }

    fn regex_editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("RegExp");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.draft.regex)
                        .desired_width(f32::INFINITY)
                        .hint_text(r":id_(?P<id>\d+)"),
                )
                .changed()
            {
                self.refresh_regex_examples();
            }
        });
        match MatchWorkspace::validate_regex(&self.draft.regex) {
            Ok(()) => ui.label("Valid Rust/espanso regular expression"),
            Err(error) => ui.label(format!("Invalid RegExp: {error}")),
        };

        egui::CollapsingHeader::new("RegExp builder and examples")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Literal prefix");
                    ui.text_edit_singleline(&mut self.builder.prefix);
                    ui.label("Suffix");
                    ui.text_edit_singleline(&mut self.builder.suffix);
                });
                ui.horizontal(|ui| {
                    ui.label("Capture name");
                    ui.text_edit_singleline(&mut self.builder.capture_name);
                    ui.label("Capture pattern");
                    ui.text_edit_singleline(&mut self.builder.capture_pattern);
                    ui.checkbox(&mut self.builder.anchor_start, "^ start");
                    ui.checkbox(&mut self.builder.anchor_end, "$ end");
                });
                if ui.button("Generate RegExp").clicked() {
                    self.apply_regex_builder();
                }
                if let Some(error) = &self.builder_error {
                    ui.label(error.as_str());
                }
                ui.columns(2, |columns| {
                    columns[0].label("One test string per line");
                    if columns[0]
                        .add(
                            egui::TextEdit::multiline(&mut self.regex_examples_text)
                                .desired_rows(5)
                                .desired_width(f32::INFINITY),
                        )
                        .changed()
                    {
                        self.refresh_regex_examples();
                    }
                    columns[1].label("Capture preview");
                    egui::ScrollArea::vertical()
                        .max_height(150.0)
                        .show(&mut columns[1], |ui| {
                            for result in &self.regex_results {
                                let status = if result.matched { "match" } else { "no match" };
                                ui.monospace(format!("{} — {status}", result.input));
                                if !result.matched_text.is_empty() {
                                    ui.small(format!("whole: {}", result.matched_text));
                                }
                                for (name, value) in &result.captures {
                                    ui.small(format!("{name} = {value}"));
                                }
                                if let Some(error) = &result.error {
                                    ui.small(error.as_str());
                                }
                                ui.separator();
                            }
                        });
                });
            });
    }

    fn raw_editor(&mut self, ui: &mut egui::Ui) {
        ui.label("Only the selected match block is replaced. Other YAML content stays untouched.");
        ui.add(
            egui::TextEdit::multiline(&mut self.raw_rule)
                .code_editor()
                .desired_rows(24)
                .desired_width(f32::INFINITY),
        );
        if ui.button("Validate and apply raw YAML").clicked() {
            self.apply_raw();
        }
    }

    fn move_ui(&mut self, ui: &mut egui::Ui) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let files = workspace.files();
        ui.horizontal(|ui| {
            ui.label("Move to");
            egui::ComboBox::from_id_salt("move_target")
                .selected_text(
                    self.move_target
                        .as_ref()
                        .map_or_else(|| "Choose YAML".to_owned(), |path| {
                            relative_display(&self.config_root, path)
                        }),
                )
                .show_ui(ui, |ui| {
                    for file in files {
                        ui.selectable_value(
                            &mut self.move_target,
                            Some(file.clone()),
                            relative_display(&self.config_root, &file),
                        );
                    }
                });
            if ui.button("Move rule").clicked() {
                self.move_selected();
            }
        });
    }

    fn playground_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Safe Match Playground")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Checks which trigger or RegExp would match. No replacement, shell, script or API is executed.");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.playground_input)
                            .desired_width(f32::INFINITY)
                            .hint_text("Paste sample input"),
                    );
                    if ui.button("Check").clicked() {
                        self.refresh_playground();
                    }
                });
                for result in &self.playground_results {
                    ui.group(|ui| {
                        ui.strong(result.display_name.as_str());
                        ui.monospace(format!("matched: {}", result.matched_text));
                        for (name, value) in &result.captures {
                            ui.small(format!("{name} = {value}"));
                        }
                    });
                }
                if !self.playground_input.is_empty() && self.playground_results.is_empty() {
                    ui.label("No active rule matched the sample.");
                }
            });
    }

    fn diagnostics_panel(&mut self, ctx: &egui::Context) {
        if !self.show_diagnostics {
            return;
        }
        let diagnostics = self
            .workspace
            .as_ref()
            .map_or_else(Vec::new, MatchWorkspace::diagnostics);
        egui::SidePanel::right("diagnostics")
            .resizable(true)
            .default_width(330.0)
            .show(ctx, |ui| {
                ui.heading("Diagnostics");
                if diagnostics.is_empty() {
                    ui.label("No YAML, RegExp, conflict or import problems detected.");
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for diagnostic in diagnostics {
                        let prefix = match diagnostic.level {
                            DiagnosticLevel::Error => "ERROR",
                            DiagnosticLevel::Warning => "WARN",
                            DiagnosticLevel::Info => "INFO",
                        };
                        ui.strong(format!("{prefix}: {}", diagnostic.message));
                        if let Some(file) = &diagnostic.file {
                            ui.small(relative_display(&self.config_root, file));
                        }
                        if let Some(rule) = diagnostic.rule {
                            if ui.button("Open rule").clicked() {
                                self.select_rule(rule);
                            }
                        }
                        ui.separator();
                    }
                });
            });
    }
}

impl eframe::App for MatchStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.top_bar(ctx);
        self.rules_panel(ctx);
        self.diagnostics_panel(ctx);
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.status.as_str());
                ui.separator();
                ui.small(format!("Config: {}", self.config_root.display()));
            });
        });
        self.central_editor(ctx);
    }
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
