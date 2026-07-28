from __future__ import annotations

import base64
import shutil
import zlib
from pathlib import Path

chunks = Path(__file__).with_name(".patch_chunks")
payload = "".join(path.read_text(encoding="utf-8") for path in sorted(chunks.glob("part*.txt")))
try:
    source = zlib.decompress(base64.b64decode(payload)).decode("utf-8")
    source = source.replace(
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub enum MatchKind",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum MatchKind",
        1,
    )
    source = source.replace(
        "'''            match rule.draft.kind {\n                MatchKind::Trigger => {\n''',",
        "'''        for rule in &rules {\n            match rule.draft.kind {\n                MatchKind::Trigger => {\n''',",
        1,
    )
    source = source.replace(
        "'''            let mut variable_names = HashSet::new();",
        "'''        for rule in &rules {\n            let mut variable_names = HashSet::new();",
        1,
    )
    exec(compile(source, str(Path(__file__)), "exec"))

    app_path = Path("espanso-editor/src/app.rs")
    app = app_path.read_text(encoding="utf-8")
    app = app.replace('set_string_param(variable, "echo", "".to_owned())', 'set_string_param(variable, "echo", String::new())')
    app = app.replace('set_string_param(variable, "cmd", "".to_owned())', 'set_string_param(variable, "cmd", String::new())')
    app = app.replace('set_string_param(variable, "layout", "".to_owned())', 'set_string_param(variable, "layout", String::new())')
    app_path.write_text(app, encoding="utf-8")

    workspace_path = Path("espanso-editor/src/workspace.rs")
    workspace = workspace_path.read_text(encoding="utf-8")
    workspace = workspace.replace(
        '''        result.push_str(&format!(
            "{item_indent}- name: {}\\n",
            serde_json::to_string(variable.name.trim()).unwrap_or_else(|_| "\\\"\\\"".to_owned())
        ));''',
        '''        let _ = writeln!(
            result,
            "{item_indent}- name: {}",
            serde_json::to_string(variable.name.trim()).unwrap_or_else(|_| "\\\"\\\"".to_owned())
        );''',
    )
    workspace = workspace.replace(
        '''        result.push_str(&format!(
            "{field_indent}type: {}\\n",
            serde_json::to_string(variable.var_type.trim())
                .unwrap_or_else(|_| "\\\"\\\"".to_owned())
        ));''',
        '''        let _ = writeln!(
            result,
            "{field_indent}type: {}",
            serde_json::to_string(variable.var_type.trim())
                .unwrap_or_else(|_| "\\\"\\\"".to_owned())
        );''',
    )
    workspace = workspace.replace(
        '            result.push_str(&format!("{field_indent}params:\\n"));',
        '            let _ = writeln!(result, "{field_indent}params:");',
    )
    workspace = workspace.replace(
        '                result.push_str(&format!("{param_indent}{key}: {encoded}\\n"));',
        '                let _ = writeln!(result, "{param_indent}{key}: {encoded}");',
    )
    workspace_path.write_text(workspace, encoding="utf-8")
finally:
    shutil.rmtree(chunks, ignore_errors=True)
