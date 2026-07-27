use eframe::egui;
use rhai::{Dynamic, Engine, Scope, AST};
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    ffi::OsString,
    fs,
    hash::{Hash, Hasher},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};
use walkdir::WalkDir;

const SCRIPT_REFRESH_INTERVAL: Duration = Duration::from_secs(3);
const DEFAULT_FILE_NAME: &str = "script.rhai";
const DEFAULT_SCRIPT: &str = r#"let length = input.len;
if length > 0 {
    `Триггер ${trigger}: получено ${length} символов`
} else {
    "Пустая строка"
}"#;

struct CompiledScript {
    source: String,
    ast: AST,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckKind {
    Idle,
    Success,
    Error,
}

struct CheckMessage {
    kind: CheckKind,
    text: String,
}

impl Default for CheckMessage {
    fn default() -> Self {
        Self {
            kind: CheckKind::Idle,
            text: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LintLevel {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LintMessage {
    level: LintLevel,
    line: Option<usize>,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fingerprint {
    length: u64,
    content_hash: u64,
}

struct ExecutionResult {
    rendered: String,
    value_text: String,
}

pub struct RhaiLab {
    engine: Engine,
    scripts_root: PathBuf,
    files: Vec<PathBuf>,
    selected: Option<PathBuf>,
    script: String,
    original: String,
    new_file_name: String,
    file_filter: String,
    show_files_panel: bool,
    file_error: Option<String>,
    file_status: String,
    snapshot: BTreeMap<PathBuf, Fingerprint>,
    next_file_scan: Instant,
    external_change_pending: bool,
    trigger: String,
    input: String,
    clipboard: String,
    expected_output: String,
    expect_contains: bool,
    compiled: Option<CompiledScript>,
    syntax: CheckMessage,
    compilation: CheckMessage,
    execution: CheckMessage,
    test_result: CheckMessage,
    lint: Vec<LintMessage>,
    output: String,
}

impl RhaiLab {
    pub fn new(config_root: &Path) -> Self {
        let mut engine = Engine::new();
        engine.set_max_operations(200_000);

        let scripts_root = config_root.join("scripts");
        let file_error = fs::create_dir_all(&scripts_root).err().map(|error| {
            format!(
                "Не удалось создать папку Rhai-скриптов {}: {error}",
                scripts_root.display()
            )
        });
        let snapshot = capture_scripts(&scripts_root);
        let files = snapshot.keys().cloned().collect::<Vec<_>>();

        let mut lab = Self {
            engine,
            scripts_root,
            files,
            selected: None,
            script: DEFAULT_SCRIPT.to_owned(),
            original: DEFAULT_SCRIPT.to_owned(),
            new_file_name: DEFAULT_FILE_NAME.to_owned(),
            file_filter: String::new(),
            show_files_panel: true,
            file_error,
            file_status: String::new(),
            snapshot,
            next_file_scan: Instant::now() + SCRIPT_REFRESH_INTERVAL,
            external_change_pending: false,
            trigger: ":пример".to_owned(),
            input: "тестовая строка".to_owned(),
            clipboard: "содержимое буфера".to_owned(),
            expected_output: String::new(),
            expect_contains: false,
            compiled: None,
            syntax: CheckMessage::default(),
            compilation: CheckMessage::default(),
            execution: CheckMessage::default(),
            test_result: CheckMessage::default(),
            lint: Vec::new(),
            output: String::new(),
        };

        if let Some(path) = lab.files.first().cloned() {
            if let Err(error) = lab.load_file(path) {
                lab.file_error = Some(error);
            }
        } else {
            lab.lint_current();
        }

        lab
    }

    pub fn dirty(&self) -> bool {
        self.script != self.original
    }

    pub fn compile_current(&mut self) {
        self.lint_current();
        match compile_source(&self.engine, &self.script) {
            Ok(ast) => {
                self.compiled = Some(CompiledScript {
                    source: self.script.clone(),
                    ast,
                });
                self.syntax = success("Синтаксис корректен");
                self.compilation = success("Скрипт скомпилирован в AST и готов к запуску");
                self.execution = CheckMessage::default();
                self.test_result = CheckMessage::default();
            }
            Err(error) => {
                self.compiled = None;
                self.syntax = failure(format!("Ошибка синтаксиса: {error}"));
                self.compilation = failure("Компиляция не выполнена из-за ошибки синтаксиса");
                self.execution = CheckMessage::default();
                self.test_result = CheckMessage::default();
                self.output.clear();
            }
        }
    }

    pub fn run_current(&mut self) {
        match self.evaluate_current() {
            Ok(result) => {
                self.output = result.rendered;
                self.execution = success("Скрипт выполнен без ошибок");
                self.test_result = CheckMessage::default();
            }
            Err(error) => {
                self.output.clear();
                self.execution = failure(format!("Ошибка выполнения: {error}"));
                self.test_result = CheckMessage::default();
            }
        }
    }

    pub fn save_current(&mut self) -> Result<String, String> {
        self.save_impl(false)
    }

    pub fn save_and_check_current(&mut self) -> Result<String, String> {
        self.save_impl(true)
    }

    pub fn reload_current(&mut self) -> Result<String, String> {
        let path = self
            .selected
            .clone()
            .ok_or_else(|| "Сначала выберите сохранённый Rhai-скрипт".to_owned())?;
        self.load_file(path.clone())?;
        Ok(format!("Rhai-скрипт перечитан: {}", path.display()))
    }

    pub fn start_new_script(&mut self) -> Result<String, String> {
        if self.dirty() {
            return Err("Сначала сохраните или отмените изменения текущего скрипта".to_owned());
        }
        self.selected = None;
        DEFAULT_SCRIPT.clone_into(&mut self.script);
        DEFAULT_SCRIPT.clone_into(&mut self.original);
        DEFAULT_FILE_NAME.clone_into(&mut self.new_file_name);
        self.external_change_pending = false;
        self.file_error = None;
        "Новый Rhai-скрипт: задайте имя и сохраните файл".clone_into(&mut self.file_status);
        self.invalidate();
        self.lint_current();
        Ok(self.file_status.clone())
    }

    pub fn ui(&mut self, root: &mut egui::Ui, app_status: &mut String) {
        self.poll_external_changes(root.ctx());

        if self.show_files_panel {
            self.files_panel(root, app_status);
        }

        egui::CentralPanel::default().show(root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("Rhai IDE");
                    if ui
                        .button(if self.show_files_panel {
                            "Скрыть скрипты"
                        } else {
                            "Показать скрипты"
                        })
                        .clicked()
                    {
                        self.show_files_panel = !self.show_files_panel;
                    }
                    if self.dirty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(210, 135, 25),
                            "Есть несохранённые изменения",
                        );
                    }
                });
                ui.label(
                    "Выбирайте реальные .rhai-файлы из папки scripts, редактируйте, проверяйте, запускайте и сохраняйте их в одном окне.",
                );
                ui.label(
                    egui::RichText::new(format!(
                        "Папка скриптов: {}",
                        self.scripts_root.display()
                    ))
                    .weak(),
                );
                if let Some(path) = &self.selected {
                    ui.monospace(path.display().to_string());
                } else {
                    ui.monospace("Новый несохранённый скрипт");
                }

                if self.external_change_pending {
                    ui.colored_label(
                        egui::Color32::from_rgb(210, 135, 25),
                        "Текущий файл изменён или удалён другой программой. Сохранение защищено от перезаписи.",
                    );
                }
                if let Some(error) = &self.file_error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                } else if !self.file_status.is_empty() {
                    ui.label(egui::RichText::new(self.file_status.as_str()).weak());
                }

                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button("Сохранить")
                        .on_hover_text("Ctrl+S")
                        .clicked()
                    {
                        let result = self.save_current();
                        self.report_file_action(result, app_status);
                    }
                    if ui.button("Сохранить и проверить").clicked() {
                        let result = self.save_and_check_current();
                        self.report_file_action(result, app_status);
                    }
                    if ui
                        .add_enabled(
                            self.selected.is_some(),
                            egui::Button::new("Обновить с диска"),
                        )
                        .on_hover_text("Ctrl+R. Локальные изменения будут отброшены")
                        .clicked()
                    {
                        let result = self.reload_current();
                        self.report_file_action(result, app_status);
                    }
                    if ui.button("Новый скрипт").on_hover_text("Ctrl+N").clicked() {
                        let result = self.start_new_script();
                        self.report_file_action(result, app_status);
                    }
                });

                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Примеры:");
                    if ui.button("Условие и строка").clicked() {
                        self.replace_script(DEFAULT_SCRIPT);
                    }
                    if ui.button("Арифметика").clicked() {
                        self.replace_script("let total = 40 + 2; total");
                    }
                    if ui.button("Функция").clicked() {
                        self.replace_script(
                            "fn normalize(value) { value.trim().to_lower() }\nnormalize(input)",
                        );
                    }
                    if ui.button("Пример ошибки").clicked() {
                        self.replace_script("let value = ;");
                    }
                });

                egui::CollapsingHeader::new("Тестовый контекст")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("rhai_variables")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.monospace("trigger");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.trigger)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.end_row();
                                ui.monospace("input");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.input)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.end_row();
                                ui.monospace("clipboard");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.clipboard)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.end_row();
                                ui.monospace("ожидается");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.expected_output)
                                        .desired_width(f32::INFINITY)
                                        .hint_text("необязательно: ожидаемое значение результата"),
                                );
                                ui.end_row();
                            });
                        ui.checkbox(
                            &mut self.expect_contains,
                            "Проверять вхождение вместо точного совпадения",
                        );
                    });

                ui.add_space(8.0);
                ui.label("Исходный Rhai-скрипт");
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut self.script)
                            .code_editor()
                            .desired_rows(22)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    self.invalidate();
                    self.lint_current();
                }

                ui.horizontal_wrapped(|ui| {
                    if ui.button("Проверить всё").clicked() {
                        self.compile_current();
                    }
                    if ui.button("Синтаксис").clicked() {
                        self.check_syntax();
                    }
                    if ui.button("Линтер").clicked() {
                        self.lint_current();
                    }
                    if ui
                        .button("Скомпилировать")
                        .on_hover_text("Ctrl+Shift+Enter")
                        .clicked()
                    {
                        self.compile_current();
                    }
                    if ui
                        .add_sized(
                            [150.0, 30.0],
                            egui::Button::new(egui::RichText::new("Запустить").strong()),
                        )
                        .on_hover_text("Ctrl+Enter")
                        .clicked()
                    {
                        self.run_current();
                    }
                    if ui.button("Запустить тест").clicked() {
                        self.run_test_current();
                    }
                });

                show_check(ui, "Синтаксис", &self.syntax);
                show_check(ui, "Компиляция", &self.compilation);
                show_check(ui, "Выполнение", &self.execution);
                show_check(ui, "Тест", &self.test_result);
                self.show_lint(ui);

                if !self.output.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("Результат").strong());
                    ui.add(
                        egui::TextEdit::multiline(&mut self.output)
                            .code_editor()
                            .desired_rows(6)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                }

                if self.execution.kind == CheckKind::Success {
                    "Rhai-скрипт успешно скомпилирован и выполнен".clone_into(app_status);
                } else if self.compilation.kind == CheckKind::Success {
                    "Rhai-скрипт успешно скомпилирован".clone_into(app_status);
                } else if self.syntax.kind == CheckKind::Error {
                    "Rhai-скрипт содержит ошибку синтаксиса".clone_into(app_status);
                }
            });
        });
    }

    fn files_panel(&mut self, root: &mut egui::Ui, app_status: &mut String) {
        let selected = self.selected.clone();
        egui::Panel::left("rhai_script_files")
            .resizable(true)
            .default_size(330.0)
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Rhai-скрипты");
                    if ui.small_button("Скрыть").clicked() {
                        self.show_files_panel = false;
                    }
                });
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {} файлов",
                        self.scripts_root.display(),
                        self.files.len()
                    ))
                    .weak(),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.file_filter)
                        .desired_width(f32::INFINITY)
                        .hint_text("Фильтр по имени файла…"),
                );
                ui.separator();

                let filter = self.file_filter.to_lowercase();
                egui::ScrollArea::vertical()
                    .id_salt("rhai_script_file_list")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut visible = 0_usize;
                        for path in self.files.clone() {
                            let label = relative_display(&self.scripts_root, &path);
                            if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                                continue;
                            }
                            visible += 1;
                            let is_selected = selected.as_ref() == Some(&path);
                            let label = if is_selected && self.dirty() {
                                format!("{label}  *")
                            } else {
                                label
                            };
                            if ui.selectable_label(is_selected, label).clicked() {
                                let result = self.select_file(path);
                                self.report_file_action(result, app_status);
                            }
                        }
                        if visible == 0 {
                            ui.label("Скрипты по фильтру не найдены");
                        }
                    });

                ui.separator();
                ui.label("Новый файл");
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_file_name)
                        .desired_width(f32::INFINITY)
                        .hint_text("например: medical/days.rhai"),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Новый").clicked() {
                        let result = self.start_new_script();
                        self.report_file_action(result, app_status);
                    }
                    if ui.button("Обновить список").clicked() {
                        let message = self.refresh_file_list();
                        message.clone_into(app_status);
                    }
                });
            });
    }

    fn select_file(&mut self, path: PathBuf) -> Result<String, String> {
        if self.dirty() {
            return Err("Сначала сохраните или отмените изменения текущего скрипта".to_owned());
        }
        self.load_file(path.clone())?;
        Ok(format!("Открыт Rhai-скрипт: {}", path.display()))
    }

    fn load_file(&mut self, path: PathBuf) -> Result<(), String> {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("Не удалось открыть {}: {error}", path.display()))?;
        self.selected = Some(path.clone());
        self.script.clone_from(&content);
        self.original = content;
        self.new_file_name = relative_display(&self.scripts_root, &path);
        self.file_error = None;
        self.file_status = format!("Открыт: {}", path.display());
        self.external_change_pending = false;
        self.invalidate();
        self.lint_current();
        self.refresh_snapshot();
        Ok(())
    }

    fn save_impl(&mut self, check_before_save: bool) -> Result<String, String> {
        if check_before_save {
            self.compile_current();
            if self.syntax.kind == CheckKind::Error {
                return Err("Сохранение остановлено: исправьте синтаксис Rhai".to_owned());
            }
        }

        let path = match self.selected.clone() {
            Some(path) => path,
            None => self
                .scripts_root
                .join(normalize_script_name(&self.new_file_name)?),
        };

        if let Some(selected) = &self.selected {
            let current = fs::read_to_string(selected).map_err(|error| {
                format!("Не удалось перечитать {}: {error}", selected.display())
            })?;
            if current != self.original {
                self.external_change_pending = true;
                return Err(
                    "Файл был изменён другой программой. Обновите его с диска перед сохранением"
                        .to_owned(),
                );
            }
            fs::write(backup_path(selected), current).map_err(|error| {
                format!(
                    "Не удалось создать резервную копию {}: {error}",
                    selected.display()
                )
            })?;
        } else if path.exists() {
            return Err(format!("Файл уже существует: {}", path.display()));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("Не удалось создать папку {}: {error}", parent.display())
            })?;
        }
        fs::write(&path, &self.script)
            .map_err(|error| format!("Не удалось сохранить {}: {error}", path.display()))?;

        self.selected = Some(path.clone());
        self.original.clone_from(&self.script);
        self.new_file_name = relative_display(&self.scripts_root, &path);
        self.external_change_pending = false;
        self.file_error = None;
        self.refresh_snapshot();
        let message = format!("Rhai-скрипт сохранён: {}", path.display());
        self.file_status.clone_from(&message);
        Ok(message)
    }

    fn refresh_file_list(&mut self) -> String {
        self.refresh_snapshot();
        let message = format!("Найдено Rhai-скриптов: {}", self.files.len());
        self.file_status.clone_from(&message);
        message
    }

    fn refresh_snapshot(&mut self) {
        self.snapshot = capture_scripts(&self.scripts_root);
        self.files = self.snapshot.keys().cloned().collect();
    }

    fn poll_external_changes(&mut self, context: &egui::Context) {
        let now = Instant::now();
        if now < self.next_file_scan {
            context.request_repaint_after(self.next_file_scan.saturating_duration_since(now));
            return;
        }
        self.next_file_scan = now + SCRIPT_REFRESH_INTERVAL;
        context.request_repaint_after(SCRIPT_REFRESH_INTERVAL);

        let current = capture_scripts(&self.scripts_root);
        if current == self.snapshot {
            return;
        }

        let selected_changed = self
            .selected
            .as_ref()
            .is_some_and(|path| self.snapshot.get(path) != current.get(path));
        self.snapshot = current;
        self.files = self.snapshot.keys().cloned().collect();

        if !selected_changed {
            return;
        }

        let Some(path) = self.selected.clone() else {
            return;
        };
        if self.dirty() {
            self.external_change_pending = true;
            "Текущий Rhai-файл изменился снаружи; локальный текст оставлен без изменений"
                .clone_into(&mut self.file_status);
            return;
        }

        if path.is_file() {
            if let Err(error) = self.load_file(path) {
                self.file_error = Some(error);
            } else {
                "Rhai-файл автоматически обновлён с диска".clone_into(&mut self.file_status);
            }
        } else {
            self.selected = None;
            DEFAULT_SCRIPT.clone_into(&mut self.script);
            DEFAULT_SCRIPT.clone_into(&mut self.original);
            self.external_change_pending = false;
            self.invalidate();
            self.lint_current();
            "Открытый Rhai-файл был удалён с диска".clone_into(&mut self.file_status);
        }
    }

    fn evaluate_current(&mut self) -> Result<ExecutionResult, String> {
        let needs_compile = self
            .compiled
            .as_ref()
            .is_none_or(|compiled| compiled.source != self.script);
        if needs_compile {
            self.compile_current();
        }
        let compiled = self
            .compiled
            .as_ref()
            .ok_or_else(|| "Скрипт не скомпилирован".to_owned())?;

        let mut scope = Scope::new();
        scope.push("trigger", self.trigger.clone());
        scope.push("input", self.input.clone());
        scope.push("clipboard", self.clipboard.clone());
        execute_ast(&self.engine, &mut scope, &compiled.ast)
    }

    fn run_test_current(&mut self) {
        match self.evaluate_current() {
            Ok(result) => {
                self.output = result.rendered;
                self.execution = success("Скрипт выполнен без ошибок");
                if self.expected_output.is_empty() {
                    self.test_result = success(
                        "Тест выполнен; заполните ожидаемое значение для автоматической проверки",
                    );
                    return;
                }
                let passed = if self.expect_contains {
                    result.value_text.contains(&self.expected_output)
                } else {
                    result.value_text == self.expected_output
                };
                self.test_result = if passed {
                    success("Ожидаемый результат получен")
                } else {
                    failure(format!(
                        "Ожидалось: {:?}; получено: {:?}",
                        self.expected_output, result.value_text
                    ))
                };
            }
            Err(error) => {
                self.output.clear();
                self.execution = failure(format!("Ошибка выполнения: {error}"));
                self.test_result = failure("Тест не выполнен");
            }
        }
    }

    fn check_syntax(&mut self) {
        match compile_source(&self.engine, &self.script) {
            Ok(_) => self.syntax = success("Синтаксис корректен"),
            Err(error) => self.syntax = failure(format!("Ошибка синтаксиса: {error}")),
        }
    }

    fn lint_current(&mut self) {
        self.lint = lint_source(&self.engine, &self.script);
    }

    fn show_lint(&self, ui: &mut egui::Ui) {
        let errors = self
            .lint
            .iter()
            .filter(|message| message.level == LintLevel::Error)
            .count();
        let warnings = self
            .lint
            .iter()
            .filter(|message| message.level == LintLevel::Warning)
            .count();
        let info = self
            .lint
            .iter()
            .filter(|message| message.level == LintLevel::Info)
            .count();
        let title = format!("Линтер: ошибок {errors}, предупреждений {warnings}, сведений {info}");
        egui::CollapsingHeader::new(title)
            .default_open(errors > 0 || warnings > 0)
            .show(ui, |ui| {
                if self.lint.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(40, 150, 90), "Замечаний не найдено");
                    return;
                }
                for message in &self.lint {
                    let prefix = match message.level {
                        LintLevel::Error => "ОШИБКА",
                        LintLevel::Warning => "ВНИМАНИЕ",
                        LintLevel::Info => "СВЕДЕНИЕ",
                    };
                    let location = message
                        .line
                        .map_or_else(String::new, |line| format!(" · строка {line}"));
                    let text = format!("{prefix}{location}: {}", message.text);
                    match message.level {
                        LintLevel::Error => {
                            ui.colored_label(ui.visuals().error_fg_color, text);
                        }
                        LintLevel::Warning => {
                            ui.colored_label(egui::Color32::from_rgb(210, 135, 25), text);
                        }
                        LintLevel::Info => {
                            ui.label(egui::RichText::new(text).weak());
                        }
                    }
                }
            });
    }

    fn replace_script(&mut self, source: &str) {
        source.clone_into(&mut self.script);
        self.invalidate();
        self.lint_current();
    }

    fn invalidate(&mut self) {
        self.compiled = None;
        self.syntax = CheckMessage::default();
        self.compilation = CheckMessage::default();
        self.execution = CheckMessage::default();
        self.test_result = CheckMessage::default();
        self.output.clear();
    }

    fn report_file_action(&mut self, result: Result<String, String>, app_status: &mut String) {
        match result {
            Ok(message) => {
                self.file_error = None;
                self.file_status.clone_from(&message);
                message.clone_into(app_status);
            }
            Err(error) => {
                self.file_error = Some(error.clone());
                format!("Rhai: {error}").clone_into(app_status);
            }
        }
    }
}

impl Default for RhaiLab {
    fn default() -> Self {
        Self::new(Path::new("."))
    }
}

fn compile_source(engine: &Engine, source: &str) -> Result<AST, String> {
    engine.compile(source).map_err(|error| error.to_string())
}

fn execute_ast(
    engine: &Engine,
    scope: &mut Scope<'_>,
    ast: &AST,
) -> Result<ExecutionResult, String> {
    engine
        .eval_ast_with_scope::<Dynamic>(scope, ast)
        .map(format_dynamic)
        .map_err(|error| error.to_string())
}

fn format_dynamic(value: Dynamic) -> ExecutionResult {
    let type_name = value.type_name().to_owned();
    let value_text = if value.is_unit() {
        "()".to_owned()
    } else {
        value.to_string()
    };
    ExecutionResult {
        rendered: format!("{value_text}\nТип: {type_name}"),
        value_text,
    }
}

fn lint_source(engine: &Engine, source: &str) -> Vec<LintMessage> {
    let mut messages = Vec::new();
    if source.trim().is_empty() {
        messages.push(lint_message(
            LintLevel::Warning,
            None,
            "Скрипт пуст и вернёт значение ()",
        ));
    }

    if let Err(error) = compile_source(engine, source) {
        messages.push(lint_message(
            LintLevel::Error,
            None,
            format!("Синтаксис не позволяет скомпилировать скрипт: {error}"),
        ));
    }

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if line.trim_end() != line {
            messages.push(lint_message(
                LintLevel::Warning,
                Some(line_number),
                "Пробелы в конце строки",
            ));
        }
        if line.contains('\t') {
            messages.push(lint_message(
                LintLevel::Warning,
                Some(line_number),
                "Использована табуляция; для предсказуемого форматирования лучше пробелы",
            ));
        }
        if line.chars().count() > 120 {
            messages.push(lint_message(
                LintLevel::Info,
                Some(line_number),
                "Строка длиннее 120 символов",
            ));
        }
        if trimmed.contains("eval(") {
            messages.push(lint_message(
                LintLevel::Warning,
                Some(line_number),
                "Динамический eval усложняет проверку и тестирование",
            ));
        }
        let compact = trimmed
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        if compact.contains("whiletrue{") {
            messages.push(lint_message(
                LintLevel::Warning,
                Some(line_number),
                "Обнаружен потенциально бесконечный цикл; выполнение остановит лимит операций",
            ));
        }
        for name in ["trigger", "input", "clipboard"] {
            if declares_name(trimmed, name) {
                messages.push(lint_message(
                    LintLevel::Warning,
                    Some(line_number),
                    format!("Локальная переменная {name} скрывает тестовую переменную среды"),
                ));
            }
        }
        if trimmed.contains("print(") || trimmed.contains("debug(") {
            messages.push(lint_message(
                LintLevel::Info,
                Some(line_number),
                "Вывод print/debug не отображается как результат скрипта; верните значение последним выражением",
            ));
        }
        if trimmed.contains("TODO") || trimmed.contains("FIXME") {
            messages.push(lint_message(
                LintLevel::Info,
                Some(line_number),
                "В скрипте осталось служебное замечание TODO/FIXME",
            ));
        }
    }

    messages
}

fn declares_name(line: &str, name: &str) -> bool {
    ["let ", "const "].iter().any(|prefix| {
        let declaration = format!("{prefix}{name}");
        line.starts_with(&declaration)
            && line[declaration.len()..]
                .chars()
                .next()
                .is_none_or(|character| !character.is_alphanumeric() && character != '_')
    })
}

fn lint_message(level: LintLevel, line: Option<usize>, text: impl Into<String>) -> LintMessage {
    LintMessage {
        level,
        line,
        text: text.into(),
    }
}

fn capture_scripts(root: &Path) -> BTreeMap<PathBuf, Fingerprint> {
    let mut files = BTreeMap::new();
    if !root.is_dir() {
        return files;
    }
    for entry in WalkDir::new(root).follow_links(false) {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !entry.file_type().is_file() || !is_rhai(path) {
            continue;
        }
        let Ok(content) = fs::read(path) else {
            continue;
        };
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        files.insert(
            path.to_path_buf(),
            Fingerprint {
                length: content.len() as u64,
                content_hash: hasher.finish(),
            },
        );
    }
    files
}

fn is_rhai(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("rhai"))
}

fn normalize_script_name(value: &str) -> Result<PathBuf, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Укажите имя нового Rhai-файла".to_owned());
    }
    let mut relative = PathBuf::from(value);
    if relative.extension().is_none() {
        relative.set_extension("rhai");
    }
    if !relative
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rhai"))
    {
        return Err("Имя файла должно оканчиваться на .rhai".to_owned());
    }
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Используйте безопасный относительный путь внутри папки scripts".to_owned());
    }
    Ok(relative)
}

fn backup_path(path: &Path) -> PathBuf {
    let mut value = OsString::from(path.as_os_str());
    value.push(".respanso.bak");
    PathBuf::from(value)
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn success(text: impl Into<String>) -> CheckMessage {
    CheckMessage {
        kind: CheckKind::Success,
        text: text.into(),
    }
}

fn failure(text: impl Into<String>) -> CheckMessage {
    CheckMessage {
        kind: CheckKind::Error,
        text: text.into(),
    }
}

fn show_check(ui: &mut egui::Ui, label: &str, check: &CheckMessage) {
    if check.kind == CheckKind::Idle {
        return;
    }
    let color = if check.kind == CheckKind::Success {
        egui::Color32::from_rgb(40, 150, 90)
    } else {
        ui.visuals().error_fg_color
    };
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{label}:"));
        ui.colored_label(color, check.text.as_str());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_script_compiles_and_runs() {
        let engine = Engine::new();
        let ast = compile_source(&engine, "let value = 40 + 2; value").expect("compile");
        let mut scope = Scope::new();
        let output = execute_ast(&engine, &mut scope, &ast).expect("execute");
        assert_eq!(output.value_text, "42");
    }

    #[test]
    fn invalid_script_is_rejected_during_compilation() {
        let engine = Engine::new();
        let error = compile_source(&engine, "let value = ;").expect_err("invalid script");
        assert!(!error.is_empty());
    }

    #[test]
    fn test_variables_are_available_to_script() {
        let engine = Engine::new();
        let ast = compile_source(&engine, "trigger + input").expect("compile");
        let mut scope = Scope::new();
        scope.push("trigger", ":test".to_owned());
        scope.push("input", " value".to_owned());
        let output = execute_ast(&engine, &mut scope, &ast).expect("execute");
        assert_eq!(output.value_text, ":test value");
    }

    #[test]
    fn script_scanner_is_recursive_and_ignores_other_extensions() {
        let directory = tempdir::TempDir::new("respanso-rhai-files").unwrap();
        let nested = directory.path().join("medical");
        fs::create_dir_all(&nested).unwrap();
        fs::write(directory.path().join("root.rhai"), "40 + 2").unwrap();
        fs::write(nested.join("days.RHAI"), "1 + 1").unwrap();
        fs::write(nested.join("notes.txt"), "ignore").unwrap();
        let scripts = capture_scripts(directory.path());
        assert_eq!(scripts.len(), 2);
    }

    #[test]
    fn linter_reports_syntax_style_and_shadowing_problems() {
        let engine = Engine::new();
        let messages = lint_source(&engine, "let trigger = ;\t \nwhile true { print(\"x\"); }");
        assert!(messages
            .iter()
            .any(|message| message.level == LintLevel::Error));
        assert!(messages
            .iter()
            .any(|message| message.text.contains("скрывает")));
        assert!(messages
            .iter()
            .any(|message| message.text.contains("бесконечный")));
    }

    #[test]
    fn new_script_name_must_stay_inside_scripts_directory() {
        assert_eq!(
            normalize_script_name("medical/days").unwrap(),
            PathBuf::from("medical/days.rhai")
        );
        assert!(normalize_script_name("../outside.rhai").is_err());
        assert!(normalize_script_name("wrong.txt").is_err());
    }
}
