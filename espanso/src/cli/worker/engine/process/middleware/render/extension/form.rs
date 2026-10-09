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

use std::collections::HashMap;

use espanso_render::{
    extension::form::{FormProvider, FormProviderResult},
    Number, Params, Value,
};
use log::error;

use crate::gui::{FormField, FormOptions, FormUI};

pub struct FormProviderAdapter<'a> {
    form_ui: &'a dyn FormUI,
}

impl<'a> FormProviderAdapter<'a> {
    pub fn new(form_ui: &'a dyn FormUI) -> Self {
        Self { form_ui }
    }
}

impl FormProvider for FormProviderAdapter<'_> {
    fn show(&self, layout: &str, fields: &Params, options: &Params) -> FormProviderResult {
        let fields = convert_fields(fields);
        let preview = options
            .get("preview")
            .and_then(Value::as_bool)
            .copied()
            .unwrap_or(false);

        let form_options = FormOptions {
            preview,
            preview_layout: options
                .get("preview_layout")
                .and_then(Value::as_string)
                .cloned(),
            preview_mode: options
                .get("preview_mode")
                .and_then(Value::as_string)
                .cloned()
                .unwrap_or_else(|| "live".to_owned()),
            preview_debounce_ms: options
                .get("preview_debounce_ms")
                .and_then(value_as_usize)
                .unwrap_or(350),
            computed: options
                .get("computed")
                .map(render_value_to_json)
                .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new())),
        };

        match self
            .form_ui
            .show_with_options(layout, &fields, &form_options)
        {
            Ok(Some(results)) => FormProviderResult::Success(results),
            Ok(None) => FormProviderResult::Aborted,
            Err(err) => FormProviderResult::Error(err),
        }
    }
}

fn value_as_usize(value: &Value) -> Option<usize> {
    match value {
        Value::Number(Number::Integer(value)) => usize::try_from(*value).ok(),
        Value::Number(Number::Float(value)) if *value >= 0.0 => Some(*value as usize),
        _ => None,
    }
}

fn render_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::Number(Number::Integer(value)) => (*value).into(),
        Value::Number(Number::Float(value)) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(value) => serde_json::Value::String(value.clone()),
        Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(render_value_to_json).collect())
        }
        Value::Object(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(name, value)| (name.clone(), render_value_to_json(value)))
                .collect(),
        ),
    }
}

fn convert_fields(fields: &Params) -> HashMap<String, FormField> {
    let mut out = HashMap::new();
    for (name, field) in fields {
        let mut form_field = None;

        if let Value::Object(params) = field {
            form_field = match params.get("type") {
                Some(Value::String(field_type)) if field_type == "choice" => {
                    Some(FormField::Choice {
                        default: params
                            .get("default")
                            .and_then(|val| val.as_string())
                            .cloned(),
                        values: params
                            .get("values")
                            .and_then(|v| extract_values(v, params.get("trim_string_values")))
                            .unwrap_or_default(),
                    })
                }
                Some(Value::String(field_type)) if field_type == "list" => Some(FormField::List {
                    default: params
                        .get("default")
                        .and_then(|val| val.as_string())
                        .cloned(),
                    values: params
                        .get("values")
                        .and_then(|v| extract_values(v, params.get("trim_string_values")))
                        .unwrap_or_default(),
                    separator: params
                        .get("separator")
                        .and_then(|val| val.as_string())
                        .cloned()
                        .unwrap_or(", ".to_string()),
                }),
                // By default, it's considered type 'text'
                _ => Some(FormField::Text {
                    default: params
                        .get("default")
                        .and_then(|val| val.as_string())
                        .cloned(),
                    multiline: params
                        .get("multiline")
                        .and_then(|val| val.as_bool())
                        .copied()
                        .unwrap_or(false),
                }),
            }
        }

        if let Some(form_field) = form_field {
            out.insert(name.clone(), form_field);
        } else {
            error!("malformed form field format for '{name}'");
        }
    }
    out
}

fn extract_values(value: &Value, trim_string_values: Option<&Value>) -> Option<Vec<String>> {
    let trim_string_values = *trim_string_values
        .and_then(|v| v.as_bool())
        .unwrap_or(&true);

    match value {
        Value::Array(values) => Some(
            values
                .iter()
                .filter_map(|choice| choice.as_string())
                .cloned()
                .collect(),
        ),
        Value::String(values) => Some(
            values
                .lines()
                .filter_map(|line| {
                    if trim_string_values {
                        let trimmed_line = line.trim();
                        if trimmed_line.is_empty() {
                            None
                        } else {
                            Some(trimmed_line)
                        }
                    } else {
                        Some(line)
                    }
                })
                .map(String::from)
                .collect(),
        ),
        _ => None,
    }
}
