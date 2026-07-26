use eframe::egui;
use serde_norway::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlagGroup {
    Common,
    Windows,
    OtherPlatforms,
}

#[derive(Debug, Clone, Copy)]
struct BoolFlag {
    key: &'static str,
    title: &'static str,
    when_true: &'static str,
    when_false: &'static str,
    default: bool,
    group: FlagGroup,
}

const BOOL_FLAGS: &[BoolFlag] = &[
    BoolFlag {
        key: "enable",
        title: "Включить rEspanso для этой конфигурации",
        when_true: "подстановки активны",
        when_false: "эта конфигурация отключает подстановки",
        default: true,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "auto_restart",
        title: "Автоматически перечитывать конфигурацию",
        when_true: "worker перезапускается после изменения файлов",
        when_false: "изменения применятся только после ручного перезапуска",
        default: true,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "preserve_clipboard",
        title: "Сохранять содержимое буфера обмена",
        when_true: "после вставки возвращается прежний буфер",
        when_false: "в буфере остаётся текст подстановки",
        default: true,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "apply_patch",
        title: "Применять встроенные патчи совместимости",
        when_true: "rEspanso использует встроенные исправления для приложений",
        when_false: "патчи совместимости отключены",
        default: true,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "undo_backspace",
        title: "Отменять подстановку клавишей Backspace",
        when_true: "Backspace сразу после вставки восстанавливает триггер",
        when_false: "Backspace работает как обычное удаление",
        default: true,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "show_notifications",
        title: "Показывать уведомления",
        when_true: "системные уведомления разрешены",
        when_false: "все уведомления скрыты",
        default: true,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "show_icon",
        title: "Показывать значок в системном трее",
        when_true: "значок rEspanso виден в трее",
        when_false: "значок скрыт",
        default: true,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "stats_enabled",
        title: "Вести локальную статистику подстановок",
        when_true: "счётчики использования записываются локально",
        when_false: "статистика не записывается",
        default: false,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "use_standard_includes",
        title: "Подключать стандартные YAML-файлы match",
        when_true: "обычные файлы из match загружаются автоматически",
        when_false: "используются только явно заданные includes",
        default: true,
        group: FlagGroup::Common,
    },
    BoolFlag {
        key: "emulate_alt_codes",
        title: "Эмулировать Windows Alt-коды",
        when_true: "rEspanso восстанавливает ввод символов через Alt+цифры",
        when_false: "эмуляция Alt-кодов отключена",
        default: true,
        group: FlagGroup::Windows,
    },
    BoolFlag {
        key: "win32_exclude_orphan_events",
        title: "Фильтровать Windows-события без HID-источника",
        when_true: "программно созданные orphan-события игнорируются",
        when_false: "такие события принимаются, что полезно для экранных клавиатур",
        default: true,
        group: FlagGroup::Windows,
    },
    BoolFlag {
        key: "secure_input_notification",
        title: "Уведомлять о Secure Input на macOS",
        when_true: "показывается предупреждение Secure Input",
        when_false: "предупреждение скрыто",
        default: true,
        group: FlagGroup::OtherPlatforms,
    },
    BoolFlag {
        key: "disable_x11_fast_inject",
        title: "Отключить быструю вставку X11",
        when_true: "используется более медленный совместимый механизм XTest",
        when_false: "используется быстрый механизм XSendEvent",
        default: false,
        group: FlagGroup::OtherPlatforms,
    },
    BoolFlag {
        key: "x11_use_xclip_backend",
        title: "Использовать xclip для буфера X11",
        when_true: "буфер обмена работает через внешнюю команду xclip",
        when_false: "используется встроенный backend",
        default: false,
        group: FlagGroup::OtherPlatforms,
    },
    BoolFlag {
        key: "x11_use_xdotool_backend",
        title: "Использовать xdotool для вставки X11",
        when_true: "включён альтернативный backend xdotool",
        when_false: "используется стандартный backend вставки",
        default: false,
        group: FlagGroup::OtherPlatforms,
    },
];

#[derive(Default)]
pub struct SettingsEditor {
    files: Vec<PathBuf>,
    selected: Option<PathBuf>,
    text: String,
    original: String,
    error: Option<String>,
    status: String,
}

impl SettingsEditor {
    pub fn load(config_root: &Path) -> Self {
        let mut editor = Self::default();
        editor.reload_files(config_root);
        editor
    }

    pub fn dirty(&self) -> bool {
        self.selected.is_some() && self.text != self.original
    }

    pub fn save(&mut self) -> Result<(), String> {
        let path = self
            .selected
            .clone()
            .ok_or_else(|| "Файл настроек не выбран".to_owned())?;
        validate_yaml(&self.text)?;

        let current = fs::read_to_string(&path)
            .map_err(|error| format!("Не удалось перечитать {}: {error}", path.display()))?;
        if current != self.original {
            return Err(
                "Файл был изменён другой программой. Обновите его с диска перед сохранением"
                    .to_owned(),
            );
        }

        let backup = PathBuf::from(format!("{}.respanso.bak", path.display()));
        fs::write(&backup, &current)
            .map_err(|error| format!("Не удалось создать {}: {error}", backup.display()))?;
        fs::write(&path, &self.text)
            .map_err(|error| format!("Не удалось сохранить {}: {error}", path.display()))?;

        self.original.clone_from(&self.text);
        self.error = None;
        self.status = format!("Сохранено: {}", path.display());
        Ok(())
    }

    pub fn reload_selected(&mut self) -> Result<(), String> {
        let path = self
            .selected
            .clone()
            .ok_or_else(|| "Файл настроек не выбран".to_owned())?;
        self.select_file(path)
    }

    pub fn reload_files(&mut self, config_root: &Path) {
        let settings_root = config_root.join("config");
        self.files = collect_yaml_files(&settings_root);
        if self.files.is_empty() {
            if let Err(error) = create_initial_settings_file(&settings_root) {
                self.error = Some(error);
                return;
            }
            self.files = collect_yaml_files(&settings_root);
        }

        let next = self
            .selected
            .as_ref()
            .filter(|selected| self.files.contains(selected))
            .cloned()
            .or_else(|| self.files.first().cloned());
        if let Some(path) = next {
            if let Err(error) = self.select_file(path) {
                self.error = Some(error);
            }
        }
    }

    pub fn ui(&mut self, root: &mut egui::Ui, config_root: &Path, app_status: &mut String) {
        let settings_root = config_root.join("config");
        let selected = self.selected.clone();

        egui::Panel::left("settings_files")
            .resizable(true)
            .default_size(310.0)
            .show(root, |ui| {
                ui.heading("Файлы настроек");
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {} файлов",
                        settings_root.display(),
                        self.files.len()
                    ))
                    .weak(),
                );
                ui.separator();

                for path in self.files.clone() {
                    let label = relative_display(&settings_root, &path);
                    let is_selected = selected.as_ref() == Some(&path);
                    if ui.selectable_label(is_selected, label).clicked() {
                        if self.dirty() {
                            self.error = Some(
                                "Сначала сохраните изменения или обновите текущий файл с диска"
                                    .to_owned(),
                            );
                        } else if let Err(error) = self.select_file(path) {
                            self.error = Some(error);
                        }
                    }
                }

                ui.separator();
                if ui.button("Перечитать список файлов").clicked() {
                    if self.dirty() {
                        self.error = Some(
                            "Есть несохранённые изменения. Сначала сохраните или отмените их"
                                .to_owned(),
                        );
                    } else {
                        self.reload_files(config_root);
                    }
                }
            });

        egui::CentralPanel::default().show(root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("Настройки rEspanso");
                    if self.dirty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(210, 135, 25),
                            "Есть несохранённые изменения",
                        );
                    }
                });
                ui.label(
                    "Редактируются YAML-файлы из portable\\config\\config. Правила match здесь не показываются.",
                );
                if let Some(path) = &self.selected {
                    ui.monospace(path.display().to_string());
                }
                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.selected.is_some(),
                            egui::Button::new("Сохранить настройки"),
                        )
                        .on_hover_text("Ctrl+S. Перед записью создаётся .respanso.bak")
                        .clicked()
                    {
                        match self.save() {
                            Ok(()) => {
                                "Настройки сохранены. rEspanso может потребовать перезагрузку конфигурации"
                                    .clone_into(app_status);
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                    if ui
                        .add_enabled(
                            self.selected.is_some(),
                            egui::Button::new("Обновить с диска"),
                        )
                        .on_hover_text("Ctrl+R. Несохранённый текст будет потерян")
                        .clicked()
                    {
                        if let Err(error) = self.reload_selected() {
                            self.error = Some(error);
                        }
                    }
                });

                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                } else if !self.status.is_empty() {
                    ui.label(egui::RichText::new(self.status.as_str()).weak());
                }

                if self.selected.is_none() {
                    ui.add_space(30.0);
                    ui.heading("Файлы настроек не найдены");
                    ui.label(format!("Ожидаемая папка: {}", settings_root.display()));
                    return;
                }

                ui.add_space(8.0);
                ui.heading("Основные флаги");
                ui.label(
                    egui::RichText::new(
                        "Флажки отражают текущий YAML. Если ключ отсутствует, показано штатное значение по умолчанию.",
                    )
                    .weak(),
                );
                draw_flag_group(
                    ui,
                    &mut self.text,
                    &mut self.error,
                    FlagGroup::Common,
                );

                ui.add_space(8.0);
                ui.heading("Windows");
                draw_flag_group(
                    ui,
                    &mut self.text,
                    &mut self.error,
                    FlagGroup::Windows,
                );

                egui::CollapsingHeader::new("Флаги других платформ")
                    .default_open(false)
                    .show(ui, |ui| {
                        draw_flag_group(
                            ui,
                            &mut self.text,
                            &mut self.error,
                            FlagGroup::OtherPlatforms,
                        );
                    });

                ui.add_space(12.0);
                ui.separator();
                ui.heading("Исходный YAML");
                ui.label(
                    egui::RichText::new(
                        "Можно редактировать файл вручную. Флажки выше обновятся автоматически.",
                    )
                    .weak(),
                );
                let response = ui.add(
                    egui::TextEdit::multiline(&mut self.text)
                        .code_editor()
                        .desired_rows(28)
                        .desired_width(f32::INFINITY),
                );
                if response.changed() {
                    self.error = validate_yaml(&self.text).err();
                }

                match validate_yaml(&self.text) {
                    Ok(()) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(40, 150, 90),
                            "YAML корректен",
                        );
                    }
                    Err(error) => {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                }
                ui.label(
                    egui::RichText::new(
                        "После сохранения используйте кнопку «(Пере)запустить rEspanso» в правом верхнем углу.",
                    )
                    .weak(),
                );
            });
        });
    }

    fn select_file(&mut self, path: PathBuf) -> Result<(), String> {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("Не удалось открыть {}: {error}", path.display()))?;
        self.selected = Some(path);
        self.text.clone_from(&content);
        self.original = content;
        self.error = validate_yaml(&self.text).err();
        self.status.clear();
        Ok(())
    }
}

fn draw_flag_group(
    ui: &mut egui::Ui,
    text: &mut String,
    error: &mut Option<String>,
    group: FlagGroup,
) {
    for flag in BOOL_FLAGS.iter().filter(|flag| flag.group == group) {
        ui.group(|ui| {
            let explicit = read_top_level_bool(text, flag.key);
            let mut value = explicit.unwrap_or_else(|| flag_default(flag));
            let changed = ui
                .checkbox(&mut value, format!("{} — {}", flag.key, flag.title))
                .changed();
            if changed {
                *text = set_top_level_bool(text, flag.key, value);
                *error = validate_yaml(text).err();
            }

            let source = if explicit.is_some() {
                "указано в YAML"
            } else {
                "значение по умолчанию; ключ отсутствует"
            };
            ui.label(
                egui::RichText::new(format!(
                    "Сейчас: {value} ({source}). true — {}; false — {}.",
                    flag.when_true, flag.when_false
                ))
                .small()
                .weak(),
            );
        });
    }
}

fn flag_default(flag: &BoolFlag) -> bool {
    if flag.key == "emulate_alt_codes" {
        cfg!(target_os = "windows")
    } else {
        flag.default
    }
}

fn read_top_level_bool(text: &str, key: &str) -> Option<bool> {
    let value = serde_norway::from_str::<Value>(text).ok()?;
    let Value::Mapping(mapping) = value else {
        return None;
    };
    mapping.iter().find_map(|(candidate, value)| {
        let Value::String(candidate) = candidate else {
            return None;
        };
        if candidate != key {
            return None;
        }
        match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    })
}

fn set_top_level_bool(text: &str, key: &str, value: bool) -> String {
    let replacement = format!("{key}: {value}");
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return format!("{replacement}\n");
    }

    let had_trailing_newline = text.ends_with('\n');
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if is_top_level_key(line, key) {
            let comment = line.find('#').map(|index| line[index..].trim_start());
            lines.push(match comment {
                Some(comment) => format!("{replacement}  {comment}"),
                None => replacement.clone(),
            });
            replaced = true;
        } else {
            lines.push(line.to_owned());
        }
    }

    if !replaced {
        lines.push(replacement);
    }

    let mut result = lines.join("\n");
    if had_trailing_newline || !replaced {
        result.push('\n');
    }
    result
}

fn is_top_level_key(line: &str, key: &str) -> bool {
    if line
        .chars()
        .next()
        .is_some_and(|character| character.is_whitespace())
        || line.trim_start().starts_with('#')
    {
        return false;
    }
    let Some((candidate, _)) = line.split_once(':') else {
        return false;
    };
    candidate.trim() == key
}

fn collect_yaml_files(root: &Path) -> Vec<PathBuf> {
    let mut files = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
                })
        })
        .filter(|path| !path.to_string_lossy().ends_with(".respanso.bak"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn create_initial_settings_file(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("Не удалось создать {}: {error}", root.display()))?;
    let path = root.join("default.yml");
    if !path.exists() {
        fs::write(
            &path,
            "# Настройки rEspanso\n# Используйте флажки в Match Studio или редактируйте YAML ниже.\n{}\n",
        )
        .map_err(|error| format!("Не удалось создать {}: {error}", path.display()))?;
    }
    Ok(())
}

fn validate_yaml(text: &str) -> Result<(), String> {
    serde_norway::from_str::<Value>(text)
        .map(|_| ())
        .map_err(|error| format!("Ошибка YAML: {error}"))
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[test]
    fn settings_scan_does_not_include_match_directory() {
        let root = TempDir::new("settings").unwrap();
        let config = root.path().join("config");
        let matches = root.path().join("match");
        fs::create_dir_all(&config).unwrap();
        fs::create_dir_all(&matches).unwrap();
        fs::write(config.join("default.yml"), "{}\n").unwrap();
        fs::write(matches.join("base.yml"), "matches: []\n").unwrap();

        let files = collect_yaml_files(&config);
        assert_eq!(files, vec![config.join("default.yml")]);
    }

    #[test]
    fn rejects_invalid_yaml() {
        assert!(validate_yaml("key: [").is_err());
    }

    #[test]
    fn reads_explicit_boolean_flag() {
        assert_eq!(
            read_top_level_bool("show_icon: false\n", "show_icon"),
            Some(false)
        );
        assert_eq!(read_top_level_bool("{}\n", "show_icon"), None);
    }

    #[test]
    fn inserts_boolean_flag_into_empty_mapping() {
        assert_eq!(
            set_top_level_bool("{}\n", "show_icon", false),
            "show_icon: false\n"
        );
    }

    #[test]
    fn updates_boolean_without_reformatting_other_yaml() {
        let source = "# comment\nshow_icon: true  # tray\nbackend: auto\n";
        let updated = set_top_level_bool(source, "show_icon", false);
        assert_eq!(
            updated,
            "# comment\nshow_icon: false  # tray\nbackend: auto\n"
        );
    }
}
