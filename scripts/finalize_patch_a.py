from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")

def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8", newline="\n")

def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if old not in text:
        raise RuntimeError(f"Pattern was not found in {path}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))

def replace_all(path: str, old: str, new: str, minimum: int = 1) -> None:
    text = read(path)
    count = text.count(old)
    if count < minimum:
        raise RuntimeError(
            f"Expected at least {minimum} replacements in {path}, found {count}: {old[:120]!r}"
        )
    write(path, text.replace(old, new))

def replace_regex(path: str, pattern: str, replacement: str, count: int = 1) -> None:
    text = read(path)
    updated, replacements = re.subn(pattern, replacement, text, count=count, flags=re.S)
    if replacements != count:
        raise RuntimeError(
            f"Expected {count} regex replacements in {path}, found {replacements}: {pattern[:120]!r}"
        )
    write(path, updated)

def patch_config_transfer() -> None:
    path = "espanso-editor/src/config_transfer.rs"
    replace_once(path, "const PACKAGE_VERSION: u32 = 1;", "const PACKAGE_VERSION: u32 = 2;")
    replace_all(
        path,
        'for directory in ["config", "match", "scripts"] {',
        'for directory in ["match", "scripts"] {',
        minimum=3,
    )
    replace_once(
        path,
        'if !matches!(root.to_str(), Some("config" | "match" | "scripts")) {',
        'if !matches!(root.to_str(), Some("match" | "scripts")) {',
    )
    replace_once(
        path,
        '''    match root.to_str() {
        Some("config" | "match") => {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        }
        Some("scripts") => extension.eq_ignore_ascii_case("rhai"),
        _ => false,
    }''',
        '''    match root.to_str() {
        Some("match") => {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        }
        Some("scripts") => extension.eq_ignore_ascii_case("rhai"),
        _ => false,
    }''',
    )
    text = read(path)
    text = text.replace('        fs::create_dir_all(root.join("config")).unwrap();\n', '')
    text = text.replace(
        '''        fs::write(
            root.join("config/default.yml"),
            "show_notifications: true\\n",
        )
        .unwrap();
''',
        '',
    )
    text = text.replace("assert_eq!(report.file_count, 4);", "assert_eq!(report.file_count, 3);")
    text = text.replace("assert_eq!(summary.file_count, 4);", "assert_eq!(summary.file_count, 3);")
    write(path, text)

def patch_app() -> None:
    path = "espanso-editor/src/app.rs"
    replace_once(
        path,
        '''use crate::{
    config_transfer::{self, PackageSummary},
    file_monitor::FileMonitor,
    rhai_lab::RhaiLab,
    runtime::RuntimeMonitor,
    settings::SettingsEditor,
    workspace::{
        DiagnosticLevel, MatchKind, MatchWorkspace, PlaygroundResult, RegexBuilderSpec,
        RegexExampleResult, RuleDraft, RuleId,
    },
    yaml_imports,
};''',
        '''use crate::{
    config_transfer::{self, PackageSummary},
    diagnostics::{collect_project_diagnostics, DiagnosticManager, DiagnosticState},
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
};''',
    )
    replace_once(
        path,
        'const FILE_CHECK_INTERVAL: Duration = Duration::from_secs(3);',
        'const FILE_CHECK_INTERVAL: Duration = Duration::from_secs(3);\nconst FILE_STABILITY_RECHECK: Duration = Duration::from_millis(850);',
    )
    replace_once(
        path,
        '''    file_monitor: FileMonitor,
    next_file_check: Instant,
    external_change_pending: bool,''',
        '''    file_monitor: FileMonitor,
    diagnostics: DiagnosticManager,
    next_file_check: Instant,
    external_change_pending: bool,''',
    )
    replace_once(
        path,
        '''        let settings = SettingsEditor::load(&config_root);
        let rhai_lab = RhaiLab::new(&config_root);
        let file_monitor = FileMonitor::new(&config_root);
        let config_packages = config_transfer::list_packages(&config_root).unwrap_or_default();
        Self {''',
        '''        let settings = SettingsEditor::load(&config_root);
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
        Self {''',
    )
    replace_once(
        path,
        '''            file_monitor,
            next_file_check: Instant::now() + FILE_CHECK_INTERVAL,''',
        '''            file_monitor,
            diagnostics,
            next_file_check: now + FILE_CHECK_INTERVAL,''',
    )
    replace_regex(
        path,
        r'''    fn reload_all_from_disk\(&mut self\) \{.*?\n    \}\n\n    fn has_transfer_unsaved_changes''',
        '''    fn reload_all_from_disk(&mut self) {
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
        let diagnostics =
            collect_project_diagnostics(&self.config_root, self.workspace.as_ref());
        self.diagnostics.reconcile(
            diagnostics,
            &self.config_root,
            self.workspace.as_ref(),
        );
    }

    fn has_transfer_unsaved_changes''',
    )
    replace_once(
        path,
        '''    fn has_transfer_unsaved_changes(&self) -> bool {
        self.has_unsaved_changes() || self.rhai_lab.dirty()
    }''',
        '''    fn has_transfer_unsaved_changes(&self) -> bool {
        self.workspace
            .as_ref()
            .is_some_and(|workspace| !workspace.dirty_files().is_empty())
            || self.rhai_lab.dirty()
    }''',
    )
    replace_all(
        path,
        '"Экспорт остановлен: сначала сохраните изменения правил, настроек и Rhai-скрипта"',
        '"Экспорт остановлен: сначала сохраните изменения правил и Rhai-скрипта"',
    )
    replace_all(
        path,
        '"Единый пакет .respanso-config включает config/**/*.yml|yaml, match/**/*.yml|yaml и scripts/**/*.rhai."',
        '"Пакет .respanso-config включает только match/**/*.yml|yaml и scripts/**/*.rhai. Внутренний config не переносится."',
    )
    replace_all(
        path,
        '"Текущие папки config, match и scripts будут заменены."',
        '"Текущие папки match и scripts будут заменены. Внутренний config останется без изменений."',
    )
    replace_all(
        path,
        '"Перенос config, match и scripts единым проверяемым пакетом"',
        '"Перенос match и scripts единым проверяемым пакетом"',
    )
    replace_all(
        path,
        'FileMonitor::new(&self.config_root)',
        'FileMonitor::new(&self.config_root, Instant::now())',
    )
    replace_all(
        path,
        'self.file_monitor.refresh(&self.config_root);',
        'self.file_monitor.refresh(&self.config_root, Instant::now());\n                self.validate_project();',
        minimum=1,
    )
    replace_regex(
        path,
        r'''    fn check_external_file_changes\(&mut self, context: &egui::Context\) \{.*?\n    \}\n\n    fn request_reload''',
        '''    fn check_external_file_changes(&mut self, context: &egui::Context) {
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
                self.status = format!(
                    "Файлы изменяются: ожидается устойчивый хеш ({changed_files})"
                );
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
                        "Проверен устойчивый снимок проекта: изменено файлов {changed_files}"
                    );
                }
            }
        }
        context.request_repaint_after(
            self.next_file_check
                .saturating_duration_since(Instant::now()),
        );
    }

    fn request_reload''',
    )
    replace_once(
        path,
        '''    fn report_rhai_action(&mut self, result: Result<String, String>) {
        self.status = match result {
            Ok(message) => message,
            Err(error) => format!("Rhai: {error}"),
        };
    }''',
        '''    fn report_rhai_action(&mut self, result: Result<String, String>) {
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
            self.file_monitor
                .refresh(&self.config_root, Instant::now());
            self.validate_project();
        }
    }''',
    )
    replace_all(
        path,
        '''                let result = self.rhai_lab.save_current();
                self.report_rhai_action(result);''',
        '''                self.save_rhai_current();''',
        minimum=1,
    )
    replace_once(
        path,
        '''                ui.heading(APP_TITLE);
                ui.separator();''',
        '''                storm_logo::show(ui);
                ui.heading(APP_TITLE);
                ui.separator();''',
    )
    replace_all(
        path,
        'ui.checkbox(&mut self.show_diagnostics, "Проверка").on_hover_text(',
        'ui.checkbox(&mut self.show_diagnostics, "Диагностика").on_hover_text(',
    )
    replace_all(
        path,
        '"Ctrl+L. Ошибки YAML, RegExp, дубликаты и проблемы импортов"',
        '"Ctrl+L. Показать или скрыть устойчивые экземпляры ошибок"',
    )
    replace_regex(
        path,
        r'''    fn diagnostics_panel\(&mut self, root: &mut egui::Ui\) \{.*?\n    \}\n\n    fn dialogs''',
        '''    fn diagnostics_panel(&mut self, root: &mut egui::Ui) {
        if !self.show_diagnostics {
            return;
        }
        let diagnostics = self.diagnostics.instances();
        let generation = self.diagnostics.generation();
        let active_count = self.diagnostics.active_count();

        egui::Panel::right("diagnostics")
            .resizable(true)
            .default_size(360.0)
            .show(root, |ui| {
                ui.heading("Диагностика проекта");
                ui.label(
                    egui::RichText::new(format!(
                        "Снимок #{generation} · активных проблем: {active_count}"
                    ))
                    .weak(),
                );
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
                            let text =
                                format!("{prefix}: {}", ru_message(&diagnostic.message));
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
                                    "Наблюдений: {} · впервые: #{} · последнее: #{}",
                                    diagnostic.occurrence_count,
                                    diagnostic.first_seen_generation,
                                    diagnostic.last_seen_generation
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

    fn dialogs''',
    )
    replace_all(
        path,
        '"Ctrl+L", "Показать или скрыть проверку"',
        '"Ctrl+L", "Показать или скрыть диагностику"',
    )
    replace_once(
        path,
        '''                self.refresh_config_packages();
                self.select_config_package(report.package.clone());

                let warning''',
        '''                self.refresh_config_packages();
                self.select_config_package(report.package.clone());
                self.validate_project();

                let warning''',
    )

if __name__ == "__main__":
    patch_config_transfer()
    patch_app()
