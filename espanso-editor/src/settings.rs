use eframe::egui;
use serde_norway::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

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
                    .add_enabled(self.selected.is_some(), egui::Button::new("Сохранить настройки"))
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
                    .add_enabled(self.selected.is_some(), egui::Button::new("Обновить с диска"))
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

            let response = ui.add(
                egui::TextEdit::multiline(&mut self.text)
                    .code_editor()
                    .desired_rows(32)
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
                    "После сохранения используйте перезагрузку конфигурации rEspanso либо перезапустите его.",
                )
                .weak(),
            );
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

fn collect_yaml_files(root: &Path) -> Vec<PathBuf> {
    let mut files = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
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
            "# Настройки rEspanso\n# Добавляйте параметры в формате YAML ниже.\n{}\n",
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
}
