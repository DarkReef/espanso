from __future__ import annotations

import re
from pathlib import Path

APP = Path("espanso-editor/src/app.rs")
text = APP.read_text(encoding="utf-8")


def replace_once(source: str, old: str, new: str, label: str) -> str:
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    return source.replace(old, new, 1)


def replace_regex(source: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, source, count=1, flags=re.S)
    if count != 1:
        raise RuntimeError(f"{label}: expected one regex match, found {count}")
    return updated


text = replace_once(
    text,
    "use std::{\n    fs,\n    io::Write as _,\n    path::{Path, PathBuf},\n    time::{Duration, Instant},\n};\n",
    "use std::{\n    fs,\n    io::Write as _,\n    path::{Path, PathBuf},\n    time::{Duration, Instant},\n};\nuse walkdir::WalkDir;\n",
    "walkdir import",
)

text = replace_once(
    text,
    'const FILE_STABILITY_RECHECK: Duration = Duration::from_millis(850);\n',
    'const FILE_STABILITY_RECHECK: Duration = Duration::from_millis(850);\n'
    'const DISABLED_YAML_SUFFIX: &str = ".disabled";\n'
    'const ESPANSO_REGEXP_DOCS_URL: &str = "https://espanso.org/docs/matches/regex-triggers/";\n',
    "constants",
)

text = replace_once(
    text,
    "enum EditorMode {\n    Structured,\n    Raw,\n}\n",
    "enum EditorMode {\n    Structured,\n    Raw,\n}\n\n"
    "#[derive(Debug, Clone)]\n"
    "struct YamlFileEntry {\n"
    "    path: PathBuf,\n"
    "    enabled: bool,\n"
    "}\n",
    "yaml entry type",
)

set_yaml_file_enabled = r'''    fn set_yaml_file_enabled(&mut self, file: PathBuf, enabled: bool) {
        if self.has_transfer_unsaved_changes() || self.settings.dirty() {
            "Сначала сохраните изменения, затем включайте или выключайте YAML-файл"
                .clone_into(&mut self.status);
            return;
        }

        let display_name = yaml_display_name(&self.config_root, &file, !is_disabled_yaml_path(&file));
        let target = if enabled {
            match enabled_yaml_path(&file) {
                Some(path) => path,
                None => {
                    self.status = format!("Файл {display_name} уже включён");
                    return;
                }
            }
        } else {
            if is_disabled_yaml_path(&file) {
                self.status = format!("Файл {display_name} уже выключен");
                return;
            }
            disabled_yaml_path(&file)
        };

        let result = (|| -> Result<(), String> {
            if !file.is_file() {
                return Err(format!("Файл не найден: {}", file.display()));
            }
            if target.exists() {
                return Err(format!(
                    "Целевой файл уже существует: {}",
                    target.display()
                ));
            }

            let workspace = self
                .workspace
                .as_mut()
                .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
            let files = workspace.files();
            let base_file = yaml_imports::find_base_file(&files, workspace.match_root());
            let logical_enabled_path = if enabled { &target } else { &file };
            if base_file.as_ref() == Some(logical_enabled_path) {
                return Err("Нельзя выключить основной base.yml/base.yaml".to_owned());
            }

            // Imports не управляют загрузкой файлов из match: Espanso агрегирует все
            // .yml/.yaml. При выключении очищаем устаревшую запись и меняем расширение.
            if !enabled {
                if let Some(base_file) = base_file {
                    let base_content = workspace
                        .raw_file(&base_file)
                        .map_err(|error| ru_message(&error.to_string()))?
                        .to_owned();
                    let updated =
                        yaml_imports::update_import(&base_content, &base_file, &file, false)?;
                    if updated != base_content {
                        workspace
                            .set_raw_file(&base_file, updated)
                            .map_err(|error| ru_message(&error.to_string()))?;
                        workspace
                            .save_all()
                            .map_err(|error| ru_message(&error.to_string()))?;
                    }
                }
            }

            fs::rename(&file, &target).map_err(|error| {
                format!(
                    "Не удалось переименовать {} в {}: {error}",
                    file.display(),
                    target.display()
                )
            })?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.selected = None;
                self.raw_rule.clear();
                self.reload();
                self.file_filter = Some(target.clone());
                self.status = if enabled {
                    format!(
                        "Файл {display_name} включён. Перезагрузите конфигурацию или перезапустите rEspanso"
                    )
                } else {
                    format!(
                        "Файл {display_name} выключен расширением .disabled и больше не загружается. Перезагрузите конфигурацию или перезапустите rEspanso"
                    )
                };
            }
            Err(error) => {
                self.status = format!("Не удалось изменить состояние {display_name}: {error}");
            }
        }
    }

'''
text = replace_regex(
    text,
    r"    fn set_file_import_enabled\(&mut self, file: PathBuf, enabled: bool\) \{.*?\n    fn create_yaml_file",
    set_yaml_file_enabled + "    fn create_yaml_file",
    "YAML toggle method",
)

delete_yaml_file = r'''    fn delete_yaml_file(&mut self, path: PathBuf) -> bool {
        if self.has_transfer_unsaved_changes() || self.settings.dirty() {
            "Сначала сохраните текущие изменения, затем удалите YAML-файл"
                .clone_into(&mut self.status);
            return false;
        }

        let disabled = is_disabled_yaml_path(&path);
        let logical_path = if disabled {
            match enabled_yaml_path(&path) {
                Some(path) => path,
                None => {
                    "Не удалось определить исходное имя выключенного YAML-файла"
                        .clone_into(&mut self.status);
                    return false;
                }
            }
        } else {
            path.clone()
        };

        let Some(workspace) = &mut self.workspace else {
            return false;
        };
        let files = workspace.files();
        if disabled {
            if !path.is_file() {
                "Выбранный выключенный YAML-файл больше не существует"
                    .clone_into(&mut self.status);
                return false;
            }
        } else if !files.iter().any(|file| file == &path) {
            "Выбранный YAML-файл больше не входит в рабочую область"
                .clone_into(&mut self.status);
            return false;
        }

        let base_file = yaml_imports::find_base_file(&files, workspace.match_root());
        if base_file.as_ref() == Some(&logical_path) {
            "Нельзя удалить основной base.yml/base.yaml".clone_into(&mut self.status);
            return false;
        }

        let display_name = yaml_display_name(&self.config_root, &path, !disabled);
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
            let updated =
                match yaml_imports::update_import(&base_content, &base_file, &logical_path, false) {
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
            self.status = format!("Не удалось удалить {display_name}: {error}");
            return false;
        }
        self.file_filter = None;
        self.selected = None;
        self.raw_rule.clear();
        self.reload();
        self.status = format!("Удалён {display_name}; связанные imports также очищены");
        true
    }

'''
text = replace_regex(
    text,
    r"    fn delete_yaml_file\(&mut self, path: PathBuf\) -> bool \{.*?\n    fn sync_builtin_variables",
    delete_yaml_file + "    fn sync_builtin_variables",
    "delete YAML method",
)

rules_panel = r'''    fn rules_panel(&mut self, root: &mut egui::Ui) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let files = workspace.files();
        let rules = workspace.rules();
        let config_root = workspace.config_root().to_path_buf();
        let match_root = workspace.match_root().to_path_buf();
        let yaml_entries = yaml_file_entries(&files, &match_root);
        let files_count = yaml_entries.len();
        let disabled_count = yaml_entries.iter().filter(|entry| !entry.enabled).count();
        let rules_count = rules.len();
        let base_file = yaml_imports::find_base_file(&files, &match_root);
        let mut pending_yaml_toggle = None;
        let mut request_create_yaml_file = false;
        let mut request_delete_yaml_file = None;

        egui::Panel::left("rules")
            .resizable(true)
            .default_size(350.0)
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Правила");
                    let files_summary = if disabled_count == 0 {
                        format!("{rules_count} правил · {files_count} файлов")
                    } else {
                        format!(
                            "{rules_count} правил · {files_count} файлов · выключено {disabled_count}"
                        )
                    };
                    ui.label(egui::RichText::new(files_summary).weak());
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
                                "Флажок реально включает файл. Выключенный файл получает расширение .disabled, поэтому rEspanso больше не загружает его правила.",
                            )
                            .weak(),
                        );
                        ui.label(
                            egui::RichText::new(
                                "После переключения перезагрузите конфигурацию или перезапустите rEspanso.",
                            )
                            .weak(),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .button("Создать файл")
                                .on_hover_text("Создать новый YAML-файл в папке match")
                                .clicked()
                            {
                                request_create_yaml_file = true;
                            }
                            let selected_file = self.file_filter.clone();
                            let can_delete = selected_file.as_ref().is_some_and(|file| {
                                let logical = if is_disabled_yaml_path(file) {
                                    enabled_yaml_path(file).unwrap_or_else(|| file.clone())
                                } else {
                                    file.clone()
                                };
                                base_file.as_ref() != Some(&logical)
                            });
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
                            .max_height(240.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(24.0);
                                    if ui
                                        .selectable_label(
                                            self.file_filter.is_none(),
                                            "Все включённые YAML-файлы",
                                        )
                                        .clicked()
                                    {
                                        self.file_filter = None;
                                    }
                                });

                                let mut visible_files = 0_usize;
                                for entry in &yaml_entries {
                                    let display_name = yaml_display_name(
                                        &config_root,
                                        &entry.path,
                                        entry.enabled,
                                    );
                                    if !yaml_filter.is_empty()
                                        && !display_name.to_lowercase().contains(&yaml_filter)
                                    {
                                        continue;
                                    }
                                    visible_files += 1;
                                    let logical_path = if entry.enabled {
                                        entry.path.clone()
                                    } else {
                                        enabled_yaml_path(&entry.path)
                                            .unwrap_or_else(|| entry.path.clone())
                                    };
                                    let is_base = base_file.as_ref() == Some(&logical_path);
                                    let mut enabled = entry.enabled;

                                    ui.horizontal(|ui| {
                                        let checkbox = ui.add_enabled(
                                            !is_base,
                                            egui::Checkbox::new(&mut enabled, ""),
                                        );
                                        let changed = checkbox.changed();
                                        if is_base {
                                            checkbox.on_hover_text(
                                                "Основной файл: он всегда включён",
                                            );
                                        } else if entry.enabled {
                                            checkbox.on_hover_text(
                                                "Выключить: переименовать файл в .yml.disabled/.yaml.disabled",
                                            );
                                        } else {
                                            checkbox.on_hover_text(
                                                "Включить: вернуть расширение .yml/.yaml",
                                            );
                                        }
                                        if changed {
                                            pending_yaml_toggle =
                                                Some((entry.path.clone(), enabled));
                                        }

                                        let selected =
                                            self.file_filter.as_ref() == Some(&entry.path);
                                        let label = if entry.enabled {
                                            display_name
                                        } else {
                                            format!("{display_name}  ·  выключен")
                                        };
                                        if ui.selectable_label(selected, label).clicked() {
                                            self.file_filter = Some(entry.path.clone());
                                            if !entry.enabled {
                                                self.selected = None;
                                                self.raw_rule.clear();
                                            }
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
                            if self
                                .file_filter
                                .as_ref()
                                .is_some_and(|file| is_disabled_yaml_path(file))
                            {
                                ui.label("Файл выключен: его правила не загружаются");
                            } else {
                                ui.label("По заданному фильтру ничего не найдено");
                            }
                        });
                    }
                });
            });

        if let Some((file, enabled)) = pending_yaml_toggle {
            self.set_yaml_file_enabled(file, enabled);
        }
        if request_create_yaml_file {
            self.show_create_yaml_file = true;
        }
        if let Some(file) = request_delete_yaml_file {
            self.pending_delete_yaml_file = Some(file);
            self.confirm_delete_yaml_file = true;
        }
    }

'''
text = replace_regex(
    text,
    r"    fn rules_panel\(&mut self, root: &mut egui::Ui\) \{.*?\n    fn central_editor",
    rules_panel + "    fn central_editor",
    "rules panel",
)

text = replace_once(
    text,
    '''            if self.selected.is_none() {
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
''',
    '''            if self.selected.is_none() {
                let disabled_file_selected = self
                    .file_filter
                    .as_ref()
                    .is_some_and(|file| is_disabled_yaml_path(file));
                ui.vertical_centered(|ui| {
                    ui.add_space(35.0);
                    if disabled_file_selected {
                        ui.heading("YAML-файл выключен");
                        ui.label(
                            "Его расширение оканчивается на .disabled, поэтому rEspanso не загружает правила из этого файла.",
                        );
                        ui.label("Поставьте флажок слева, чтобы снова включить файл.");
                    } else {
                        ui.heading("Выберите правило слева или создайте новое");
                        ui.label(
                            "Редактор объединяет правила из всех включённых .yml и .yaml файлов папки match.",
                        );
                        ui.label("Ctrl+N — новое правило, Ctrl+F — поиск, F1 — справка.");
                    }
                });
                ui.add_space(20.0);
                self.playground_ui(ui);
                return;
            }
''',
    "empty editor state",
)

regex_editor = r'''    fn regex_editor(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Понятный конструктор RegExp");
                ui.hyperlink_to(egui::RichText::new("?").strong(), ESPANSO_REGEXP_DOCS_URL)
                    .on_hover_text("Открыть официальную справку Espanso по RegExp-триггерам");
            });
            ui.label(
                "Соберите выражение из обычного текста и одной изменяемой части. Конструктор сам экранирует постоянный текст.",
            );
            ui.label(
                egui::RichText::new(
                    "RegExp используйте только для изменяемых триггеров. Одинаковые обычные триггеры допустимы: rEspanso покажет окно выбора.",
                )
                .weak(),
            );
            ui.add_space(6.0);

            ui.strong("1. Постоянный текст до переменной");
            ui.add(
                egui::TextEdit::singleline(&mut self.builder.prefix)
                    .desired_width(f32::INFINITY)
                    .hint_text("Например: :пациент_"),
            );

            ui.add_space(4.0);
            ui.strong("2. Изменяемая часть");
            ui.label(egui::RichText::new("Выберите готовый тип или задайте свой шаблон").weak());
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Число")
                    .on_hover_text(r"Одна или несколько цифр: \d+")
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
                if ui
                    .button("Дата ДД.ММ.ГГГГ")
                    .on_hover_text(r"Например 29.07.2026")
                    .clicked()
                {
                    self.use_regex_preset("date", r"\d{2}\.\d{2}\.\d{4}");
                }
                if ui
                    .button("Без пробелов")
                    .on_hover_text(r"Любой непробельный фрагмент: \S+")
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
            ui.columns(2, |columns| {
                columns[0].label("Имя переменной");
                columns[0].add(
                    egui::TextEdit::singleline(&mut self.builder.capture_name)
                        .desired_width(f32::INFINITY)
                        .hint_text("Например: patient_id"),
                );
                columns[0].label(
                    egui::RichText::new(
                        "Значение станет доступно в подстановке как {{patient_id}}",
                    )
                    .weak(),
                );

                columns[1].label("Шаблон переменной");
                columns[1].add(
                    egui::TextEdit::singleline(&mut self.builder.capture_pattern)
                        .desired_width(f32::INFINITY)
                        .hint_text(r"Например: \d+"),
                );
                columns[1].label(
                    egui::RichText::new(r"Частые варианты: \d+ — цифры, \S+ — без пробелов, .+? — любой текст")
                        .weak(),
                );
            });

            ui.add_space(4.0);
            ui.strong("3. Постоянный текст после переменной");
            ui.add(
                egui::TextEdit::singleline(&mut self.builder.suffix)
                    .desired_width(f32::INFINITY)
                    .hint_text("Можно оставить пустым"),
            );
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.builder.anchor_start,
                    "Начинать совпадение строго с этого места (^)",
                );
                ui.checkbox(
                    &mut self.builder.anchor_end,
                    "Завершать совпадение здесь ($)",
                );
            });

            match MatchWorkspace::build_regex(&self.builder) {
                Ok(preview) => {
                    ui.add_space(4.0);
                    ui.label("Предварительный результат");
                    ui.monospace(preview);
                }
                Err(error) => {
                    ui.colored_label(ui.visuals().error_fg_color, ru_message(&error));
                }
            }
            if ui
                .button("Собрать и использовать выражение")
                .on_hover_text("Записать результат конструктора в RegExp выбранного правила")
                .clicked()
            {
                self.apply_regex_builder();
            }
            if let Some(error) = &self.builder_error {
                ui.colored_label(ui.visuals().error_fg_color, error.as_str());
            }
        });

        ui.add_space(8.0);
        egui::CollapsingHeader::new("Готовое выражение и ручная правка")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("RegExp-триггер");
                    ui.hyperlink_to("?", ESPANSO_REGEXP_DOCS_URL)
                        .on_hover_text("Синтаксис RegExp-триггеров Espanso и именованных групп");
                });
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut self.draft.regex)
                            .code_editor()
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .hint_text(r":id_(?P<id>\d+)$"),
                    )
                    .changed()
                {
                    self.refresh_regex_examples();
                }
                ui.label(
                    egui::RichText::new(
                        r"Подсказка: (?P<name>...) создаёт переменную {{name}}. Espanso использует синтаксис Rust Regex.",
                    )
                    .weak(),
                );
                match MatchWorkspace::validate_regex(&self.draft.regex) {
                    Ok(()) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(40, 150, 90),
                            "✓ Выражение корректно",
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
                ui.label(
                    egui::RichText::new(
                        "Введите реальные варианты триггера. Проверка не запускает подстановки, команды или скрипты.",
                    )
                    .weak(),
                );
                ui.columns(2, |columns| {
                    columns[0].label("По одному примеру в строке");
                    if columns[0]
                        .add(
                            egui::TextEdit::multiline(&mut self.regex_examples_text)
                                .desired_rows(7)
                                .desired_width(f32::INFINITY),
                        )
                        .changed()
                    {
                        self.refresh_regex_examples();
                    }
                    columns[1].label("Что найдёт RegExp");
                    egui::ScrollArea::vertical()
                        .max_height(205.0)
                        .show(&mut columns[1], |ui| {
                            for result in &self.regex_results {
                                if result.matched {
                                    ui.strong(format!("✓ {} — совпало", result.input));
                                } else {
                                    ui.label(format!("— {} — не совпало", result.input));
                                }
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
                                    ui.small(format!("{{{{{name}}}}} = {value}"));
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

'''
text = replace_regex(
    text,
    r"    fn regex_editor\(&mut self, ui: &mut egui::Ui\) \{.*?\n    fn raw_editor",
    regex_editor + "    fn raw_editor",
    "regex editor",
)

text = replace_once(
    text,
    '''        let diagnostics = self.diagnostics.instances();
        let active_count = self.diagnostics.active_count();
''',
    '''        let diagnostics = self
            .diagnostics
            .instances()
            .into_iter()
            .filter(|diagnostic| !is_expected_simple_trigger_duplicate(&diagnostic.message))
            .collect::<Vec<_>>();
        let active_count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.state != DiagnosticState::PendingResolved)
            .count();
''',
    "diagnostic filtering",
)

text = replace_once(
    text,
    '''                ui.heading("Диагностика проекта");
                ui.label(egui::RichText::new(format!("Активных проблем: {active_count}")).weak());
''',
    '''                ui.heading("Диагностика проекта");
                ui.label(egui::RichText::new(format!("Активных проблем: {active_count}")).weak());
                ui.label(
                    egui::RichText::new(
                        "Повторы обычных триггеров допустимы и открывают окно выбора. Повторы RegExp продолжают отображаться как проблема.",
                    )
                    .weak(),
                );
''',
    "diagnostic explanation",
)

helpers = r'''
fn yaml_file_entries(active_files: &[PathBuf], match_root: &Path) -> Vec<YamlFileEntry> {
    let mut entries = active_files
        .iter()
        .cloned()
        .map(|path| YamlFileEntry {
            path,
            enabled: true,
        })
        .collect::<Vec<_>>();

    if match_root.is_dir() {
        for path in WalkDir::new(match_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(walkdir::DirEntry::into_path)
            .filter(|path| is_disabled_yaml_path(path))
        {
            let logical = enabled_yaml_path(&path).unwrap_or_else(|| path.clone());
            if entries.iter().any(|entry| entry.path == logical) {
                continue;
            }
            entries.push(YamlFileEntry {
                path,
                enabled: false,
            });
        }
    }

    entries.sort_by_key(|entry| {
        yaml_visible_path(&entry.path, entry.enabled)
            .to_string_lossy()
            .to_lowercase()
    });
    entries
}

fn disabled_yaml_path(path: &Path) -> PathBuf {
    let Some(file_name) = path.file_name() else {
        return path.with_extension("disabled");
    };
    let mut disabled_name = file_name.to_os_string();
    disabled_name.push(DISABLED_YAML_SUFFIX);
    path.with_file_name(disabled_name)
}

fn enabled_yaml_path(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_string_lossy();
    let lower = file_name.to_ascii_lowercase();
    if !lower.ends_with(DISABLED_YAML_SUFFIX) {
        return None;
    }
    let enabled_name = &file_name[..file_name.len() - DISABLED_YAML_SUFFIX.len()];
    let enabled_lower = enabled_name.to_ascii_lowercase();
    if !enabled_lower.ends_with(".yml") && !enabled_lower.ends_with(".yaml") {
        return None;
    }
    Some(path.with_file_name(enabled_name))
}

fn is_disabled_yaml_path(path: &Path) -> bool {
    enabled_yaml_path(path).is_some()
}

fn yaml_visible_path(path: &Path, enabled: bool) -> PathBuf {
    if enabled {
        path.to_path_buf()
    } else {
        enabled_yaml_path(path).unwrap_or_else(|| path.to_path_buf())
    }
}

fn yaml_display_name(root: &Path, path: &Path, enabled: bool) -> String {
    relative_display(root, &yaml_visible_path(path, enabled))
}

fn is_expected_simple_trigger_duplicate(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .starts_with("duplicate match cause: trigger:")
}

'''
text = replace_once(
    text,
    "fn normalize_yaml_file_name(input: &str) -> Result<String, String> {\n",
    helpers + "fn normalize_yaml_file_name(input: &str) -> Result<String, String> {\n",
    "helper insertion",
)

if "set_file_import_enabled" in text:
    raise RuntimeError("old YAML toggle method still present")
if "pending_import_toggle" in text:
    raise RuntimeError("old pending import toggle still present")
if "Флажок подключает файл через imports" in text:
    raise RuntimeError("old imports UI text still present")

APP.write_text(text, encoding="utf-8")
print("Updated", APP)
