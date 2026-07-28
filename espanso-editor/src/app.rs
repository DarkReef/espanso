use crate::{
    config_transfer::{self, PackageSummary},
    diagnostics::{collect_project_diagnostics, DiagnosticManager, DiagnosticState},
    dynamic_variables::{self, DynamicVariableAction, DynamicVariableDialog},
    file_monitor::{FileMonitor, PollResult},
    rhai_lab::RhaiLab,
    runtime::RuntimeMonitor,
    settings::SettingsEditor,
    storm_logo,
    workspace::{
        DiagnosticLevel, MatchKind, MatchWorkspace, PlaygroundResult, RegexBuilderSpec,
        RegexExampleResult, RuleDraft, RuleId,
    },
    yaml_imports,
};
use eframe::egui;
use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const APP_TITLE: &str = "rEspanso Match Studio";
const FILE_CHECK_INTERVAL: Duration = Duration::from_secs(3);
const FILE_STABILITY_RECHECK: Duration = Duration::from_millis(850);

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
        Box::new(move |_creation_context| Ok(Box::new(MatchStudioApp::new(config_root)))),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainTab {
    Rules,
    Settings,
    Rhai,
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
    active_tab: MainTab,
    runtime: RuntimeMonitor,
    settings: SettingsEditor,
    rhai_lab: RhaiLab,
    filter: String,
    focus_filter: bool,
    file_filter: Option<PathBuf>,
    yaml_file_filter: String,
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
    show_dynamic_variables: bool,
    dynamic_variables: DynamicVariableDialog,
    show_create_yaml_file: bool,
    new_yaml_file_name: String,
    confirm_delete_yaml_file: bool,
    pending_delete_yaml_file: Option<PathBuf>,
    confirm_delete: bool,
    confirm_reload: bool,
    reload_all_after_confirm: bool,
    file_monitor: FileMonitor,
    diagnostics: DiagnosticManager,
    next_file_check: Instant,
    external_change_pending: bool,
    show_config_transfer: bool,
    config_packages: Vec<PathBuf>,
    selected_config_package: Option<PathBuf>,
    selected_config_summary: Option<PackageSummary>,
    config_transfer_status: String,
    pending_config_import: Option<PathBuf>,
    confirm_config_import: bool,
}

impl MatchStudioApp {
    fn new(config_root: PathBuf) -> Self {
        let (workspace, load_error) = match MatchWorkspace::load(config_root.clone()) {
            Ok(workspace) => (Some(workspace), None),
            Err(error) => (None, Some(ru_message(&error.to_string()))),
        };
        let settings = SettingsEditor::load(&config_root);
        let rhai_lab = RhaiLab::new(&config_root);
        let now = Instant::now();
        let file_monitor = FileMonitor::new(&config_root, now);
        let mut diagnostics = DiagnosticManager::default();
        diagnostics.reconcile(
            collect_project_diagnostics(&config_root, workspace.as_ref()),
            &config_root,
            workspace.as_ref(),
        );
        let config_packages = config_transfer::list_packages(&config_root).unwrap_or_default();
        Self {
            config_root,
            workspace,
            load_error,
            active_tab: MainTab::Rules,
            runtime: RuntimeMonitor::new(),
            settings,
            rhai_lab,
            filter: String::new(),
            focus_filter: false,
            file_filter: None,
            yaml_file_filter: String::new(),
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
            show_dynamic_variables: false,
            dynamic_variables: DynamicVariableDialog::default(),
            show_create_yaml_file: false,
            new_yaml_file_name: "rules.yml".to_owned(),
            confirm_delete_yaml_file: false,
            pending_delete_yaml_file: None,
            confirm_delete: false,
            confirm_reload: false,
            reload_all_after_confirm: false,
            file_monitor,
            diagnostics,
            next_file_check: now + FILE_CHECK_INTERVAL,
            external_change_pending: false,
            show_config_transfer: false,
            config_packages,
            selected_config_package: None,
            selected_config_summary: None,
            config_transfer_status: String::new(),
            pending_config_import: None,
            confirm_config_import: false,
        }
    }

    fn reload(&mut self) {
        match MatchWorkspace::load(self.config_root.clone()) {
            Ok(workspace) => {
                self.workspace = Some(workspace);
                self.load_error = None;
                self.selected = None;
                self.raw_rule.clear();
                self.file_monitor.refresh(&self.config_root, Instant::now());
                self.validate_project();
                "Правила перечитаны с диска".clone_into(&mut self.status);
            }
            Err(error) => {
                let message = ru_message(&error.to_string());
                self.load_error = Some(message.clone());
                self.status = format!("Не удалось обновить правила: {message}");
            }
        }
    }

    fn has_unsaved_changes(&self) -> bool {
        self.workspace
            .as_ref()
            .is_some_and(|workspace| !workspace.dirty_files().is_empty())
            || self.settings.dirty()
    }

    fn reload_all_from_disk(&mut self) {
        self.reload();
        if self.load_error.is_some() {
            self.external_change_pending = true;
            return;
        }
        self.settings = SettingsEditor::load(&self.config_root);
        self.rhai_lab = RhaiLab::new(&self.config_root);
        self.external_change_pending = false;
        "Обнаружены устойчивые изменения файлов; Studio обновила проект"
            .clone_into(&mut self.status);
    }

    fn validate_project(&mut self) {
        let diagnostics = collect_project_diagnostics(&self.config_root, self.workspace.as_ref());
        self.diagnostics
            .reconcile(diagnostics, &self.config_root, self.workspace.as_ref());
    }

    fn has_transfer_unsaved_changes(&self) -> bool {
        self.workspace
            .as_ref()
            .is_some_and(|workspace| !workspace.dirty_files().is_empty())
            || self.rhai_lab.dirty()
    }

    fn refresh_config_packages(&mut self) {
        match config_transfer::list_packages(&self.config_root) {
            Ok(packages) => {
                self.config_packages = packages;
                if let Some(selected) = self.selected_config_package.clone() {
                    if selected.is_file() {
                        self.select_config_package(selected);
                    } else {
                        self.selected_config_package = None;
                        self.selected_config_summary = None;
                    }
                }
            }
            Err(error) => self.config_transfer_status = error,
        }
    }

    fn select_config_package(&mut self, path: PathBuf) {
        match config_transfer::inspect_package(&path) {
            Ok(summary) => {
                self.selected_config_package = Some(path);
                self.selected_config_summary = Some(summary);
                self.config_transfer_status.clear();
            }
            Err(error) => {
                self.selected_config_package = Some(path);
                self.selected_config_summary = None;
                self.config_transfer_status = error;
            }
        }
    }

    fn handle_dropped_config_packages(&mut self, context: &egui::Context) {
        let dropped = context.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if let Some(path) = dropped
            .into_iter()
            .find(|path| config_transfer::is_package_path(path))
        {
            self.show_config_transfer = true;
            self.select_config_package(path);
            "Пакет конфигурации получен перетаскиванием".clone_into(&mut self.status);
        }
    }

    fn export_config_package(&mut self) {
        if self.has_transfer_unsaved_changes() {
            "Экспорт остановлен: сначала сохраните изменения правил и Rhai-скрипта"
                .clone_into(&mut self.config_transfer_status);
            return;
        }
        match config_transfer::export_current(&self.config_root) {
            Ok(summary) => {
                let path = summary.path.clone();
                self.config_transfer_status = format!(
                    "Экспортировано файлов: {} · {} КБ · {}",
                    summary.file_count,
                    summary.total_bytes.div_ceil(1024),
                    path.display()
                );
                self.status.clone_from(&self.config_transfer_status);
                self.refresh_config_packages();
                self.select_config_package(path);
            }
            Err(error) => {
                self.config_transfer_status = format!("Экспорт не выполнен: {error}");
                self.status.clone_from(&self.config_transfer_status);
            }
        }
    }

    fn perform_config_import(&mut self) {
        let Some(package) = self.pending_config_import.take() else {
            self.confirm_config_import = false;
            return;
        };
        match config_transfer::import_package(&self.config_root, &package) {
            Ok(report) => {
                let (workspace, load_error) = match MatchWorkspace::load(self.config_root.clone()) {
                    Ok(workspace) => (Some(workspace), None),
                    Err(error) => (None, Some(ru_message(&error.to_string()))),
                };
                self.workspace = workspace;
                self.load_error = load_error;
                self.settings = SettingsEditor::load(&self.config_root);
                self.rhai_lab = RhaiLab::new(&self.config_root);
                self.file_monitor = FileMonitor::new(&self.config_root, Instant::now());
                self.next_file_check = Instant::now() + FILE_CHECK_INTERVAL;
                self.external_change_pending = false;
                self.selected = None;
                self.raw_rule.clear();
                self.file_filter = None;
                self.yaml_file_filter.clear();
                self.refresh_config_packages();
                self.select_config_package(report.package.clone());
                self.validate_project();

                let warning = if report.cleanup_warnings.is_empty() {
                    String::new()
                } else {
                    format!(
                        " Предупреждения очистки: {}",
                        report.cleanup_warnings.join("; ")
                    )
                };
                self.config_transfer_status = format!(
                    "Импортировано файлов: {}. Резервная копия: {}. Перезапустите rEspanso.{}",
                    report.file_count,
                    report.backup.display(),
                    warning
                );
                self.status.clone_from(&self.config_transfer_status);
            }
            Err(error) => {
                self.config_transfer_status = format!("Импорт отменён: {error}");
                self.status.clone_from(&self.config_transfer_status);
            }
        }
        self.confirm_config_import = false;
    }

    fn config_transfer_dialog(&mut self, context: &egui::Context) {
        if !self.show_config_transfer {
            return;
        }

        let mut open = self.show_config_transfer;
        let packages = self.config_packages.clone();
        let selected = self.selected_config_package.clone();
        let summary = self.selected_config_summary.clone();
        let unsaved = self.has_transfer_unsaved_changes();
        let mut export = false;
        let mut refresh = false;
        let mut open_folder = false;
        let mut choose = None;
        let mut request_import = false;

        egui::Window::new("Импорт и экспорт конфигурации")
            .open(&mut open)
            .default_width(760.0)
            .default_height(560.0)
            .resizable(true)
            .show(context, |ui| {
                ui.label(
                    "Пакет .respanso-config включает только match/**/*.yml|yaml и scripts/**/*.rhai. Внутренний config не переносится.",
                );
                ui.label(
                    egui::RichText::new(format!(
                        "Папка обмена: {}",
                        config_transfer::exchange_dir(&self.config_root).display()
                    ))
                    .weak(),
                );
                ui.label(
                    egui::RichText::new(
                        "Можно перетащить пакет из Проводника прямо в окно Studio.",
                    )
                    .weak(),
                );

                if unsaved {
                    ui.colored_label(
                        egui::Color32::from_rgb(210, 135, 25),
                        "Сначала сохраните изменения: экспорт и импорт работают с состоянием на диске.",
                    );
                }

                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!unsaved, egui::Button::new("Экспортировать пакет"))
                        .clicked()
                    {
                        export = true;
                    }
                    if ui.button("Открыть папку обмена").clicked() {
                        open_folder = true;
                    }
                    if ui.button("Обновить список").clicked() {
                        refresh = true;
                    }
                });

                if !self.config_transfer_status.is_empty() {
                    ui.label(self.config_transfer_status.as_str());
                }
                ui.separator();
                ui.columns(2, |columns| {
                    columns[0].heading(format!("Пакеты ({})", packages.len()));
                    egui::ScrollArea::vertical()
                        .id_salt("config_package_list")
                        .max_height(360.0)
                        .show(&mut columns[0], |ui| {
                            for path in &packages {
                                let label = path
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    .unwrap_or("пакет конфигурации");
                                if ui
                                    .selectable_label(selected.as_ref() == Some(path), label)
                                    .clicked()
                                {
                                    choose = Some(path.clone());
                                }
                            }
                            if packages.is_empty() {
                                ui.label("Экспортированных пакетов пока нет");
                            }
                        });

                    columns[1].heading("Выбранный пакет");
                    if let Some(path) = &selected {
                        columns[1].label(path.display().to_string());
                    } else {
                        columns[1].label("Пакет не выбран");
                    }
                    if let Some(summary) = &summary {
                        columns[1].separator();
                        columns[1].label(format!("Файлов: {}", summary.file_count));
                        columns[1].label(format!(
                            "Объём данных: {} КБ",
                            summary.total_bytes.div_ceil(1024)
                        ));
                        columns[1].label(format!(
                            "Время экспорта (Unix): {}",
                            summary.exported_at_unix
                        ));
                    }
                    columns[1].add_space(12.0);
                    if columns[1]
                        .add_enabled(
                            !unsaved && summary.is_some(),
                            egui::Button::new("Импортировать выбранный пакет"),
                        )
                        .clicked()
                    {
                        request_import = true;
                    }
                    columns[1].label(
                        egui::RichText::new(
                            "Перед импортом Studio автоматически создаст резервный пакет текущей конфигурации.",
                        )
                        .weak(),
                    );
                });
            });

        self.show_config_transfer = open;
        if open_folder {
            match config_transfer::open_exchange_dir(&self.config_root) {
                Ok(()) => "Папка обмена открыта".clone_into(&mut self.config_transfer_status),
                Err(error) => self.config_transfer_status = error,
            }
        }
        if refresh {
            self.refresh_config_packages();
        }
        if export {
            self.export_config_package();
        }
        if let Some(path) = choose {
            self.select_config_package(path);
        }
        if request_import {
            self.pending_config_import = selected;
            self.confirm_config_import = self.pending_config_import.is_some();
        }
    }

    fn config_import_confirmation(&mut self, context: &egui::Context) {
        if !self.confirm_config_import {
            return;
        }
        let mut open = self.confirm_config_import;
        let package = self.pending_config_import.clone();
        let mut import = false;
        let mut cancel = false;
        egui::Window::new("Импорт конфигурации rEspanso")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(context, |ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(210, 105, 35),
                    "Текущие папки match и scripts будут заменены. Внутренний config останется без изменений.",
                );
                if let Some(path) = &package {
                    ui.label(format!("Пакет: {}", path.display()));
                }
                ui.label(
                    "Сначала будет создан автоматический резервный пакет. При ошибке исходные папки восстанавливаются.",
                );
                ui.horizontal(|ui| {
                    if ui
                        .button("Создать резервную копию и импортировать")
                        .clicked()
                    {
                        import = true;
                    }
                    if ui.button("Отмена").clicked() {
                        cancel = true;
                    }
                });
            });
        if import {
            self.perform_config_import();
            open = false;
        } else if cancel {
            self.pending_config_import = None;
            open = false;
        }
        self.confirm_config_import = open;
    }

    fn request_external_reload(&mut self) {
        if self.has_unsaved_changes() || self.rhai_lab.dirty() {
            self.reload_all_after_confirm = true;
            self.confirm_reload = true;
        } else {
            self.reload_all_from_disk();
        }
    }

    fn check_external_file_changes(&mut self, context: &egui::Context) {
        let now = Instant::now();
        if now < self.next_file_check {
            context.request_repaint_after(self.next_file_check.saturating_duration_since(now));
            return;
        }

        match self.file_monitor.poll(&self.config_root, now) {
            PollResult::Unchanged => {
                self.next_file_check = now + FILE_CHECK_INTERVAL;
            }
            PollResult::Pending { changed_files } => {
                self.next_file_check = now + FILE_STABILITY_RECHECK;
                self.status =
                    format!("Файлы изменяются: ожидается устойчивый хеш ({changed_files})");
            }
            PollResult::Audit => {
                self.next_file_check = now + FILE_CHECK_INTERVAL;
                if !self.has_transfer_unsaved_changes() && !self.settings.dirty() {
                    self.validate_project();
                }
            }
            PollResult::StableChanged { changed_files } => {
                self.next_file_check = now + FILE_CHECK_INTERVAL;
                if self.has_unsaved_changes() || self.rhai_lab.dirty() {
                    self.external_change_pending = true;
                    self.status = format!(
                        "Обнаружены внешние изменения ({changed_files}). Сохраните или отмените локальные изменения, затем обновите проект"
                    );
                } else {
                    self.reload_all_from_disk();
                    self.status = format!(
                        "Проект проверен после устойчивого изменения файлов: изменено {changed_files}"
                    );
                }
            }
        }
        context.request_repaint_after(
            self.next_file_check
                .saturating_duration_since(Instant::now()),
        );
    }

    fn request_reload(&mut self) {
        let has_unsaved = self
            .workspace
            .as_ref()
            .is_some_and(|workspace| !workspace.dirty_files().is_empty());
        if has_unsaved {
            self.reload_all_after_confirm = false;
            self.confirm_reload = true;
        } else {
            self.reload();
        }
    }

    fn save_settings(&mut self) {
        match self.settings.save() {
            Ok(()) => {
                self.file_monitor.refresh(&self.config_root, Instant::now());
                self.validate_project();
                "Настройки rEspanso сохранены. Перезагрузите конфигурацию или перезапустите rEspanso"
                    .clone_into(&mut self.status);
            }
            Err(error) => self.status = error,
        }
    }

    fn reload_settings(&mut self) {
        if self.settings.dirty() {
            "Настройки не обновлены: сначала сохраните изменения".clone_into(&mut self.status);
            return;
        }
        match self.settings.reload_selected() {
            Ok(()) => {
                self.file_monitor.refresh(&self.config_root, Instant::now());
                self.validate_project();
                "Настройки перечитаны с диска".clone_into(&mut self.status);
            }
            Err(error) => self.status = error,
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
                self.file_monitor.refresh(&self.config_root, Instant::now());
                self.validate_project();
                "Нет изменений для сохранения".clone_into(&mut self.status);
            }
            Ok(saved) => {
                self.file_monitor.refresh(&self.config_root, Instant::now());
                self.validate_project();
                self.status = format!("Сохранено файлов: {}. Резервные копии созданы", saved.len());
            }
            Err(error) => {
                self.status = format!("Сохранение остановлено: {}", ru_message(&error.to_string()));
            }
        }
    }

    fn set_file_import_enabled(&mut self, file: PathBuf, enabled: bool) {
        let display_name = relative_display(&self.config_root, &file);
        let result = (|| -> Result<String, String> {
            let workspace = self
                .workspace
                .as_mut()
                .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
            let files = workspace.files();
            let base_file = yaml_imports::find_base_file(&files, workspace.match_root())
                .ok_or_else(|| "Не найден match/base.yml или match/base.yaml".to_owned())?;
            let base_content = workspace
                .raw_file(&base_file)
                .map_err(|error| ru_message(&error.to_string()))?
                .to_owned();
            let updated = yaml_imports::update_import(&base_content, &base_file, &file, enabled)?;

            if updated != base_content {
                workspace
                    .set_raw_file(&base_file, updated)
                    .map_err(|error| ru_message(&error.to_string()))?;
            }

            Ok(if enabled {
                format!(
                    "Файл {display_name} подключён через base.yml. Сохраните изменения (Ctrl+S)"
                )
            } else {
                format!(
                    "Файл {display_name} исключён из imports base.yml. Сохраните изменения (Ctrl+S)"
                )
            })
        })();

        self.status = match result {
            Ok(message) => message,
            Err(error) => format!("Не удалось изменить imports base.yml: {error}"),
        };
    }

    fn create_yaml_file(&mut self) -> bool {
        if self.has_transfer_unsaved_changes() || self.settings.dirty() {
            "Сначала сохраните текущие изменения, затем создайте YAML-файл"
                .clone_into(&mut self.status);
            return false;
        }

        let name = match normalize_yaml_file_name(&self.new_yaml_file_name) {
            Ok(name) => name,
            Err(error) => {
                self.status = error;
                return false;
            }
        };
        let path = self.config_root.join("match").join(&name);
        let result = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| {
                file.write_all(b"matches:\n  - trigger: \":new\"\n    replace: \"\"\n")
            });
        if let Err(error) = result {
            self.status = if path.exists() {
                format!("YAML-файл {name} уже существует")
            } else {
                format!("Не удалось создать YAML-файл {name}: {error}")
            };
            return false;
        }

        self.reload();
        if self.load_error.is_some() {
            return false;
        }
        self.file_filter = Some(path.clone());
        self.select_rule(RuleId {
            file: path,
            ordinal: 0,
        });
        "rules.yml".clone_into(&mut self.new_yaml_file_name);
        self.status = format!("Создан match\\{name}. Добавьте правила и сохраните их Ctrl+S");
        true
    }

    fn delete_yaml_file(&mut self, path: PathBuf) -> bool {
        if self.has_transfer_unsaved_changes() || self.settings.dirty() {
            "Сначала сохраните текущие изменения, затем удалите YAML-файл"
                .clone_into(&mut self.status);
            return false;
        }

        let Some(workspace) = &mut self.workspace else {
            return false;
        };
        let files = workspace.files();
        if !files.iter().any(|file| file == &path) {
            "Выбранный YAML-файл больше не входит в рабочую область".clone_into(&mut self.status);
            return false;
        }
        let base_file = yaml_imports::find_base_file(&files, workspace.match_root());
        if base_file.as_ref() == Some(&path) {
            "Нельзя удалить основной base.yml/base.yaml".clone_into(&mut self.status);
            return false;
        }

        let display_name = relative_display(&self.config_root, &path);
        if let Some(base_file) = base_file {
            let base_content = match workspace.raw_file(&base_file) {
                Ok(content) => content.to_owned(),
                Err(error) => {
                    self.status = format!(
                        "Не удалось прочитать imports перед удалением {display_name}: {}",
                        ru_message(&error.to_string())
                    );
                    return false;
                }
            };
            let updated = match yaml_imports::update_import(&base_content, &base_file, &path, false)
            {
                Ok(updated) => updated,
                Err(error) => {
                    self.status = format!(
                        "Удаление {display_name} остановлено: не удалось обновить imports: {error}"
                    );
                    return false;
                }
            };
            if updated != base_content {
                if let Err(error) = workspace.set_raw_file(&base_file, updated) {
                    self.status = format!(
                        "Удаление {display_name} остановлено: {}",
                        ru_message(&error.to_string())
                    );
                    return false;
                }
                if let Err(error) = workspace.save_all() {
                    self.status = format!(
                        "Удаление {display_name} остановлено: {}",
                        ru_message(&error.to_string())
                    );
                    return false;
                }
            }
        }

        if let Err(error) = fs::remove_file(&path) {
            self.status =
                format!("Не удалось удалить {display_name}: {error}. Файл уже исключён из imports");
            return false;
        }
        self.file_filter = None;
        self.selected = None;
        self.raw_rule.clear();
        self.reload();
        self.status = format!("Удалён {display_name}; import из base.yml также очищен");
        true
    }

    fn sync_builtin_variables(&mut self, id: &RuleId) -> Result<usize, String> {
        let definitions = dynamic_variables::builtin_definitions_in(&self.draft.replace);
        if definitions.is_empty() {
            return Ok(0);
        }

        let mut raw = self.raw_rule.clone();
        let mut added = 0_usize;
        for definition in definitions {
            let (updated, was_added) = dynamic_variables::upsert_rule_variable(&raw, &definition)?;
            raw = updated;
            added += usize::from(was_added);
        }

        if raw != self.raw_rule {
            let workspace = self
                .workspace
                .as_mut()
                .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
            workspace
                .update_rule_raw(id, &raw)
                .map_err(|error| ru_message(&error.to_string()))?;
            self.raw_rule = raw;
        }
        Ok(added)
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

    fn apply_structured(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let result = self
            .workspace
            .as_mut()
            .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())
            .and_then(|workspace| {
                workspace
                    .update_rule(&id, &self.draft)
                    .map_err(|error| ru_message(&error.to_string()))?;
                workspace
                    .rule(&id)
                    .map(|rule| rule.raw)
                    .map_err(|error| ru_message(&error.to_string()))
            });

        match result {
            Ok(raw) => {
                self.raw_rule = raw;
                match self.sync_builtin_variables(&id) {
                    Ok(0) => "Изменения ожидают сохранения (Ctrl+S)".clone_into(&mut self.status),
                    Ok(count) => {
                        self.status = format!(
                            "Изменения ожидают сохранения. Автоматически объявлено переменных: {count}"
                        );
                    }
                    Err(error) => {
                        self.status = format!("Не удалось объявить переменную: {error}");
                    }
                }
            }
            Err(error) => self.status = error,
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
                self.status = format!("YAML отклонён: {}", ru_message(&error.to_string()));
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

    fn add_dynamic_variable(&mut self, action: DynamicVariableAction) {
        let Some(id) = self.selected.clone() else {
            "Сначала выберите правило".clone_into(&mut self.status);
            return;
        };
        let previous_draft = self.draft.clone();
        self.draft.replace.push_str(&action.placeholder);

        let result = self
            .workspace
            .as_mut()
            .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())
            .and_then(|workspace| {
                workspace
                    .update_rule(&id, &self.draft)
                    .map_err(|error| ru_message(&error.to_string()))?;
                let raw = workspace
                    .rule(&id)
                    .map(|rule| rule.raw)
                    .map_err(|error| ru_message(&error.to_string()))?;
                let (updated, added) =
                    dynamic_variables::upsert_rule_variable(&raw, &action.definition)?;
                workspace
                    .update_rule_raw(&id, &updated)
                    .map_err(|error| ru_message(&error.to_string()))?;
                Ok((updated, added))
            });

        match result {
            Ok((updated, added)) => {
                self.raw_rule = updated;
                self.status = if added {
                    format!("{}. Нажмите Ctrl+S для записи YAML", action.message)
                } else {
                    format!(
                        "Переменная {} уже объявлена; шаблон добавлен в текст. Нажмите Ctrl+S",
                        action.placeholder
                    )
                };
            }
            Err(error) => {
                self.draft = previous_draft;
                if let Some(workspace) = &mut self.workspace {
                    let _ = workspace.update_rule(&id, &self.draft);
                    if let Ok(rule) = workspace.rule(&id) {
                        self.raw_rule = rule.raw;
                    }
                }
                self.status = format!("Не удалось добавить переменную: {error}");
            }
        }
    }
    fn report_rhai_action(&mut self, result: Result<String, String>) {
        self.status = match result {
            Ok(message) => message,
            Err(error) => format!("Rhai: {error}"),
        };
    }

    fn save_rhai_current(&mut self) {
        let result = self.rhai_lab.save_current();
        let saved = result.is_ok();
        self.report_rhai_action(result);
        if saved {
            self.file_monitor.refresh(&self.config_root, Instant::now());
            self.validate_project();
        }
    }

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        if self.active_tab == MainTab::Rhai {
            let (run, compile, save, reload, new_script, help) = context.input(|input| {
                let primary = input.modifiers.ctrl || input.modifiers.command;
                (
                    primary && !input.modifiers.shift && input.key_pressed(egui::Key::Enter),
                    primary && input.modifiers.shift && input.key_pressed(egui::Key::Enter),
                    primary && input.key_pressed(egui::Key::S),
                    primary && input.key_pressed(egui::Key::R),
                    primary && input.key_pressed(egui::Key::N),
                    input.key_pressed(egui::Key::F1),
                )
            });
            if save {
                self.save_rhai_current();
            }
            if reload {
                let result = self.rhai_lab.reload_current();
                self.report_rhai_action(result);
            }
            if new_script {
                let result = self.rhai_lab.start_new_script();
                self.report_rhai_action(result);
            }
            if compile {
                self.rhai_lab.compile_current();
            }
            if run {
                self.rhai_lab.run_current();
            }
            if help {
                self.show_shortcuts = true;
            }
            return;
        }

        if self.active_tab == MainTab::Settings {
            let (save, reload, help) = context.input(|input| {
                let primary = input.modifiers.ctrl || input.modifiers.command;
                (
                    primary && input.key_pressed(egui::Key::S),
                    primary && input.key_pressed(egui::Key::R),
                    input.key_pressed(egui::Key::F1),
                )
            });
            if save {
                self.save_settings();
            }
            if reload {
                self.reload_settings();
            }
            if help {
                self.show_shortcuts = true;
            }
            return;
        }

        let (save, new_rule, duplicate, delete, reload, search, apply_raw, diagnostics, help) =
            context.input(|input| {
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
        if apply_raw && self.selected.is_some() && self.mode == EditorMode::Raw {
            self.apply_raw();
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
            ui.horizontal(|ui| {
                storm_logo::show(ui);
                ui.heading(APP_TITLE);
                ui.separator();

                let (runtime_color, runtime_text) = if self.runtime.running() {
                    (
                        egui::Color32::from_rgb(40, 160, 90),
                        format!("rEspanso запущен · {} проц.", self.runtime.process_ids().len()),
                    )
                } else {
                    (
                        egui::Color32::from_rgb(190, 70, 70),
                        "rEspanso не запущен".to_owned(),
                    )
                };
                ui.colored_label(runtime_color, runtime_text).on_hover_text(format!(
                    "Проверяется каждую секунду. Последнее изменение состояния: {} сек. назад. PID: {}",
                    self.runtime.seconds_since_change(),
                    self.runtime
                        .process_ids()
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, MainTab::Rules, "Правила");
                ui.selectable_value(
                    &mut self.active_tab,
                    MainTab::Settings,
                    "Настройки rEspanso",
                );
                ui.selectable_value(&mut self.active_tab, MainTab::Rhai, "Rhai");
                ui.separator();
                if ui
                    .button("Импорт / экспорт")
                    .on_hover_text("Перенос match и scripts единым проверяемым пакетом")
                    .clicked()
                {
                    self.show_config_transfer = true;
                    self.refresh_config_packages();
                }
                ui.separator();

                match self.active_tab {
                    MainTab::Rules => {
                        if ui
                            .button("Новое правило")
                            .on_hover_text("Ctrl+N. Добавляет правило в выбранный YAML-файл")
                            .clicked()
                        {
                            self.create_rule();
                        }
                        if ui
                            .button("Сохранить всё")
                            .on_hover_text(
                                "Ctrl+S. Записывает изменения и создаёт резервные копии",
                            )
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
                        ui.checkbox(&mut self.show_diagnostics, "Диагностика").on_hover_text(
                            "Ctrl+L. Показать или скрыть устойчивые экземпляры ошибок",
                        );
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
                    }
                    MainTab::Settings => {
                        if ui
                            .button("Сохранить настройки")
                            .on_hover_text("Ctrl+S")
                            .clicked()
                        {
                            self.save_settings();
                        }
                        if ui
                            .button("Обновить настройки")
                            .on_hover_text("Ctrl+R")
                            .clicked()
                        {
                            self.reload_settings();
                        }
                        if self.settings.dirty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(210, 135, 25),
                                "Настройки не сохранены",
                            );
                        }
                    }
                    MainTab::Rhai => {
                        if ui.button("Новый скрипт").on_hover_text("Ctrl+N").clicked() {
                            let result = self.rhai_lab.start_new_script();
                            self.report_rhai_action(result);
                        }
                        if ui.button("Сохранить").on_hover_text("Ctrl+S").clicked() {
                            self.save_rhai_current();
                        }
                        if ui
                            .button("Скомпилировать")
                            .on_hover_text("Ctrl+Shift+Enter")
                            .clicked()
                        {
                            self.rhai_lab.compile_current();
                        }
                        if ui
                            .button("Запустить")
                            .on_hover_text("Ctrl+Enter")
                            .clicked()
                        {
                            self.rhai_lab.run_current();
                        }
                        if self.rhai_lab.dirty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(210, 135, 25),
                                "Скрипт не сохранён",
                            );
                        }
                    }
                }

                let right_width = ui.available_width();
                let restart_message = ui
                    .allocate_ui_with_layout(
                        egui::vec2(right_width, 30.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let restart_message = self.runtime.restart_button(ui);
                            if ui.button("Горячие клавиши").on_hover_text("F1").clicked() {
                                self.show_shortcuts = true;
                            }
                            restart_message
                        },
                    )
                    .inner;
                if let Some(message) = restart_message {
                    self.status = message;
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
        let match_root = workspace.match_root().to_path_buf();
        let files_count = files.len();
        let rules_count = rules.len();
        let base_file = yaml_imports::find_base_file(&files, &match_root);
        let (import_entries, import_error) = match base_file.as_ref() {
            Some(base_file) => match workspace.raw_file(base_file) {
                Ok(base_content) => {
                    match yaml_imports::import_entries(&files, base_file, base_content) {
                        Ok(entries) => (entries, None),
                        Err(error) => (Vec::new(), Some(error)),
                    }
                }
                Err(error) => (Vec::new(), Some(ru_message(&error.to_string()))),
            },
            None => (Vec::new(), None),
        };
        let mut pending_import_toggle = None;
        let mut request_create_yaml_file = false;
        let mut request_delete_yaml_file = None;

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

                ui.add_space(4.0);
                egui::CollapsingHeader::new(format!("YAML-файлы ({files_count})"))
                    .id_salt("yaml_files_header")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(
                                "Флажок подключает файл через imports в base.yml; название фильтрует правила",
                            )
                            .weak(),
                        );
                        if base_file.is_none() {
                            ui.colored_label(
                                ui.visuals().error_fg_color,
                                "Не найден base.yml или base.yaml — подключение файлов недоступно",
                            );
                        }
                        if let Some(error) = &import_error {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .button("Создать файл")
                                .on_hover_text("Создать новый YAML-файл в папке match")
                                .clicked()
                            {
                                request_create_yaml_file = true;
                            }
                            let selected_file = self.file_filter.clone();
                            let can_delete = selected_file
                                .as_ref()
                                .is_some_and(|file| base_file.as_ref() != Some(file));
                            if ui
                                .add_enabled(can_delete, egui::Button::new("Удалить файл"))
                                .on_hover_text("Удалить выбранный YAML-файл; base.yml удалить нельзя")
                                .clicked()
                            {
                                request_delete_yaml_file = selected_file;
                            }
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut self.yaml_file_filter)
                                .desired_width(f32::INFINITY)
                                .hint_text("Фильтр YAML-файлов…"),
                        );

                        let yaml_filter = self.yaml_file_filter.to_lowercase();
                        egui::ScrollArea::vertical()
                            .id_salt("yaml_file_list")
                            .max_height(220.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(24.0);
                                    if ui
                                        .selectable_label(self.file_filter.is_none(), "Все YAML-файлы")
                                        .clicked()
                                    {
                                        self.file_filter = None;
                                    }
                                });

                                let mut visible_files = 0_usize;
                                for file in &files {
                                    let display_name = relative_display(&config_root, file);
                                    if !yaml_filter.is_empty()
                                        && !display_name.to_lowercase().contains(&yaml_filter)
                                    {
                                        continue;
                                    }
                                    visible_files += 1;
                                    let is_base = base_file.as_ref() == Some(file);
                                    let mut enabled = if is_base {
                                        true
                                    } else {
                                        import_entries.iter().any(|entry| {
                                            entry.path.as_path() == file.as_path() && entry.enabled
                                        })
                                    };
                                    let can_toggle = !is_base
                                        && base_file.is_some()
                                        && import_error.is_none();

                                    ui.horizontal(|ui| {
                                        let checkbox = ui.add_enabled(
                                            can_toggle,
                                            egui::Checkbox::new(&mut enabled, ""),
                                        );
                                        let changed = checkbox.changed();
                                        if is_base {
                                            checkbox.on_hover_text(
                                                "Основной файл: он всегда загружается напрямую",
                                            );
                                        } else if can_toggle {
                                            checkbox.on_hover_text(
                                                "Добавить или удалить файл из imports в base.yml",
                                            );
                                        } else {
                                            checkbox.on_hover_text(
                                                "Сначала исправьте base.yml или создайте основной файл",
                                            );
                                        }
                                        if changed {
                                            pending_import_toggle = Some((file.clone(), enabled));
                                        }

                                        let selected = self.file_filter.as_ref() == Some(file);
                                        if ui.selectable_label(selected, display_name).clicked() {
                                            self.file_filter = Some(file.clone());
                                        }
                                        if is_base {
                                            ui.label(egui::RichText::new("основной").weak());
                                        }
                                    });
                                }
                                if visible_files == 0 {
                                    ui.label("YAML-файлы по фильтру не найдены");
                                }
                            });
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

        if let Some((file, enabled)) = pending_import_toggle {
            self.set_file_import_enabled(file, enabled);
        }
        if request_create_yaml_file {
            self.show_create_yaml_file = true;
        }
        if let Some(file) = request_delete_yaml_file {
            self.pending_delete_yaml_file = Some(file);
            self.confirm_delete_yaml_file = true;
        }
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
                ui.label("Ожидаемая папка правил: rEspanso\\match");
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
                ui.selectable_value(&mut self.mode, EditorMode::Structured, "Редактор");
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
        let previous_draft = self.draft.clone();
        ui.horizontal_wrapped(|ui| {
            ui.label("Условие срабатывания:");
            ui.selectable_value(&mut self.draft.kind, MatchKind::Trigger, "Обычный триггер");
            ui.selectable_value(&mut self.draft.kind, MatchKind::Regex, "Гибкий RegExp");
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
        ui.horizontal(|ui| {
            ui.label("Текст подстановки");
            if ui
                .small_button("?")
                .on_hover_text(
                    "Переменные: date, time, string, clipboard, echo, choice, form, random, rhai, script, shell",
                )
                .clicked()
            {
                self.show_dynamic_variables = true;
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.draft.replace)
                .desired_rows(10)
                .desired_width(f32::INFINITY)
                .hint_text("Текст, который rEspanso вставит вместо триггера"),
        );
        if self.draft != previous_draft {
            self.apply_structured();
        }
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
                }
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
        let diagnostics = self.diagnostics.instances();
        let active_count = self.diagnostics.active_count();

        egui::Panel::right("diagnostics")
            .resizable(true)
            .default_size(360.0)
            .show(root, |ui| {
                ui.heading("Диагностика проекта");
                ui.label(egui::RichText::new(format!("Активных проблем: {active_count}")).weak());
                if active_count == 0 {
                    ui.colored_label(
                        egui::Color32::from_rgb(40, 150, 90),
                        "Проблем не обнаружено",
                    );
                    ui.label(
                        egui::RichText::new(
                            "YAML, RegExp, импорты, внутренний config и Rhai проверены",
                        )
                        .weak(),
                    );
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for diagnostic in diagnostics {
                        ui.push_id(diagnostic.id, |ui| {
                            let prefix = match diagnostic.state {
                                DiagnosticState::PendingResolved => "ИСПРАВЛЯЕТСЯ",
                                DiagnosticState::New => "НОВАЯ",
                                DiagnosticState::Active => match diagnostic.level {
                                    DiagnosticLevel::Error => "ОШИБКА",
                                    DiagnosticLevel::Warning => "ВНИМАНИЕ",
                                    DiagnosticLevel::Info => "СВЕДЕНИЕ",
                                },
                            };
                            let text = format!("{prefix}: {}", ru_message(&diagnostic.message));
                            if diagnostic.state == DiagnosticState::PendingResolved {
                                ui.label(egui::RichText::new(text).weak().italics());
                            } else {
                                ui.strong(text);
                            }
                            if let Some(file) = &diagnostic.file {
                                ui.small(relative_display(&self.config_root, file));
                            }
                            ui.small(
                                egui::RichText::new(format!(
                                    "Наблюдений: {}",
                                    diagnostic.occurrence_count
                                ))
                                .weak(),
                            );
                            if let Some(rule) = diagnostic.rule {
                                if ui.button("Открыть правило").clicked() {
                                    self.select_rule(rule);
                                }
                            }
                            ui.separator();
                        });
                    }
                });
            });
    }

    fn dialogs(&mut self, context: &egui::Context) {
        self.config_transfer_dialog(context);
        self.config_import_confirmation(context);

        if self.show_create_yaml_file {
            let mut open = self.show_create_yaml_file;
            let mut create = false;
            let mut cancel = false;
            egui::Window::new("Создание YAML-файла")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(context, |ui| {
                    ui.label("Имя нового файла в папке match");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_yaml_file_name)
                            .desired_width(360.0)
                            .hint_text("Например: терапия.yml"),
                    );
                    ui.label(egui::RichText::new("Расширение .yml добавится автоматически").weak());
                    ui.horizontal(|ui| {
                        if ui.button("Создать").clicked() {
                            create = true;
                        }
                        if ui.button("Отмена").clicked() {
                            cancel = true;
                        }
                    });
                });
            if (create && self.create_yaml_file()) || cancel {
                open = false;
            }
            self.show_create_yaml_file = open;
        }

        if self.confirm_delete_yaml_file {
            let mut open = self.confirm_delete_yaml_file;
            let mut delete = false;
            let mut cancel = false;
            let path = self.pending_delete_yaml_file.clone();
            egui::Window::new("Удаление YAML-файла")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(context, |ui| {
                    if let Some(path) = &path {
                        ui.label(format!(
                            "Удалить {} и все правила внутри?",
                            relative_display(&self.config_root, path)
                        ));
                    }
                    ui.colored_label(
                        egui::Color32::from_rgb(190, 70, 70),
                        "Файл будет удалён сразу. Сначала сохраните нужные изменения.",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Удалить файл").clicked() {
                            delete = true;
                        }
                        if ui.button("Отмена").clicked() {
                            cancel = true;
                        }
                    });
                });
            if delete {
                if let Some(path) = path {
                    if self.delete_yaml_file(path) {
                        self.pending_delete_yaml_file = None;
                        open = false;
                    }
                }
            } else if cancel {
                self.pending_delete_yaml_file = None;
                open = false;
            }
            self.confirm_delete_yaml_file = open;
        }
        if self.show_dynamic_variables {
            let mut open = self.show_dynamic_variables;
            if let Some(action) = self.dynamic_variables.show(context, &mut open) {
                self.add_dynamic_variable(action);
            }
            self.show_dynamic_variables = open;
        }

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
                            shortcut_row(ui, "Ctrl+S", "Сохранить изменения активной вкладки");
                            shortcut_row(ui, "Ctrl+N", "Создать правило или новый Rhai-скрипт");
                            shortcut_row(
                                ui,
                                "Ctrl+Enter",
                                "Проверить исходный YAML или запустить Rhai-скрипт",
                            );
                            shortcut_row(ui, "Ctrl+Alt+M", "Найти триггер по выделенному тексту");
                            shortcut_row(ui, "Ctrl+F", "Перейти к поиску правил");
                            shortcut_row(ui, "Ctrl+D", "Дублировать правило");
                            shortcut_row(ui, "Ctrl+Shift+D", "Удалить правило");
                            shortcut_row(
                                ui,
                                "Ctrl+R",
                                "Обновить активный YAML или Rhai-файл с диска",
                            );
                            shortcut_row(ui, "Ctrl+L", "Показать или скрыть диагностику");
                            shortcut_row(ui, "Ctrl+Shift+Enter", "Скомпилировать Rhai-скрипт");
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
                if self.reload_all_after_confirm {
                    self.reload_all_from_disk();
                } else {
                    self.reload();
                }
                self.reload_all_after_confirm = false;
                open = false;
            } else if cancel {
                self.reload_all_after_confirm = false;
                open = false;
            }
            self.confirm_reload = open;
        }
    }
}

impl eframe::App for MatchStudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.runtime.update(ui.ctx());
        self.handle_dropped_config_packages(ui.ctx());
        self.check_external_file_changes(ui.ctx());
        self.handle_shortcuts(ui.ctx());
        self.top_bar(ui);

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.status.as_str());
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("Конфигурация: {}", self.config_root.display()))
                        .weak(),
                );
                if self.external_change_pending {
                    ui.separator();
                    if ui.button("Обновить внешние изменения").clicked() {
                        self.request_external_reload();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.hyperlink_to(
                        "imaganate.dark@gmail.com",
                        "mailto:imaganate.dark@gmail.com",
                    )
                    .on_hover_text("Автор форка: Куцин Иван Юрьевич");
                });
            });
        });

        match self.active_tab {
            MainTab::Rules => {
                self.rules_panel(ui);
                self.diagnostics_panel(ui);
                self.central_editor(ui);
            }
            MainTab::Settings => {
                let config_root = self.config_root.clone();
                self.settings.ui(ui, &config_root, &mut self.status);
            }
            MainTab::Rhai => {
                self.rhai_lab.ui(ui, &mut self.status);
            }
        }
        self.dialogs(ui.ctx());
    }
}

fn shortcut_row(ui: &mut egui::Ui, shortcut: &str, description: &str) {
    ui.monospace(shortcut);
    ui.label(description);
    ui.end_row();
}

fn normalize_yaml_file_name(value: &str) -> Result<String, String> {
    let mut name = value.trim().to_owned();
    if name.is_empty() {
        return Err("Укажите имя YAML-файла".to_owned());
    }
    if name.starts_with('.')
        || name.chars().any(|character| {
            character.is_control()
                || ['<', '>', ':', '"', '/', '\\', '|', '?', '*'].contains(&character)
        })
    {
        return Err("Имя содержит недопустимые для Windows символы".to_owned());
    }
    if Path::new(&name).extension().is_none() {
        name.push_str(".yml");
    }
    let path = Path::new(&name);
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("yml") && !extension.eq_ignore_ascii_case("yaml") {
        return Err("Допустимы только расширения .yml и .yaml".to_owned());
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    if stem.is_empty() {
        return Err("Имя YAML-файла не может быть пустым".to_owned());
    }
    let reserved = stem.to_ascii_uppercase();
    if matches!(
        reserved.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        return Err("Это имя зарезервировано Windows".to_owned());
    }
    Ok(name)
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
