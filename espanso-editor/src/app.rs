use crate::workspace::{
    DiagnosticLevel, MatchKind, MatchWorkspace, PlaygroundResult, RegexBuilderSpec,
    RegexExampleResult, RuleDraft, RuleId,
};
use eframe::egui;
use std::path::{Path, PathBuf};

const APP_TITLE: &str = "rEspanso Match Studio";

pub fn run(config_root: PathBuf) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([1050.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |creation_context| {
            let mut style = (*creation_context.egui_ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 6.0);
            creation_context.egui_ctx.set_style(style);
            Ok(Box::new(MatchStudioApp::new(config_root)))
        }),
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
    focus_filter: bool,
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
    show_shortcuts: bool,
    confirm_delete: bool,
    confirm_reload: bool,
}

impl MatchStudioApp {
    fn new(config_root: PathBuf) -> Self {
        let (workspace, load_error) = match MatchWorkspace::load(config_root.clone()) {
            Ok(workspace) => (Some(workspace), None),
            Err(error) => (None, Some(ru_message(&error.to_string()))),
        };
        Self {
            config_root,
            workspace,
            load_error,
            filter: String::new(),
            focus_filter: false,
            file_filter: None,
            selected: None,
            draft: RuleDraft::default(),
            raw_rule: String::new(),
            mode: EditorMode::Structured,
            status: "Готово к работе".to_owned(),
            move_target: None,
            playground_input: String::new(),
            playground_results: Vec::new(),
            regex_examples_text: ":код_42\n:код_иванов".to_owned(),
            regex_results: Vec::new(),
            builder: RegexBuilderSpec::default(),
            builder_error: None,
            show_diagnostics: true,
            show_shortcuts: false,
            confirm_delete: false,
            confirm_reload: false,
        }
    }

    fn reload(&mut self) {
        match MatchWorkspace::load(self.config_root.clone()) {
            Ok(workspace) => {
                self.workspace = Some(workspace);
                self.load_error = None;
                self.selected = None;
                self.raw_rule.clear();
                "Правила перечитаны с диска".clone_into(&mut self.status);
            }
            Err(error) => {
                let message = ru_message(&error.to_string());
                self.load_error = Some(message.clone());
                self.status = format!("Не удалось обновить правила: {message}");
            }
        }
    }

    fn request_reload(&mut self) {
        let has_unsaved = self
            .workspace
            .as_ref()
            .is_some_and(|workspace| !workspace.dirty_files().is_empty());
        if has_unsaved {
            self.confirm_reload = true;
        } else {
            self.reload();
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
            Err(error) => self.status = ru_message(&error.to_string()),
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
            Ok(saved) if saved.is_empty() => {
                "Нет изменений для сохранения".clone_into(&mut self.status)
            }
            Ok(saved) => {
                self.status = format!("Сохранено файлов: {}. Резервные копии созданы", saved.len())
            }
            Err(error) => {
                self.status = format!("Сохранение остановлено: {}", ru_message(&error.to_string()))
            }
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
            "В папке match нет доступных YAML-файлов".clone_into(&mut self.status);
            return;
        };
        match workspace.create_rule(&target, &RuleDraft::default()) {
            Ok(id) => {
                self.status = format!(
                    "Новое правило добавлено в {}. Нажмите Ctrl+S для записи на диск",
                    relative_display(&self.config_root, &target)
                );
                self.select_rule(id);
            }
            Err(error) => self.status = ru_message(&error.to_string()),
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
                "Правило продублировано. Сохраните изменения".clone_into(&mut self.status);
                self.select_rule(new_id);
            }
            Err(error) => self.status = ru_message(&error.to_string()),
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
                "Правило удалено из рабочей копии. Нажмите Ctrl+S для записи на диск"
                    .clone_into(&mut self.status);
            }
            Err(error) => self.status = ru_message(&error.to_string()),
        }
    }

    fn apply_current(&mut self) {
        match self.mode {
            EditorMode::Structured => self.apply_structured(),
            EditorMode::Raw => self.apply_raw(),
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
                "Изменения применены к рабочей копии. Для записи нажмите Ctrl+S"
                    .clone_into(&mut self.status);
                self.refresh_selected();
            }
            Err(error) => self.status = ru_message(&error.to_string()),
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
                "YAML проверен и применён к рабочей копии".clone_into(&mut self.status);
                self.refresh_selected();
            }
            Err(error) => {
                self.status = format!("YAML отклонён: {}", ru_message(&error.to_string()))
            }
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
            "Выберите другой YAML-файл".clone_into(&mut self.status);
            return;
        }
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        match workspace.move_rule(&id, &target, None) {
            Ok(new_id) => {
                self.file_filter = Some(target);
                "Правило перенесено. Сохраните изменения".clone_into(&mut self.status);
                self.select_rule(new_id);
            }
            Err(error) => self.status = ru_message(&error.to_string()),
        }
    }

    fn refresh_playground(&mut self) {
        self.playground_results = self.workspace.as_ref().map_or_else(Vec::new, |workspace| {
            workspace.playground(&self.playground_input)
        });
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
                "Регулярное выражение собрано. Проверьте примеры ниже".clone_into(&mut self.status);
            }
            Err(error) => self.builder_error = Some(ru_message(&error)),
        }
    }

    fn use_regex_preset(&mut self, name: &str, pattern: &str) {
        name.clone_into(&mut self.builder.capture_name);
        pattern.clone_into(&mut self.builder.capture_pattern);
        self.builder_error = None;
    }

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        let (save, new_rule, duplicate, delete, reload, search, apply, diagnostics, help) = context
            .input(|input| {
                let primary = input.modifiers.ctrl || input.modifiers.command;
                (
                    primary && input.key_pressed(egui::Key::S),
                    primary && input.key_pressed(egui::Key::N),
                    primary && input.key_pressed(egui::Key::D),
                    primary && input.modifiers.shift && input.key_pressed(egui::Key::D),
                    primary && input.key_pressed(egui::Key::R),
                    primary && input.key_pressed(egui::Key::F),
                    primary && input.key_pressed(egui::Key::Enter),
                    primary && input.key_pressed(egui::Key::L),
                    input.key_pressed(egui::Key::F1),
                )
            });

        if save {
            self.save_all();
        }
        if new_rule {
            self.create_rule();
        }
        if duplicate && !delete {
            self.duplicate_selected();
        }
        if delete && self.selected.is_some() {
            self.confirm_delete = true;
        }
        if reload {
            self.request_reload();
        }
        if search {
            self.focus_filter = true;
        }
        if apply && self.selected.is_some() {
            self.apply_current();
        }
        if diagnostics {
            self.show_diagnostics = !self.show_diagnostics;
        }
        if help {
            self.show_shortcuts = true;
        }
    }

    fn top_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(APP_TITLE);
                ui.separator();
                if ui
                    .button("Новое правило")
                    .on_hover_text("Ctrl+N. Добавляет правило в выбранный YAML-файл")
                    .clicked()
                {
                    self.create_rule();
                }
                if ui
                    .button("Сохранить всё")
                    .on_hover_text("Ctrl+S. Записывает изменения и создаёт резервные копии")
                    .clicked()
                {
                    self.save_all();
                }
                if ui
                    .button("Обновить")
                    .on_hover_text("Ctrl+R. Перечитывает YAML-файлы с диска")
                    .clicked()
                {
                    self.request_reload();
                }
                let has_selection = self.selected.is_some();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Дублировать"))
                    .on_hover_text("Ctrl+D")
                    .clicked()
                {
                    self.duplicate_selected();
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("Удалить"))
                    .on_hover_text("Ctrl+Shift+D")
                    .clicked()
                {
                    self.confirm_delete = true;
                }
                ui.separator();
                ui.checkbox(&mut self.show_diagnostics, "Проверка")
                    .on_hover_text("Ctrl+L. Ошибки YAML, RegExp, дубликаты и проблемы импортов");
                if ui.button("Горячие клавиши").on_hover_text("F1").clicked() {
                    self.show_shortcuts = true;
                }
                if let Some(workspace) = &self.workspace {
                    let dirty = workspace.dirty_files().len();
                    if dirty > 0 {
                        ui.separator();
                        ui.colored_label(
                            egui::Color32::from_rgb(210, 135, 25),
                            format!("Не сохранено файлов: {dirty}"),
                        );
                    }
                }
            });
        });
    }

    fn rules_panel(&mut self, root: &mut egui::Ui) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let files = workspace.files();
        let rules = workspace.rules();
        let config_root = workspace.config_root().to_path_buf();
        let files_count = files.len();
        let rules_count = rules.len();

        egui::Panel::left("rules")
            .resizable(true)
            .default_size(350.0)
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Правила");
                    ui.label(
                        egui::RichText::new(format!("{rules_count} правил · {files_count} файлов"))
                            .weak(),
                    );
                });
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .desired_width(f32::INFINITY)
                        .hint_text("Поиск по триггеру, названию или тексту…"),
                );
                if self.focus_filter {
                    search.request_focus();
                    self.focus_filter = false;
                }
                egui::ComboBox::from_id_salt("file_filter")
                    .width(ui.available_width())
                    .selected_text(self.file_filter.as_ref().map_or_else(
                        || "Все YAML-файлы".to_owned(),
                        |path| relative_display(&config_root, path),
                    ))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.file_filter, None, "Все YAML-файлы");
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
                let mut visible = 0_usize;
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
                        visible += 1;
                        let selected = self.selected.as_ref() == Some(&rule.id);
                        let kind = if rule.draft.kind == MatchKind::Regex {
                            "RegExp"
                        } else {
                            "Триггер"
                        };
                        let label = format!(
                            "{kind}  ·  {}\n{}",
                            rule.display_name(),
                            relative_display(&config_root, &rule.id.file)
                        );
                        if ui.selectable_label(selected, label).clicked() {
                            self.select_rule(rule.id);
                        }
                        if !rule.replacement_preview.is_empty() {
                            ui.label(egui::RichText::new(rule.replacement_preview.as_str()).weak());
                        }
                        ui.separator();
                    }
                    if visible == 0 {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label("По заданному фильтру ничего не найдено");
                        });
                    }
                });
            });
    }

    fn central_editor(&mut self, root: &mut egui::Ui) {
        egui::CentralPanel::default().show(root, |ui| {
            if let Some(error) = &self.load_error {
                ui.heading("Не удалось открыть рабочую папку");
                ui.colored_label(ui.visuals().error_fg_color, error.as_str());
                ui.label(format!(
                    "Корень конфигурации: {}",
                    self.config_root.display()
                ));
                ui.separator();
                ui.label("Ожидаемая папка правил: portable\\config\\match");
                return;
            }
            if self.selected.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(35.0);
                    ui.heading("Выберите правило слева или создайте новое");
                    ui.label(
                        "Редактор объединяет правила из всех .yml и .yaml файлов папки match.",
                    );
                    ui.label("Ctrl+N — новое правило, Ctrl+F — поиск, F1 — справка.");
                });
                ui.add_space(20.0);
                self.playground_ui(ui);
                return;
            }

            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.mode, EditorMode::Structured, "Удобный редактор");
                ui.selectable_value(&mut self.mode, EditorMode::Raw, "Исходный YAML");
                ui.separator();
                if let Some(id) = &self.selected {
                    ui.label(
                        egui::RichText::new(relative_display(&self.config_root, &id.file)).weak(),
                    );
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
        ui.horizontal_wrapped(|ui| {
            ui.label("Условие срабатывания:");
            ui.selectable_value(&mut self.draft.kind, MatchKind::Trigger, "Обычный триггер");
            ui.selectable_value(&mut self.draft.kind, MatchKind::Regex, "Гибкий RegExp");
            ui.separator();
            ui.checkbox(&mut self.draft.disabled, "Правило временно отключено");
        });
        ui.horizontal(|ui| {
            ui.label("Название правила");
            ui.add(
                egui::TextEdit::singleline(&mut self.draft.label)
                    .desired_width(f32::INFINITY)
                    .hint_text("Необязательно, например: Номер пациента"),
            );
        });

        match self.draft.kind {
            MatchKind::Trigger => {
                let mut triggers = self.draft.triggers.join(", ");
                ui.label("Триггеры");
                ui.label(
                    egui::RichText::new(
                        "Можно указать несколько вариантов через запятую, например :привет, :здр",
                    )
                    .weak(),
                );
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut triggers)
                            .desired_width(f32::INFINITY)
                            .hint_text(":привет, :здр"),
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
            }
            MatchKind::Regex => self.regex_editor(ui),
        }

        ui.add_space(8.0);
        ui.label("Текст подстановки");
        ui.add(
            egui::TextEdit::multiline(&mut self.draft.replace)
                .desired_rows(10)
                .desired_width(f32::INFINITY)
                .hint_text("Текст, который rEspanso вставит вместо триггера"),
        );
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    [230.0, 34.0],
                    egui::Button::new(egui::RichText::new("Применить изменения").strong()),
                )
                .on_hover_text("Ctrl+Enter. После применения нажмите Ctrl+S для записи на диск")
                .clicked()
            {
                self.apply_structured();
            }
            ui.label(
                egui::RichText::new("Применение изменяет рабочую копию; Ctrl+S записывает YAML")
                    .weak(),
            );
        });
    }

    fn regex_editor(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("Понятный конструктор RegExp");
            ui.label(
                "Соберите правило из обычного текста и одной изменяемой части. Знание синтаксиса RegExp не требуется.",
            );
            ui.add_space(4.0);
            ui.label("1. Что находится до изменяемой части");
            ui.add(
                egui::TextEdit::singleline(&mut self.builder.prefix)
                    .desired_width(f32::INFINITY)
                    .hint_text("Например: :пациент_"),
            );

            ui.label("2. Что должна содержать изменяемая часть");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Число")
                    .on_hover_text("Одна или несколько цифр: 0–9")
                    .clicked()
                {
                    self.use_regex_preset("number", r"\d+");
                }
                if ui
                    .button("Слово")
                    .on_hover_text("Русские или латинские буквы")
                    .clicked()
                {
                    self.use_regex_preset("word", r"[A-Za-zА-Яа-яЁё]+");
                }
                if ui.button("Дата ДД.ММ.ГГГГ").clicked() {
                    self.use_regex_preset("date", r"\d{2}\.\d{2}\.\d{4}");
                }
                if ui
                    .button("Без пробелов")
                    .on_hover_text("Любой непробельный фрагмент")
                    .clicked()
                {
                    self.use_regex_preset("value", r"\S+");
                }
                if ui
                    .button("Любой текст")
                    .on_hover_text("Один или несколько любых символов")
                    .clicked()
                {
                    self.use_regex_preset("text", r".+?");
                }
            });
            ui.horizontal(|ui| {
                ui.label("Имя переменной");
                ui.add(
                    egui::TextEdit::singleline(&mut self.builder.capture_name)
                        .hint_text("Латиницей, например value"),
                );
                ui.label("Свой шаблон");
                ui.add(
                    egui::TextEdit::singleline(&mut self.builder.capture_pattern)
                        .desired_width(f32::INFINITY)
                        .hint_text(r"Например: \d+"),
                );
            });

            ui.label("3. Что находится после изменяемой части");
            ui.add(
                egui::TextEdit::singleline(&mut self.builder.suffix)
                    .desired_width(f32::INFINITY)
                    .hint_text("Можно оставить пустым"),
            );
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.builder.anchor_start,
                    "Совпадение должно начинаться строго здесь",
                );
                ui.checkbox(
                    &mut self.builder.anchor_end,
                    "После шаблона не должно быть других символов",
                );
            });

            match MatchWorkspace::build_regex(&self.builder) {
                Ok(preview) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Получится:");
                        ui.monospace(preview);
                    });
                }
                Err(error) => {
                    ui.colored_label(ui.visuals().error_fg_color, ru_message(&error));
                }
            }
            if ui.button("Собрать и использовать выражение").clicked() {
                self.apply_regex_builder();
            }
            if let Some(error) = &self.builder_error {
                ui.colored_label(ui.visuals().error_fg_color, error.as_str());
            }
        });

        ui.add_space(8.0);
        egui::CollapsingHeader::new("Расширенный режим RegExp")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Готовое регулярное выражение");
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
                match MatchWorkspace::validate_regex(&self.draft.regex) {
                    Ok(()) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(40, 150, 90),
                            "Выражение корректно",
                        );
                    }
                    Err(error) => {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            format!("Ошибка RegExp: {error}"),
                        );
                    }
                };
            });

        egui::CollapsingHeader::new("Проверка на примерах")
            .default_open(true)
            .show(ui, |ui| {
                ui.columns(2, |columns| {
                    columns[0].label("По одному примеру в строке");
                    if columns[0]
                        .add(
                            egui::TextEdit::multiline(&mut self.regex_examples_text)
                                .desired_rows(6)
                                .desired_width(f32::INFINITY),
                        )
                        .changed()
                    {
                        self.refresh_regex_examples();
                    }
                    columns[1].label("Результат");
                    egui::ScrollArea::vertical()
                        .max_height(175.0)
                        .show(&mut columns[1], |ui| {
                            for result in &self.regex_results {
                                let status = if result.matched {
                                    "совпало"
                                } else {
                                    "не совпало"
                                };
                                ui.monospace(format!("{} — {status}", result.input));
                                if !result.matched_text.is_empty() {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Найдено: {}",
                                            result.matched_text
                                        ))
                                        .weak(),
                                    );
                                }
                                for (name, value) in &result.captures {
                                    ui.small(format!("Переменная {name}: {value}"));
                                }
                                if let Some(error) = &result.error {
                                    ui.colored_label(ui.visuals().error_fg_color, error.as_str());
                                }
                                ui.separator();
                            }
                        });
                });
            });
    }

    fn raw_editor(&mut self, ui: &mut egui::Ui) {
        ui.label(
            "Изменяется только выбранный блок правила. Остальной YAML, комментарии и неизвестные поля сохраняются.",
        );
        ui.add(
            egui::TextEdit::multiline(&mut self.raw_rule)
                .code_editor()
                .desired_rows(24)
                .desired_width(f32::INFINITY),
        );
        if ui
            .button("Проверить и применить YAML")
            .on_hover_text("Ctrl+Enter")
            .clicked()
        {
            self.apply_raw();
        }
    }

    fn move_ui(&mut self, ui: &mut egui::Ui) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let files = workspace.files();
        ui.horizontal_wrapped(|ui| {
            ui.label("Перенести правило в файл:");
            egui::ComboBox::from_id_salt("move_target")
                .selected_text(self.move_target.as_ref().map_or_else(
                    || "Выберите YAML".to_owned(),
                    |path| relative_display(&self.config_root, path),
                ))
                .show_ui(ui, |ui| {
                    for file in files {
                        ui.selectable_value(
                            &mut self.move_target,
                            Some(file.clone()),
                            relative_display(&self.config_root, &file),
                        );
                    }
                });
            if ui.button("Перенести").clicked() {
                self.move_selected();
            }
        });
    }

    fn playground_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Безопасная проверка срабатывания")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    "Показывает, какое правило сработает. Подстановки, команды, скрипты и API не запускаются.",
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.playground_input)
                            .desired_width(f32::INFINITY)
                            .hint_text("Введите пример текста"),
                    );
                    if ui.button("Проверить").clicked() {
                        self.refresh_playground();
                    }
                });
                for result in &self.playground_results {
                    ui.group(|ui| {
                        ui.strong(result.display_name.as_str());
                        ui.monospace(format!("Совпавший фрагмент: {}", result.matched_text));
                        for (name, value) in &result.captures {
                            ui.small(format!("Переменная {name}: {value}"));
                        }
                    });
                }
                if !self.playground_input.is_empty() && self.playground_results.is_empty() {
                    ui.label("Ни одно включённое правило не совпало с примером");
                }
            });
    }

    fn diagnostics_panel(&mut self, root: &mut egui::Ui) {
        if !self.show_diagnostics {
            return;
        }
        let diagnostics = self
            .workspace
            .as_ref()
            .map_or_else(Vec::new, MatchWorkspace::diagnostics);
        egui::Panel::right("diagnostics")
            .resizable(true)
            .default_size(340.0)
            .show(root, |ui| {
                ui.heading("Проверка конфигурации");
                if diagnostics.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(40, 150, 90),
                        "Проблем не обнаружено",
                    );
                    ui.label(
                        egui::RichText::new(
                            "YAML, RegExp, дубликаты и импорты прошли базовую проверку",
                        )
                        .weak(),
                    );
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for diagnostic in diagnostics {
                        let prefix = match diagnostic.level {
                            DiagnosticLevel::Error => "ОШИБКА",
                            DiagnosticLevel::Warning => "ВНИМАНИЕ",
                            DiagnosticLevel::Info => "СВЕДЕНИЕ",
                        };
                        ui.strong(format!("{prefix}: {}", ru_message(&diagnostic.message)));
                        if let Some(file) = &diagnostic.file {
                            ui.small(relative_display(&self.config_root, file));
                        }
                        if let Some(rule) = diagnostic.rule {
                            if ui.button("Открыть правило").clicked() {
                                self.select_rule(rule);
                            }
                        }
                        ui.separator();
                    }
                });
            });
    }

    fn dialogs(&mut self, context: &egui::Context) {
        if self.show_shortcuts {
            let mut open = self.show_shortcuts;
            egui::Window::new("Горячие клавиши")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(context, |ui| {
                    egui::Grid::new("shortcut_grid")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            shortcut_row(ui, "Ctrl+S", "Сохранить все изменения");
                            shortcut_row(ui, "Ctrl+N", "Создать правило");
                            shortcut_row(ui, "Ctrl+Enter", "Применить изменения правила");
                            shortcut_row(ui, "Ctrl+F", "Перейти к поиску");
                            shortcut_row(ui, "Ctrl+D", "Дублировать правило");
                            shortcut_row(ui, "Ctrl+Shift+D", "Удалить правило");
                            shortcut_row(ui, "Ctrl+R", "Обновить файлы с диска");
                            shortcut_row(ui, "Ctrl+L", "Показать или скрыть проверку");
                            shortcut_row(ui, "F1", "Открыть эту справку");
                        });
                });
            self.show_shortcuts = open;
        }

        if self.confirm_delete {
            let mut open = self.confirm_delete;
            let mut delete = false;
            let mut cancel = false;
            egui::Window::new("Удаление правила")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(context, |ui| {
                    ui.label("Удалить выбранное правило из рабочей копии?");
                    ui.label("Изменение попадёт на диск только после сохранения.");
                    ui.horizontal(|ui| {
                        if ui.button("Удалить").clicked() {
                            delete = true;
                        }
                        if ui.button("Отмена").clicked() {
                            cancel = true;
                        }
                    });
                });
            if delete {
                self.delete_selected();
                open = false;
            } else if cancel {
                open = false;
            }
            self.confirm_delete = open;
        }

        if self.confirm_reload {
            let mut open = self.confirm_reload;
            let mut reload = false;
            let mut cancel = false;
            egui::Window::new("Есть несохранённые изменения")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(context, |ui| {
                    ui.label("Обновление с диска отменит все несохранённые изменения.");
                    ui.horizontal(|ui| {
                        if ui.button("Отменить изменения и обновить").clicked()
                        {
                            reload = true;
                        }
                        if ui.button("Вернуться").clicked() {
                            cancel = true;
                        }
                    });
                });
            if reload {
                self.reload();
                open = false;
            } else if cancel {
                open = false;
            }
            self.confirm_reload = open;
        }
    }
}

impl eframe::App for MatchStudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui.ctx());
        self.top_bar(ui);
        self.rules_panel(ui);
        self.diagnostics_panel(ui);
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(self.status.as_str());
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("Конфигурация: {}", self.config_root.display()))
                        .weak(),
                );
            });
        });
        self.central_editor(ui);
        self.dialogs(ui.ctx());
    }
}

fn shortcut_row(ui: &mut egui::Ui, shortcut: &str, description: &str) {
    ui.monospace(shortcut);
    ui.label(description);
    ui.end_row();
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn ru_message(message: &str) -> String {
    let replacements = [
        ("match directory does not exist", "папка правил не существует"),
        (
            "match file is not part of the workspace",
            "YAML-файл не входит в рабочую область",
        ),
        ("match no longer exists", "правило больше не существует"),
        ("invalid YAML in", "ошибка YAML в файле"),
        (
            "the raw YAML block must contain exactly one match",
            "блок YAML должен содержать ровно одно правило",
        ),
        (
            "file changed outside Match Studio and was not overwritten",
            "файл был изменён другой программой и не перезаписан",
        ),
        ("unable to save", "не удалось сохранить"),
        ("YAML parse error", "ошибка разбора YAML"),
        ("Empty trigger", "пустой триггер"),
        ("Empty regexp", "пустое регулярное выражение"),
        ("Invalid regexp", "ошибка регулярного выражения"),
        (
            "Duplicate match cause",
            "повторяющееся условие срабатывания",
        ),
        ("Missing import", "не найден импортируемый файл"),
        ("Import cycle", "циклический импорт"),
        (
            "Capture name must be a valid identifier",
            "имя переменной должно начинаться с латинской буквы или подчёркивания и содержать только латинские буквы, цифры и подчёркивания",
        ),
    ];
    let mut translated = message.to_owned();
    for (source, target) in replacements {
        translated = translated.replace(source, target);
    }
    translated
}
