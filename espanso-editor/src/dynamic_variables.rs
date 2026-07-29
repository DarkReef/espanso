#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableDefinition {
    pub name: String,
    pub variable_type: String,
    pub params_yaml: String,
}

impl VariableDefinition {
    #[must_use]
    pub fn placeholder(&self) -> String {
        format!("{{{{{}}}}}", self.name)
    }
}

#[must_use]
pub fn builtin_definitions_in(replacement: &str) -> Vec<VariableDefinition> {
    let mut definitions = Vec::new();
    if replacement.contains("{{date}}") {
        definitions.push(VariableDefinition {
            name: "date".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: "format: \"%d.%m.%Y\"".to_owned(),
        });
    }
    if replacement.contains("{{time}}") {
        definitions.push(VariableDefinition {
            name: "time".to_owned(),
            variable_type: "time".to_owned(),
            params_yaml: String::new(),
        });
    }
    if replacement.contains("{{weekday}}") {
        definitions.push(VariableDefinition {
            name: "weekday".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: "format: \"%A\"\nlocale: \"ru-RU\"".to_owned(),
        });
    }
    if replacement.contains("{{clipboard}}") {
        definitions.push(VariableDefinition {
            name: "clipboard".to_owned(),
            variable_type: "clipboard".to_owned(),
            params_yaml: String::new(),
        });
    }
    definitions
}

pub fn canonical_definition(
    definition: &VariableDefinition,
) -> Result<VariableDefinition, String> {
    let variable_type = definition.variable_type.trim().to_ascii_lowercase();
    let mut canonical = definition.clone();
    canonical.name = canonical.name.trim().to_owned();
    canonical.variable_type.clone_from(&variable_type);
    canonical.params_yaml = canonical.params_yaml.trim().to_owned();

    match variable_type.as_str() {
        "time" => {
            "date".clone_into(&mut canonical.variable_type);
            if canonical.params_yaml.is_empty() {
                "format: \"%H:%M\"".clone_into(&mut canonical.params_yaml);
            }
        }
        "string" => {
            if canonical.params_yaml.is_empty() {
                return Err("Для типа string укажите значение строки".to_owned());
            }
            "echo".clone_into(&mut canonical.variable_type);
            canonical.params_yaml = format!(
                "echo: {}",
                serde_json::to_string(&canonical.params_yaml)
                    .map_err(|error| format!("Не удалось подготовить строку: {error}"))?
            );
        }
        _ => {}
    }
    Ok(canonical)
}

pub fn validate_definition(definition: &VariableDefinition) -> Result<(), String> {
    let mut characters = definition.name.chars();
    let Some(first) = characters.next() else {
        return Err("Укажите имя переменной".to_owned());
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(
            "Имя должно начинаться с латинской буквы или _, далее допустимы буквы, цифры и _"
                .to_owned(),
        );
    }

    const SUPPORTED_TYPES: &[&str] = &[
        "date",
        "clipboard",
        "echo",
        "choice",
        "form",
        "random",
        "rhai",
        "script",
        "shell",
    ];
    if !SUPPORTED_TYPES.contains(&definition.variable_type.as_str()) {
        return Err(format!(
            "Неизвестный тип переменной '{}'. Допустимы: {}",
            definition.variable_type,
            SUPPORTED_TYPES.join(", ")
        ));
    }

    let _ = normalized_params(&definition.params_yaml)?;
    Ok(())
}

fn normalized_params(params_yaml: &str) -> Result<Option<String>, String> {
    if params_yaml.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_norway::from_str(params_yaml)
        .map_err(|error| format!("Ошибка YAML в параметрах: {error}"))?;
    if !value.is_object() {
        return Err("Параметры должны быть YAML-объектом вида key: value".to_owned());
    }
    let rendered = serde_norway::to_string(&value)
        .map_err(|error| format!("Не удалось подготовить параметры: {error}"))?;
    Ok(Some(rendered.trim().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_manually_typed_builtin_placeholders() {
        let definitions = builtin_definitions_in("Дата {{date}}, время {{time}}, {{clipboard}}");
        assert_eq!(definitions.len(), 3);
        assert_eq!(definitions[0].name, "date");
        assert_eq!(definitions[1].name, "time");
        assert_eq!(definitions[2].name, "clipboard");
    }

    #[test]
    fn time_alias_uses_date_extension() {
        let definition = canonical_definition(&VariableDefinition {
            name: "time".to_owned(),
            variable_type: "time".to_owned(),
            params_yaml: String::new(),
        })
        .unwrap();
        assert_eq!(definition.variable_type, "date");
        assert!(definition.params_yaml.contains("%H:%M"));
        validate_definition(&definition).unwrap();
    }

    #[test]
    fn string_alias_uses_echo_extension() {
        let definition = canonical_definition(&VariableDefinition {
            name: "doc".to_owned(),
            variable_type: "string".to_owned(),
            params_yaml: "Куцин Иван Юрьевич".to_owned(),
        })
        .unwrap();
        assert_eq!(definition.variable_type, "echo");
        assert!(definition.params_yaml.contains("Куцин Иван Юрьевич"));
        validate_definition(&definition).unwrap();
    }

    #[test]
    fn rejects_invalid_variable_name() {
        let definition = VariableDefinition {
            name: "patient-name".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: String::new(),
        };
        assert!(validate_definition(&definition).is_err());
    }

    #[test]
    fn rejects_unknown_variable_type() {
        let definition = VariableDefinition {
            name: "date".to_owned(),
            variable_type: "datе".to_owned(),
            params_yaml: String::new(),
        };
        assert!(validate_definition(&definition).is_err());
    }
}
