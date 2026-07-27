from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


app = Path("espanso-editor/src/app.rs")
replace_once(
    app,
    "    confirm_reload: bool,\n    file_monitor: FileMonitor,",
    "    confirm_reload: bool,\n    reload_all_after_confirm: bool,\n    file_monitor: FileMonitor,",
    "app reload scope field",
)
replace_once(
    app,
    "            confirm_reload: false,\n            file_monitor,",
    "            confirm_reload: false,\n            reload_all_after_confirm: false,\n            file_monitor,",
    "app reload scope initialization",
)
replace_once(
    app,
    "                self.file_monitor.refresh(&self.config_root);\n                self.external_change_pending = false;\n                \"Правила перечитаны с диска\".clone_into(&mut self.status);",
    "                self.file_monitor.refresh(&self.config_root);\n                \"Правила перечитаны с диска\".clone_into(&mut self.status);",
    "do not acknowledge settings changes during rules-only reload",
)
replace_once(
    app,
    '''    fn check_external_file_changes(&mut self, context: &egui::Context) {
        let now = Instant::now();
        if now < self.next_file_check {
            context.request_repaint_after(self.next_file_check.saturating_duration_since(now));
            return;
        }
        self.next_file_check = now + FILE_CHECK_INTERVAL;
        context.request_repaint_after(FILE_CHECK_INTERVAL);

        if !self.file_monitor.changed(&self.config_root) {
            return;
        }

        let unsaved_rules = self
            .workspace
            .as_ref()
            .is_some_and(|workspace| !workspace.dirty_files().is_empty());
        if unsaved_rules || self.settings.dirty() {
            self.external_change_pending = true;
            self.status = "Файлы конфигурации изменились снаружи. Сохраните или отмените локальные изменения, затем обновите с диска".to_owned();
            return;
        }

        self.reload();
        self.settings = SettingsEditor::load(&self.config_root);
        self.status =
            "Обнаружены изменения YAML-файлов; Studio автоматически обновила данные".to_owned();
    }
''',
    '''    fn has_unsaved_changes(&self) -> bool {
        self.workspace
            .as_ref()
            .is_some_and(|workspace| !workspace.dirty_files().is_empty())
            || self.settings.dirty()
    }

    fn reload_all_from_disk(&mut self) {
        self.reload();
        self.settings = SettingsEditor::load(&self.config_root);
        self.external_change_pending = false;
        self.status =
            "Обнаружены изменения YAML-файлов; Studio обновила правила и настройки".to_owned();
    }

    fn request_external_reload(&mut self) {
        if self.has_unsaved_changes() {
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
        self.next_file_check = now + FILE_CHECK_INTERVAL;
        context.request_repaint_after(FILE_CHECK_INTERVAL);

        if !self.file_monitor.changed(&self.config_root) {
            return;
        }

        if self.has_unsaved_changes() {
            self.external_change_pending = true;
            self.status = "Файлы конфигурации изменились снаружи. Сохраните или отмените локальные изменения, затем обновите с диска".to_owned();
            return;
        }

        self.reload_all_from_disk();
    }
''',
    "external file reload flow",
)
replace_once(
    app,
    "        if has_unsaved {\n            self.confirm_reload = true;\n        } else {\n            self.reload();\n        }",
    "        if has_unsaved {\n            self.reload_all_after_confirm = false;\n            self.confirm_reload = true;\n        } else {\n            self.reload();\n        }",
    "rules reload scope",
)
replace_once(
    app,
    '''            if reload {
                self.reload();
                open = false;
            } else if cancel {
                open = false;
            }
            self.confirm_reload = open;
''',
    '''            if reload {
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
''',
    "confirmed reload scope",
)
replace_once(
    app,
    '''                    if ui.button("Обновить внешние изменения").clicked() {
                        self.request_reload();
                    }
''',
    '''                    if ui.button("Обновить внешние изменения").clicked() {
                        self.request_external_reload();
                    }
''',
    "external reload button",
)

main = Path("espanso-editor/src/main.rs")
replace_once(
    main,
    '''fn portable_config_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;

    let respanso_config = directory.join(".espanso");
    if respanso_config.is_dir() {
        return Some(respanso_config);
    }

    let is_named_standalone = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("rEspanso Match Studio"));
    let is_respanso_bundle = directory.join("rEspanso.exe").is_file()
        || directory.join("rEspanso-core.exe").is_file()
        || directory.join("config").is_dir()
        || directory.join("match").is_dir();

    (is_respanso_bundle || is_named_standalone).then(|| directory.to_path_buf())
}

fn normalize_config_root(mut config_root: PathBuf) -> PathBuf {
    while config_root
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("config"))
        && config_root
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("config"))
    {
        config_root.pop();
    }
    config_root
}
''',
    '''fn portable_config_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;

    let is_named_standalone = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("rEspanso Match Studio"));
    let is_respanso_bundle = directory.join("rEspanso.exe").is_file()
        || directory.join("rEspanso-core.exe").is_file()
        || directory.join("config").is_dir()
        || directory.join("match").is_dir();
    if is_respanso_bundle || is_named_standalone {
        return Some(directory.to_path_buf());
    }

    let respanso_config = directory.join(".espanso");
    respanso_config.is_dir().then_some(respanso_config)
}

fn normalize_config_root(mut config_root: PathBuf) -> PathBuf {
    loop {
        let is_config_directory = config_root
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("config"));
        if !is_config_directory {
            break;
        }

        let Some(parent) = config_root.parent() else {
            break;
        };
        let duplicate_config = parent
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("config"));
        let parent_is_bundle = parent.join("rEspanso.exe").is_file()
            || parent.join("rEspanso-core.exe").is_file()
            || parent.join("match").is_dir();
        if !duplicate_config && !parent_is_bundle {
            break;
        }
        config_root = parent.to_path_buf();
    }
    config_root
}
''',
    "portable root precedence and normalization",
)
replace_once(
    main,
    "    fn collapses_accidental_config_config_root() {",
    "    fn collapses_duplicate_config_segment() {",
    "duplicate config test name",
)
replace_once(
    main,
    '''    #[test]
    fn portable_root_supports_cyrillic_bundle_paths() {
''',
    '''    #[test]
    fn normalizes_config_subdirectory_of_bundle() {
        let directory = tempdir::TempDir::new("respanso-bundle-root").unwrap();
        fs::write(directory.path().join("rEspanso.exe"), []).unwrap();
        let config = directory.path().join("config");
        fs::create_dir_all(&config).unwrap();
        assert_eq!(normalize_config_root(config), directory.path());
    }

    #[test]
    fn portable_root_supports_cyrillic_bundle_paths() {
''',
    "bundle config normalization test",
)
