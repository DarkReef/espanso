/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::{collections::HashMap, path::{Path, PathBuf}};

use rhai::{Array, Dynamic, Engine, Map, Scope as RhaiScope};
use thiserror::Error;

use crate::{
    Extension, ExtensionOutput, ExtensionResult, Number, Params, Scope, Value,
};

const DEFAULT_ENTRYPOINT: &str = "calculate";
const MAX_OPERATIONS: u64 = 100_000;
const MAX_CALL_LEVELS: usize = 32;
const MAX_EXPR_DEPTH: usize = 64;
const MAX_FUNCTION_EXPR_DEPTH: usize = 32;
const MAX_STRING_SIZE: usize = 64 * 1024;
const MAX_ARRAY_SIZE: usize = 2_048;
const MAX_MAP_SIZE: usize = 512;

pub struct RhaiExtension {
    config_path: PathBuf,
    packages_path: PathBuf,
}

impl RhaiExtension {
    pub fn new(config_path: &Path, packages_path: &Path) -> Self {
        Self {
            config_path: config_path.to_owned(),
            packages_path: packages_path.to_owned(),
        }
    }

    fn resolve_script_path(&self, raw_path: &str) -> Result<PathBuf, RhaiExtensionError> {
        let expanded = raw_path
            .replace("%CONFIG%", &self.config_path.to_string_lossy())
            .replace("%PACKAGES%", &self.packages_path.to_string_lossy());

        let candidate = PathBuf::from(expanded);
        let candidate = if candidate.is_absolute() {
            candidate
        } else {
            self.config_path.join(candidate)
        };

        let script_path = candidate.canonicalize().map_err(|error| {
            RhaiExtensionError::UnableToResolvePath(candidate.clone(), error)
        })?;

        let config_root = canonical_or_original(&self.config_path);
        let packages_root = canonical_or_original(&self.packages_path);

        if !script_path.starts_with(&config_root) && !script_path.starts_with(&packages_root) {
            return Err(RhaiExtensionError::PathOutsideAllowedRoots(script_path));
        }

        Ok(script_path)
    }
}

impl Extension for RhaiExtension {
    fn name(&self) -> &'static str {
        "rhai"
    }

    fn calculate(
        &self,
        _: &crate::Context,
        scope: &Scope,
        params: &Params,
    ) -> ExtensionResult {
        let Some(Value::String(raw_path)) = params.get("path") else {
            return ExtensionResult::Error(RhaiExtensionError::MissingPath.into());
        };

        let entrypoint = match params.get("function") {
            Some(Value::String(value)) => value.as_str(),
            _ => DEFAULT_ENTRYPOINT,
        };

        let input = match params.get("input") {
            Some(Value::String(name)) => {
                let Some(value) = scope.get(name.as_str()) else {
                    return ExtensionResult::Error(
                        RhaiExtensionError::MissingInput(name.clone()).into(),
                    );
                };
                extension_output_to_dynamic(value)
            }
            _ => scope_to_dynamic(scope),
        };

        let script_path = match self.resolve_script_path(raw_path) {
            Ok(path) => path,
            Err(error) => return ExtensionResult::Error(error.into()),
        };

        let mut engine = create_restricted_engine();
        let ast = match engine.compile_file(script_path.clone()) {
            Ok(ast) => ast,
            Err(error) => {
                return ExtensionResult::Error(
                    RhaiExtensionError::Compilation(script_path, error).into(),
                )
            }
        };

        let result = match engine.call_fn::<Dynamic>(
            &mut RhaiScope::new(),
            &ast,
            entrypoint,
            (input,),
        ) {
            Ok(result) => result,
            Err(error) => {
                return ExtensionResult::Error(
                    RhaiExtensionError::Execution(script_path, entrypoint.to_owned(), error).into(),
                )
            }
        };

        ExtensionResult::Success(dynamic_to_extension_output(result))
    }
}

fn create_restricted_engine() -> Engine {
    let mut engine = Engine::new();
    engine
        .set_max_operations(MAX_OPERATIONS)
        .set_max_call_levels(MAX_CALL_LEVELS)
        .set_max_expr_depths(MAX_EXPR_DEPTH, MAX_FUNCTION_EXPR_DEPTH)
        .set_max_string_size(MAX_STRING_SIZE)
        .set_max_array_size(MAX_ARRAY_SIZE)
        .set_max_map_size(MAX_MAP_SIZE);

    // Dynamic evaluation and module loading are unnecessary for calculator modules and
    // make scripts harder to audit. The standard Rhai engine exposes no filesystem,
    // network or process APIs unless the host explicitly registers them.
    engine.disable_symbol("eval");
    engine.disable_symbol("import");
    engine
}

fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

fn scope_to_dynamic(scope: &Scope) -> Dynamic {
    let mut map = Map::new();
    for (name, value) in scope {
        map.insert((*name).into(), extension_output_to_dynamic(value));
    }
    Dynamic::from_map(map)
}

fn extension_output_to_dynamic(output: &ExtensionOutput) -> Dynamic {
    match output {
        ExtensionOutput::Single(value) => Dynamic::from(value.clone()),
        ExtensionOutput::Multiple(values) => {
            let mut map = Map::new();
            for (name, value) in values {
                map.insert(name.as_str().into(), Dynamic::from(value.clone()));
            }
            Dynamic::from_map(map)
        }
    }
}

#[allow(dead_code)]
fn value_to_dynamic(value: &Value) -> Dynamic {
    match value {
        Value::Null => Dynamic::UNIT,
        Value::Bool(value) => Dynamic::from(*value),
        Value::Number(Number::Integer(value)) => Dynamic::from(*value),
        Value::Number(Number::Float(value)) => Dynamic::from(*value),
        Value::String(value) => Dynamic::from(value.clone()),
        Value::Array(values) => {
            let array: Array = values.iter().map(value_to_dynamic).collect();
            array.into()
        }
        Value::Object(values) => {
            let mut map = Map::new();
            for (name, value) in values {
                map.insert(name.as_str().into(), value_to_dynamic(value));
            }
            Dynamic::from_map(map)
        }
    }
}

fn dynamic_to_extension_output(value: Dynamic) -> ExtensionOutput {
    if value.is::<Map>() {
        let map = value
            .try_cast::<Map>()
            .expect("Rhai Dynamic type was checked as Map");
        let values = map
            .into_iter()
            .map(|(name, value)| (name.to_string(), dynamic_to_text(value)))
            .collect::<HashMap<_, _>>();
        ExtensionOutput::Multiple(values)
    } else {
        ExtensionOutput::Single(dynamic_to_text(value))
    }
}

fn dynamic_to_text(value: Dynamic) -> String {
    if value.is_unit() {
        String::new()
    } else {
        value.to_string()
    }
}

#[derive(Error, Debug)]
pub enum RhaiExtensionError {
    #[error("missing 'path' parameter")]
    MissingPath,

    #[error("Rhai input variable '{0}' is unavailable")]
    MissingInput(String),

    #[error("unable to resolve Rhai script path '{0}': {1}")]
    UnableToResolvePath(PathBuf, std::io::Error),

    #[error("Rhai script path is outside the config and packages directories: '{0}'")]
    PathOutsideAllowedRoots(PathBuf),

    #[error("unable to compile Rhai script '{0}': {1}")]
    Compilation(PathBuf, Box<rhai::EvalAltResult>),

    #[error("Rhai function '{1}' failed in script '{0}': {2}")]
    Execution(PathBuf, String, Box<rhai::EvalAltResult>),
}

#[cfg(test)]
mod tests {
    use std::{fs, collections::HashMap};

    use tempdir::TempDir;

    use super::*;

    fn params(path: &Path) -> Params {
        [
            (
                "path".to_owned(),
                Value::String(path.to_string_lossy().into_owned()),
            ),
            ("input".to_owned(), Value::String("form".to_owned())),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn executes_script_without_external_runtime() {
        let dir = TempDir::new("respanso-rhai").expect("temporary directory should be created");
        let script = dir.path().join("calculator.rhai");
        fs::write(
            &script,
            r#"
                fn calculate(input) {
                    let age = parse_int(input.age);
                    `Age next year: ${age + 1}`
                }
            "#,
        )
        .expect("test script should be written");

        let extension = RhaiExtension::new(dir.path(), dir.path());
        let mut fields = HashMap::new();
        fields.insert("age".to_owned(), "42".to_owned());
        let mut scope = Scope::new();
        scope.insert("form", ExtensionOutput::Multiple(fields));

        let output = extension
            .calculate(&crate::Context::default(), &scope, &params(&script))
            .into_success()
            .expect("script should execute");

        assert_eq!(
            output,
            ExtensionOutput::Single("Age next year: 43".to_owned())
        );
    }

    #[test]
    fn converts_map_result_to_multiple_output() {
        let dir = TempDir::new("respanso-rhai-map")
            .expect("temporary directory should be created");
        let script = dir.path().join("calculator.rhai");
        fs::write(
            &script,
            r#"
                fn calculate(input) {
                    #{ score: parse_int(input.age) + 1, text: `Age: ${input.age}` }
                }
            "#,
        )
        .expect("test script should be written");

        let extension = RhaiExtension::new(dir.path(), dir.path());
        let mut fields = HashMap::new();
        fields.insert("age".to_owned(), "42".to_owned());
        let mut scope = Scope::new();
        scope.insert("form", ExtensionOutput::Multiple(fields));

        let output = extension
            .calculate(&crate::Context::default(), &scope, &params(&script))
            .into_success()
            .expect("script should execute");

        let ExtensionOutput::Multiple(values) = output else {
            panic!("expected multiple output");
        };
        assert_eq!(values.get("score"), Some(&"43".to_owned()));
        assert_eq!(values.get("text"), Some(&"Age: 42".to_owned()));
    }
}
