from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def patch(path: str, old: str, new: str, label: str) -> None:
    file = ROOT / path
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


patch(
    "espanso-editor/src/app.rs",
    "    dynamic_variables::{self, VariableDefinition},\n",
    "    dynamic_variables,\n",
    "remove unused import",
)
patch(
    "espanso-editor/src/app.rs",
    '''        let logical_path = if disabled {
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
''',
    '''        let logical_path = if disabled {
            let Some(path) = enabled_yaml_path(&path) else {
                "Не удалось определить исходное имя выключенного YAML-файла"
                    .clone_into(&mut self.status);
                return false;
            };
            path
        } else {
            path.clone()
        };
''',
    "use let-else for disabled file",
)
patch(
    "espanso-editor/src/app.rs",
    '''    let enabled_lower = enabled_name.to_ascii_lowercase();
    if !enabled_lower.ends_with(".yml") && !enabled_lower.ends_with(".yaml") {
        return None;
    }
''',
    '''    let extension = Path::new(enabled_name)
        .extension()
        .and_then(|extension| extension.to_str());
    if !extension.is_some_and(|extension| {
        extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
    }) {
        return None;
    }
''',
    "use Path extension comparison",
)
patch(
    "espanso-editor/src/global_variables.rs",
    '''                        self.name = "my_variable".to_owned();
                        self.variable_type = "string".to_owned();
''',
    '''                        "my_variable".clone_into(&mut self.name);
                        "string".clone_into(&mut self.variable_type);
''',
    "avoid assigning clones",
)
