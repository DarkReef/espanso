/*
 * This file is part of modulo.
 *
 * Copyright (C) 2020-2021 Federico Terzi
 *
 * modulo is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * modulo is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with modulo.  If not, see <https://www.gnu.org/licenses/>.
 */

use super::config::{FieldConfig, FieldTypeConfig, FormConfig};
use super::parser::layout::Token;
use crate::sys::form::types::{
    ChoiceMetadata, ChoiceType, Field, FieldType, Form, LabelMetadata, PreviewMode, RowMetadata,
    TextMetadata,
};
use std::collections::HashMap;

const PREVIEW_SENTINEL_ID: &str = "__respanso_preview__";

pub fn generate(config: FormConfig) -> Form {
    let structure = super::parser::layout::parse_layout(&config.layout);
    build_form(config, structure)
}

fn create_field(token: &Token, field_map: &HashMap<String, FieldConfig>) -> Field {
    match token {
        Token::Text(text) => Field {
            field_type: FieldType::Label(LabelMetadata { text: text.clone() }),
            ..Default::default()
        },
        Token::Field(name) => {
            let config = if let Some(config) = field_map.get(name) {
                config.clone()
            } else {
                FieldConfig::default()
            };

            let field_type = match &config.field_type {
                FieldTypeConfig::Text(config) => FieldType::Text(TextMetadata {
                    default_text: config.default.clone(),
                    multiline: config.multiline,
                }),
                FieldTypeConfig::Choice(config) => FieldType::Choice(ChoiceMetadata {
                    values: config.values.clone(),
                    choice_type: ChoiceType::Dropdown,
                    default_value: config.default.clone(),
                    separator: String::new(),
                }),
                FieldTypeConfig::List(config) => FieldType::Choice(ChoiceMetadata {
                    values: config.values.clone(),
                    choice_type: ChoiceType::List,
                    default_value: config.default.clone(),
                    separator: config.separator.clone(),
                }),
            };

            Field {
                id: Some(name.clone()),
                field_type,
            }
        }
    }
}

fn build_form(form: FormConfig, structure: Vec<Vec<Token>>) -> Form {
    let computed_preview = !form.computed.is_empty();
    let preview_mode = if computed_preview {
        match form.preview_mode.trim().to_ascii_lowercase().as_str() {
            "manual" => PreviewMode::Manual,
            "submit" => PreviewMode::Submit,
            _ => PreviewMode::Live,
        }
    } else {
        PreviewMode::Layout
    };
    let preview_debounce_ms = form.preview_debounce_ms.clamp(50, 5_000) as i32;
    let field_map = form.fields;
    let mut fields = Vec::new();

    for row in &structure {
        let current_field = if row.len() == 1 {
            // Single field
            create_field(&row[0], &field_map)
        } else {
            // Row field
            let inner_fields = row
                .iter()
                .map(|token| create_field(token, &field_map))
                .collect();

            Field {
                field_type: FieldType::Row(RowMetadata {
                    fields: inner_fields,
                }),
                ..Default::default()
            }
        };

        fields.push(current_field);
    }

    if form.preview {
        fields.push(Field {
            id: Some(PREVIEW_SENTINEL_ID.to_owned()),
            field_type: FieldType::Label(LabelMetadata {
                text: String::new(),
            }),
        });
    }

    Form {
        title: form.title,
        icon: form.icon,
        fields,
        max_form_width: form.max_form_width,
        max_form_height: form.max_form_height,
        computed_preview,
        preview_mode,
        preview_debounce_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form_config(preview: bool) -> FormConfig {
        FormConfig {
            title: "test".to_owned(),
            icon: None,
            layout: "Result: [[value]]".to_owned(),
            fields: HashMap::new(),
            preview,
            preview_layout: None,
            preview_mode: "live".to_owned(),
            preview_debounce_ms: 350,
            computed: HashMap::new(),
            max_form_width: 700,
            max_form_height: 500,
        }
    }

    #[test]
    fn preview_adds_native_marker() {
        let form = generate(form_config(true));
        assert_eq!(
            form.fields.last().and_then(|field| field.id.as_deref()),
            Some(PREVIEW_SENTINEL_ID)
        );
    }

    #[test]
    fn disabled_preview_keeps_original_fields() {
        let form = generate(form_config(false));
        assert!(form
            .fields
            .iter()
            .all(|field| field.id.as_deref() != Some(PREVIEW_SENTINEL_ID)));
    }
}
