from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / "espanso-editor/src/app.rs"
text = path.read_text(encoding="utf-8")

old_reload = '''    fn request_external_reload(&mut self) {
        if self.has_unsaved_changes() {'''
new_reload = '''    fn request_external_reload(&mut self) {
        if self.has_unsaved_changes() || self.rhai_lab.dirty() {'''
if old_reload not in text:
    raise RuntimeError("request_external_reload pattern was not found")
text = text.replace(old_reload, new_reload, 1)

old_save = '''                        if ui.button("Сохранить").on_hover_text("Ctrl+S").clicked() {
                            let result = self.rhai_lab.save_current();
                            self.report_rhai_action(result);
                        }'''
new_save = '''                        if ui.button("Сохранить").on_hover_text("Ctrl+S").clicked() {
                            self.save_rhai_current();
                        }'''
if old_save not in text:
    raise RuntimeError("Rhai toolbar save pattern was not found")
text = text.replace(old_save, new_save, 1)

path.write_text(text, encoding="utf-8", newline="\n")
