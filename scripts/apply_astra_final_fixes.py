#!/usr/bin/env python3
from pathlib import Path


def replace(path, old, new, count=1):
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    actual = text.count(old)
    if actual < count:
        raise SystemExit(f"{path}: expected at least {count} occurrence(s), found {actual}: {old[:80]!r}")
    text = text.replace(old, new, count)
    p.write_text(text, encoding="utf-8")
    print(f"patched {path}")

# 1. Search shortcut: user-facing default is Ctrl+Alt+Space on rEspanso.
replace(
    "espanso-config/src/config/resolve.rs",
    'None => Some("ALT+SPACE".to_string()),',
    'None => Some("CTRL+ALT+SPACE".to_string()),',
)

# 2. X11 text decoding: buffer_len already describes payload bytes.  Do not
# construct a CStr (embedded NULs are possible for raw X11 translations).
replace(
    "espanso-detect/src/x11/mod.rs",
    '    ffi::{c_void, CStr},',
    '    ffi::c_void,',
)
old_decode = '''            let value = if raw.buffer_len > 0 {
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
            };'''
new_decode = '''            let value = if raw.buffer_len > 0 {
                // The native backend reports the number of payload bytes; the
                // buffer is not a C string.  Reading it as CStr used to reject
                // perfectly recoverable X11 events containing an embedded NUL.
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
            };'''
replace("espanso-detect/src/x11/mod.rs", old_decode, new_decode)

# 3. Closing/restarting the engine must not panic the detector callback when
# its receiver has already gone away.
replace(
    "espanso/src/cli/worker/engine/funnel/mod.rs",
    '''                            sender
                                .send((event, source_id))
                                .expect("unable to send to the source channel");''',
    '''                            if sender.send((event, source_id)).is_err() {
                                // The engine is already shutting down/restarting.
                                // Dropping this late event is expected and avoids
                                // killing the detector thread with SendError.
                                return;
                            }''',
)

# 4. Selected text on real X11: first read the PRIMARY selection directly.
# This avoids synthesizing Ctrl+C while the global hotkey modifiers are still
# settling.  Keep the existing clipboard-copy route as a compatibility fallback.
replace(
    "espanso/src/cli/worker/engine/dispatch/executor/clipboard_injector.rs",
    'use std::{\n    convert::TryInto,\n    path::PathBuf,',
    'use std::{\n    convert::TryInto,\n    path::PathBuf,\n    process::Command,',
)
replace(
    "espanso/src/cli/worker/engine/dispatch/executor/clipboard_injector.rs",
    '''impl SelectedTextProvider for ClipboardInjectorAdapter<'_> {
    fn get_selected_text(&self) -> Option<String> {
        let params = self.params_provider.get();''',
    '''impl SelectedTextProvider for ClipboardInjectorAdapter<'_> {
    fn get_selected_text(&self) -> Option<String> {
        #[cfg(all(target_os = "linux", not(feature = "wayland")))]
        {
            // X11 PRIMARY is the selection itself.  Reading it directly is
            // considerably more reliable than injecting Ctrl+C from inside a
            // Ctrl+Alt+M/Alt+L hotkey handler, especially on KDE/Astra.
            if let Ok(output) = Command::new("xclip")
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

# Avoid an unused import on non-X11 targets.
replace(
    "espanso/src/cli/worker/engine/dispatch/executor/clipboard_injector.rs",
    'use log::{debug, error};',
    'use log::{debug, error};\n\n#[cfg(not(all(target_os = "linux", not(feature = "wayland"))))]\n#[allow(unused_imports)]\nuse std::process::Command as _PlatformCommandPlaceholder;',
)
# The placeholder above is intentionally unnecessary if Command remains in the
# grouped import; remove it and make the grouped import conditional-safe by
# referring to std::process::Command fully qualified instead.
p = Path("espanso/src/cli/worker/engine/dispatch/executor/clipboard_injector.rs")
t = p.read_text(encoding="utf-8")
t = t.replace('    process::Command,\n', '')
t = t.replace('\n#[cfg(not(all(target_os = "linux", not(feature = "wayland"))))]\n#[allow(unused_imports)]\nuse std::process::Command as _PlatformCommandPlaceholder;', '')
t = t.replace('if let Ok(output) = Command::new("xclip")', 'if let Ok(output) = std::process::Command::new("xclip")')
p.write_text(t, encoding="utf-8")

# 5. wxGTK forms: make the content vertically scrollable and never request an
# impossible client size larger than the configured form viewport.
p = Path("espanso-modulo/src/sys/form/form.cpp")
t = p.read_text(encoding="utf-8")
t = t.replace('#include <memory>\n', '#include <algorithm>\n#include <memory>\n', 1)
t = t.replace('    wxPanel *panel;', '    wxScrolledWindow *panel;', 1)
t = t.replace('    void UpdateHelpText();', '    void UpdateHelpText();\n    void FitFormToContent();', 1)
t = t.replace('    frame->SetMaxSize(maxFormSize);\n', '', 1)
t = t.replace(
    '    panel = new wxPanel(this, wxID_ANY);',
    '    panel = new wxScrolledWindow(this, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxVSCROLL);\n    panel->SetScrollRate(0, 10);',
    1,
)
t = t.replace(
    '    this->SetClientSize(panel->GetBestSize());\n    this->CentreOnScreen();',
    '    FitFormToContent();\n    this->CentreOnScreen();',
    1,
)
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
    const int width = std::clamp(best.GetWidth(), 320, maxWidth);
    const int height = std::clamp(best.GetHeight(), 180, maxHeight);

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

# 6. Match Studio raw YAML editor: scroll only the source body, keeping action
# buttons visible.
p = Path("espanso-editor/src/app_legacy.rs")
t = p.read_text(encoding="utf-8")
old_raw = '''        ui.add(
            egui::TextEdit::multiline(&mut self.raw_draft)
                .desired_rows(22)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY),
        );
        ui.add_space(8.0);'''
new_raw = '''        let yaml_height = (ui.available_height() - 56.0).max(180.0);
        egui::ScrollArea::vertical()
            .id_salt("raw_yaml_source_scroll")
            .max_height(yaml_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.raw_draft)
                        .desired_rows(40)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
            });
        ui.add_space(8.0);'''
if old_raw not in t:
    raise SystemExit("app_legacy.rs: raw YAML editor anchor not found")
t = t.replace(old_raw, new_raw, 1)
p.write_text(t, encoding="utf-8")
print("patched espanso-editor/src/app_legacy.rs")

# 7. Help must describe the backend that is actually shipped.
p = Path("espanso-editor/src/app_shell.rs")
t = p.read_text(encoding="utf-8")
old = "Astra/X11: глобальные сочетания отслеживаются через XInput2 Raw Events без эксклюзивного XGrabKey."
new = "Astra/X11: клавиатура отслеживается через XQueryKeymap; занятые глобальные сочетания работают через polling-fallback."
if old in t:
    t = t.replace(old, new, 1)
    p.write_text(t, encoding="utf-8")
    print("patched espanso-editor/src/app_shell.rs")
else:
    print("app_shell.rs: backend help text already changed or absent")

# Static assertions that make accidental partial application fail loudly.
checks = {
    "espanso-config/src/config/resolve.rs": 'CTRL+ALT+SPACE',
    "espanso-detect/src/x11/mod.rs": 'std::str::from_utf8(bytes)',
    "espanso/src/cli/worker/engine/funnel/mod.rs": 'sender.send((event, source_id)).is_err()',
    "espanso/src/cli/worker/engine/dispatch/executor/clipboard_injector.rs": 'X11 PRIMARY selection',
    "espanso-modulo/src/sys/form/form.cpp": 'FitFormToContent',
    "espanso-editor/src/app_legacy.rs": 'raw_yaml_source_scroll',
}
for file, marker in checks.items():
    if marker not in Path(file).read_text(encoding="utf-8"):
        raise SystemExit(f"verification failed: {marker!r} missing from {file}")
print("all final Astra fix markers verified")
