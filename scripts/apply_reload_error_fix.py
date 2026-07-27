from pathlib import Path

path = Path("espanso-editor/src/app.rs")
text = path.read_text(encoding="utf-8")
old = '''    fn reload_all_from_disk(&mut self) {
        self.reload();
        self.settings = SettingsEditor::load(&self.config_root);
        self.external_change_pending = false;
        self.status =
            "Обнаружены изменения YAML-файлов; Studio обновила правила и настройки".to_owned();
    }
'''
new = '''    fn reload_all_from_disk(&mut self) {
        self.reload();
        if self.load_error.is_some() {
            self.external_change_pending = true;
            return;
        }
        self.settings = SettingsEditor::load(&self.config_root);
        self.file_monitor.refresh(&self.config_root);
        self.external_change_pending = false;
        self.status =
            "Обнаружены изменения YAML-файлов; Studio обновила правила и настройки".to_owned();
    }
'''
if text.count(old) != 1:
    raise RuntimeError(f"reload_all_from_disk block count: {text.count(old)}")
text = text.replace(old, new, 1)
old = '''        match self.settings.reload_selected() {
            Ok(()) => "Настройки перечитаны с диска".clone_into(&mut self.status),
            Err(error) => self.status = error,
        }
'''
new = '''        match self.settings.reload_selected() {
            Ok(()) => {
                self.file_monitor.refresh(&self.config_root);
                "Настройки перечитаны с диска".clone_into(&mut self.status);
            }
            Err(error) => self.status = error,
        }
'''
if text.count(old) != 1:
    raise RuntimeError(f"reload settings block count: {text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
