/*
 * Reactive Rhai previews for rEspanso forms.
 */

use std::{collections::HashMap, sync::LazyLock};

use anyhow::{anyhow, bail, Result};
use espanso_modulo::form::{
    config::{ComputedConfig, FormConfig},
    ComputedPreviewResult, FormPreviewEvaluator, PreviewRequest,
};
use espanso_render::{
    extension::rhai::RhaiExtension, Context, Extension, ExtensionOutput, ExtensionResult, Params,
    Scope, Value,
};
use regex::Regex;

use crate::path::Paths;

static PREVIEW_PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{\s*([A-Za-z_][A-Za-z0-9_.]*)\s*\}\}")
        .expect("preview placeholder regex should compile")
});

#[derive(Debug, Clone)]
enum NodeOutput {
    Single(String),
    Multiple(HashMap<String, String>),
}

pub struct RhaiFormPreviewEvaluator {
    nodes: Vec<(String, ComputedConfig)>,
    preview_layout: Option<String>,
    extension: RhaiExtension,
}

impl RhaiFormPreviewEvaluator {
    pub fn new(config: &FormConfig, paths: &Paths) -> Result<Option<Self>> {
        if config.computed.is_empty() {
            return Ok(None);
        }
        let nodes = resolve_order(&config.computed)?;
        Ok(Some(Self {
            nodes,
            preview_layout: config.preview_layout.clone(),
            extension: RhaiExtension::new(&paths.config, &paths.packages),
        }))
    }

    fn calculate(&self, fields: &HashMap<String, String>) -> Result<ComputedPreviewResult> {
        let context = Context::default();
        let mut completed: Vec<(String, NodeOutput)> = Vec::new();

        for (name, node) in &self.nodes {
            if node.computed_type != "rhai" {
                bail!(
                    "вычисляемое значение '{name}' использует неподдерживаемый тип '{}'",
                    node.computed_type
                );
            }

            let mut input = fields.clone();
            for (completed_name, output) in &completed {
                match output {
                    NodeOutput::Single(value) => {
                        input.insert(completed_name.clone(), value.clone());
                    }
                    NodeOutput::Multiple(values) => {
                        if let Some(primary) = primary_value(values) {
                            input.insert(completed_name.clone(), primary);
                        }
                        for (key, value) in values {
                            input.insert(format!("{completed_name}__{key}"), value.clone());
                        }
                    }
                }
            }

            let mut scope: Scope<'_> = Scope::new();
            scope.insert("form", ExtensionOutput::Multiple(input));
            let mut params = Params::new();
            params.insert("path".to_owned(), Value::String(node.path.clone()));
            params.insert("function".to_owned(), Value::String(node.function.clone()));
            params.insert("input".to_owned(), Value::String("form".to_owned()));

            let output = match self.extension.calculate(&context, &scope, &params) {
                ExtensionResult::Success(ExtensionOutput::Single(value)) => {
                    NodeOutput::Single(value)
                }
                ExtensionResult::Success(ExtensionOutput::Multiple(values)) => {
                    if values.get("status").is_some_and(|status| status == "error") {
                        let message = values
                            .get("text")
                            .or_else(|| values.get("message"))
                            .cloned()
                            .unwrap_or_else(|| "скрипт сообщил об ошибке".to_owned());
                        bail!("{name}: {message}");
                    }
                    NodeOutput::Multiple(values)
                }
                ExtensionResult::Aborted => bail!("вычисление '{name}' было отменено"),
                ExtensionResult::Error(error) => {
                    return Err(anyhow!("{name}: {error}"));
                }
            };
            completed.push((name.clone(), output));
        }

        let mut lookup = fields.clone();
        let mut final_values = HashMap::new();
        for (name, output) in &completed {
            match output {
                NodeOutput::Single(value) => {
                    lookup.insert(name.clone(), value.clone());
                    final_values.insert(name.clone(), value.clone());
                }
                NodeOutput::Multiple(values) => {
                    if let Some(primary) = primary_value(values) {
                        lookup.insert(name.clone(), primary.clone());
                        final_values.insert(name.clone(), primary);
                    }
                    for (key, value) in values {
                        lookup.insert(format!("{name}.{key}"), value.clone());
                        final_values.insert(format!("{name}__{key}"), value.clone());
                    }
                }
            }
        }

        let text = if let Some(layout) = &self.preview_layout {
            PREVIEW_PLACEHOLDER
                .replace_all(layout, |captures: &regex::Captures<'_>| {
                    lookup
                        .get(&captures[1])
                        .cloned()
                        .unwrap_or_else(|| captures[0].to_owned())
                })
                .into_owned()
        } else {
            completed
                .iter()
                .map(|(name, output)| match output {
                    NodeOutput::Single(value) => value.clone(),
                    NodeOutput::Multiple(values) => primary_value(values).unwrap_or_else(|| {
                        let mut parts: Vec<String> = values
                            .iter()
                            .filter(|(key, _)| key.as_str() != "status")
                            .map(|(key, value)| format!("{key}: {value}"))
                            .collect();
                        parts.sort();
                        format!("{name}: {}", parts.join(", "))
                    }),
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(ComputedPreviewResult {
            text,
            values: final_values,
        })
    }
}

impl FormPreviewEvaluator for RhaiFormPreviewEvaluator {
    fn evaluate(
        &mut self,
        values: &HashMap<String, String>,
        _request: PreviewRequest,
    ) -> Result<ComputedPreviewResult, String> {
        self.calculate(values).map_err(|error| error.to_string())
    }
}

fn primary_value(values: &HashMap<String, String>) -> Option<String> {
    values
        .get("value")
        .or_else(|| values.get("text"))
        .or_else(|| values.get("result"))
        .cloned()
}

fn resolve_order(
    computed: &HashMap<String, ComputedConfig>,
) -> Result<Vec<(String, ComputedConfig)>> {
    let mut remaining: Vec<String> = computed.keys().cloned().collect();
    remaining.sort();
    let mut resolved: Vec<(String, ComputedConfig)> = Vec::new();

    while !remaining.is_empty() {
        let resolved_names: std::collections::HashSet<&str> =
            resolved.iter().map(|(name, _)| name.as_str()).collect();
        let next = remaining.iter().position(|name| {
            computed[name].depends_on.iter().all(|dependency| {
                !computed.contains_key(dependency) || resolved_names.contains(dependency.as_str())
            })
        });

        let Some(index) = next else {
            bail!("обнаружена циклическая зависимость вычисляемых значений формы");
        };
        let name = remaining.remove(index);
        resolved.push((name.clone(), computed[&name].clone()));
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use espanso_modulo::form::config::FormConfig;
    use tempdir::TempDir;

    use super::*;

    #[test]
    fn calculates_rhai_preview_and_returns_computed_value() {
        let dir = TempDir::new("respanso-reactive-preview").unwrap();
        let script = dir.path().join("score.rhai");
        fs::write(
            &script,
            r#"fn calculate(input) { #{ value: parse_int(input.age) + 1, text: `Возраст + 1: ${parse_int(input.age) + 1}` } }"#,
        )
        .unwrap();
        let config: FormConfig = serde_json::from_value(serde_json::json!({
            "layout": "Возраст: [[age]]",
            "preview": true,
            "preview_layout": "{{score.text}}",
            "computed": {
                "score": {
                    "type": "rhai",
                    "path": script.to_string_lossy(),
                    "depends_on": ["age"]
                }
            }
        }))
        .unwrap();
        let paths = Paths {
            config: dir.path().to_path_buf(),
            runtime: dir.path().to_path_buf(),
            packages: dir.path().to_path_buf(),
            is_portable_mode: true,
        };
        let mut evaluator = RhaiFormPreviewEvaluator::new(&config, &paths)
            .unwrap()
            .unwrap();
        let result = evaluator
            .evaluate(
                &[("age".to_owned(), "42".to_owned())].into_iter().collect(),
                PreviewRequest::Live,
            )
            .unwrap();
        assert_eq!(result.text, "Возраст + 1: 43");
        assert_eq!(result.values.get("score"), Some(&"43".to_owned()));
        assert_eq!(
            result.values.get("score__text"),
            Some(&"Возраст + 1: 43".to_owned())
        );
    }
}
