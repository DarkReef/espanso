#!/usr/bin/env python3
from pathlib import Path


def replace(path, old, new, count=1):
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    if text.count(old) < count:
        raise SystemExit(f"{path}: patch anchor not found: {old[:100]!r}")
    p.write_text(text.replace(old, new, count), encoding="utf-8")
    print(f"patched {path}")

# Search: rEspanso uses Ctrl+Alt+Space as its default search shortcut.
replace(
    "espanso-config/src/config/resolve.rs",
    'None => Some("ALT+SPACE".to_string()),',
    'None => Some("CTRL+ALT+SPACE".to_string()),',
)

# X11 input payload is length-delimited, not a C string.  Do not panic/log
# malformed input just because XLookupString left an interior NUL byte.
replace(
    "espanso-detect/src/x11/mod.rs",
    '    ffi::{c_void, CStr},',
    '    ffi::c_void,',
)
replace(
    "espanso-detect/src/x11/mod.rs",
    '''            let value = if raw.buffer_len > 0 {
                let raw_string_result =
                    CStr::from_bytes_with_nul(&raw.buffer[..((raw.buffer_len + 1) as usize)]);
                match raw_string_result {
                    Ok(c_string) => {
                        let string_result = c_string.to_str();
                        match string_result {
                            Ok(value) => Some(value.to_string()),
                            Err(err) => {
                                warn!("char conversion error: {}", err);
                                None
                            }
                        }
                    }
                    Err(err) => {
                        warn!("Received malformed char: {}", err);
                        None
                    }
                }
            } else {
                None
            };''',
    '''            let value = if raw.buffer_len > 0 {
                let len = (raw.buffer_len as usize).min(raw.buffer.len());
                let bytes = &raw.buffer[..len];
                let bytes = match bytes.iter().position(|byte| *byte == 0) {
                    Some(nul) => &bytes[..nul],
                    None => bytes,
                };
                match std::str::from_utf8(bytes) {
                    Ok(value) if !value.is_empty() => Some(value.to_string()),
                    Ok(_) => None,
                    Err(err) => {
                        warn!("char conversion error: {}", err);
                        None
                    }
                }
            } else {
                None
            };''',
)

# A late X11 event during worker shutdown/restart is harmless.  Never panic if
# the engine receiver has already gone away.
replace(
    "espanso/src/cli/worker/engine/funnel/mod.rs",
    '''                            sender
                                .send((event, source_id))
                                .expect("unable to send to the source channel");''',
    '''                            if sender.send((event, source_id)).is_err() {
                                return;
                            }''',
)

# On X11 the PRIMARY selection is the selected text itself. Read it first,
# avoiding a synthetic Ctrl+C while Ctrl+Alt+M/Alt+L modifiers are settling.
# The existing clipboard-copy path remains the fallback.
replace(
    "espanso/src/cli/worker/engine/dispatch/executor/clipboard_injector.rs",
    '''impl SelectedTextProvider for ClipboardInjectorAdapter<'_> {
    fn get_selected_text(&self) -> Option<String> {
        let params = self.params_provider.get();''',
    '''impl SelectedTextProvider for ClipboardInjectorAdapter<'_> {
    fn get_selected_text(&self) -> Option<String> {
        #[cfg(all(target_os = "linux", not(feature = "wayland")))]
        {
            if let Ok(output) = std::process::Command::new("xclip")
                .args(["-o", "-selection", "primary"])
                .output()
            {
                if output.status.success() {
                    if let Ok(text) = String::from_utf8(output.stdout) {
                        if !text.trim().is_empty() {
                            debug!("selected text obtained from X11 PRIMARY selection");
                            return Some(text);
                        }
                    }
                }
            }
            debug!("X11 PRIMARY selection unavailable; falling back to Ctrl+C capture");
        }

        let params = self.params_provider.get();''',
)

# wxGTK form: tall forms must scroll.  Never ask GTK for a client size larger
# than the configured viewport (the old SetMaxSize + GetBestSize combination
# produced negative content heights and aborted the form extension).
p = Path("espanso-modulo/src/sys/form/form.cpp")
t = p.read_text(encoding="utf-8")
for old, new in [
    ('#include <memory>\n', '#include <algorithm>\n#include <memory>\n'),
    ('#include <vector>\n', '#include <vector>\n#include <wx/scrolwin.h>\n'),
    ('    wxPanel *panel;', '    wxScrolledWindow *panel;'),
    ('    void UpdateHelpText();', '    void UpdateHelpText();\n    void FitFormToContent();'),
    ('    frame->SetMaxSize(maxFormSize);\n', ''),
    ('    panel = new wxPanel(this, wxID_ANY);',
     '    panel = new wxScrolledWindow(this, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxVSCROLL);\n    panel->SetScrollRate(0, 10);'),
    ('    this->SetClientSize(panel->GetBestSize());\n    this->CentreOnScreen();',
     '    FitFormToContent();\n    this->CentreOnScreen();'),
]:
    if old not in t:
        raise SystemExit(f"form.cpp: patch anchor not found: {old!r}")
    t = t.replace(old, new, 1)
old_help = '''void FormFrame::UpdateHelpText() {
    if (hasFocusedMultilineControl) {
        helpText->SetLabel(wxString::FromUTF8(
            "Ctrl+Enter — вставить, Esc — отменить"));
    } else {
        helpText->SetLabel(wxString::FromUTF8(
            "Enter — вставить, Esc — отменить"));
    }
    this->SetClientSize(panel->GetBestSize());
}'''
new_help = '''void FormFrame::FitFormToContent() {
    const wxSize best = panel->GetBestSize();
    const int maxWidth = std::max(320, formMetadata->maxWindowWidth);
    const int maxHeight = std::max(180, formMetadata->maxWindowHeight);
    const int width = std::min(std::max(best.GetWidth(), 320), maxWidth);
    const int height = std::min(std::max(best.GetHeight(), 180), maxHeight);

    panel->SetVirtualSize(best);
    panel->FitInside();
    this->SetClientSize(wxSize(width, height));
    this->Layout();
}

void FormFrame::UpdateHelpText() {
    if (hasFocusedMultilineControl) {
        helpText->SetLabel(wxString::FromUTF8(
            "Ctrl+Enter — вставить, Esc — отменить"));
    } else {
        helpText->SetLabel(wxString::FromUTF8(
            "Enter — вставить, Esc — отменить"));
    }
    panel->FitInside();
    this->Layout();
}'''
if old_help not in t:
    raise SystemExit("form.cpp: UpdateHelpText anchor not found")
t = t.replace(old_help, new_help, 1)
p.write_text(t, encoding="utf-8")
print("patched espanso-modulo/src/sys/form/form.cpp")

# Raw YAML editor: the source body gets its own vertical scroll while the
# validation/apply button stays visible.
replace(
    "espanso-editor/src/app_legacy.rs",
    '''        ui.add(
            egui::TextEdit::multiline(&mut self.raw_rule)
                .code_editor()
                .desired_rows(24)
                .desired_width(f32::INFINITY),
        );
        if ui
            .button("Проверить и применить YAML")''',
    '''        let yaml_height = (ui.available_height() - 56.0).max(180.0);
        egui::ScrollArea::vertical()
            .id_salt("raw_yaml_source_scroll")
            .max_height(yaml_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.raw_rule)
                        .code_editor()
                        .desired_rows(40)
                        .desired_width(f32::INFINITY),
                );
            });
        if ui
            .button("Проверить и применить YAML")''',
)

# Help must describe the backend actually shipped now.
p = Path("espanso-editor/src/app_shell.rs")
t = p.read_text(encoding="utf-8")
old = "Astra/X11: глобальные сочетания отслеживаются через XInput2 Raw Events без эксклюзивного XGrabKey."
new = "Astra/X11: клавиатура отслеживается через XQueryKeymap; занятые глобальные сочетания работают через polling-fallback."
if old in t:
    p.write_text(t.replace(old, new, 1), encoding="utf-8")
    print("patched espanso-editor/src/app_shell.rs")
else:
    print("app_shell.rs: backend help text already current or absent")

checks = {
    "espanso-config/src/config/resolve.rs": 'CTRL+ALT+SPACE',
    "espanso-detect/src/x11/mod.rs": 'std::str::from_utf8(bytes)',
    "espanso/src/cli/worker/engine/funnel/mod.rs": 'sender.send((event, source_id)).is_err()',
    "espanso/src/cli/worker/engine/dispatch/executor/clipboard_injector.rs": 'X11 PRIMARY selection',
    "espanso-modulo/src/sys/form/form.cpp": 'wxScrolledWindow',
    "espanso-editor/src/app_legacy.rs": 'raw_yaml_source_scroll',
}
for file, marker in checks.items():
    if marker not in Path(file).read_text(encoding="utf-8"):
        raise SystemExit(f"verification failed: {marker!r} missing from {file}")
print("all final Astra fix markers verified")
