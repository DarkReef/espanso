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

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_int;

// Form schema

pub mod types {
    #[repr(i32)]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum PreviewMode {
        Layout = 0,
        Live = 1,
        Manual = 2,
        Submit = 3,
    }

    #[derive(Debug)]
    pub struct Form {
        pub title: String,
        pub icon: Option<String>,
        pub fields: Vec<Field>,
        pub max_form_width: i32,
        pub max_form_height: i32,
        pub computed_preview: bool,
        pub preview_mode: PreviewMode,
        pub preview_debounce_ms: i32,
    }

    #[derive(Debug)]
    pub struct Field {
        pub id: Option<String>,
        pub field_type: FieldType,
    }

    impl Default for Field {
        fn default() -> Self {
            Self {
                id: None,
                field_type: FieldType::Unknown,
            }
        }
    }

    #[derive(Debug)]
    pub enum FieldType {
        Unknown,
        Row(RowMetadata),
        Label(LabelMetadata),
        Text(TextMetadata),
        Choice(ChoiceMetadata),
    }

    #[derive(Debug)]
    pub struct RowMetadata {
        pub fields: Vec<Field>,
    }

    #[derive(Debug)]
    pub struct LabelMetadata {
        pub text: String,
    }

    #[derive(Debug)]
    pub struct TextMetadata {
        pub default_text: String,
        pub multiline: bool,
    }

    #[derive(Debug)]
    pub enum ChoiceType {
        Dropdown,
        List,
    }

    #[derive(Debug)]
    pub struct ChoiceMetadata {
        pub values: Vec<String>,
        pub choice_type: ChoiceType,
        pub default_value: String,
        pub separator: String,
    }
}

// Form interop

#[allow(dead_code)]
mod interop {
    use crate::sys;

    use super::super::interop::{
        ChoiceMetadata, ChoiceType_DROPDOWN, ChoiceType_LIST, FieldMetadata, FieldType,
        FieldType_CHOICE, FieldType_LABEL, FieldType_ROW, FieldType_TEXT, FormMetadata,
        Interoperable, LabelMetadata, RowMetadata, TextMetadata,
    };
    use super::types;
    use std::ffi::{c_void, CString};
    use std::os::raw::{c_char, c_int};
    use std::ptr::null;

    pub struct OwnedForm {
        title: CString,
        icon_path: CString,
        fields: Vec<OwnedField>,

        metadata: Vec<FieldMetadata>,
        interop: Box<FormMetadata>,
    }

    impl Interoperable for OwnedForm {
        fn as_ptr(&self) -> *const c_void {
            std::ptr::from_ref::<FormMetadata>(&(*self.interop)) as *const c_void
        }
    }

    impl From<types::Form> for OwnedForm {
        fn from(form: types::Form) -> Self {
            let title = CString::new(form.title).expect("unable to convert form title to CString");
            let fields: Vec<OwnedField> = form.fields.into_iter().map(Into::into).collect();

            let metadata: Vec<FieldMetadata> = fields
                .iter()
                .map(sys::form::interop::OwnedField::metadata)
                .collect();

            let icon_path = if let Some(icon_path) = form.icon.as_ref() {
                icon_path.clone()
            } else {
                String::new()
            };

            let icon_path =
                CString::new(icon_path).expect("unable to convert form icon to CString");

            let icon_path_ptr = if form.icon.is_some() {
                icon_path.as_ptr()
            } else {
                std::ptr::null()
            };

            let max_form_width = form.max_form_width;
            let max_form_height = form.max_form_height;

            let interop = Box::new(FormMetadata {
                windowTitle: title.as_ptr(),
                iconPath: icon_path_ptr,
                fields: metadata.as_ptr(),
                fieldSize: fields.len() as c_int,
                maxWindowWidth: max_form_width,
                maxWindowHeight: max_form_height,
                computedPreviewEnabled: i32::from(form.computed_preview),
                previewMode: form.preview_mode as c_int,
                previewDebounceMs: form.preview_debounce_ms,
            });

            Self {
                title,
                icon_path,
                fields,
                metadata,
                interop,
            }
        }
    }

    struct OwnedField {
        id: Option<CString>,
        field_type: FieldType,
        specific: Box<dyn Interoperable>,
    }

    impl From<types::Field> for OwnedField {
        fn from(field: types::Field) -> Self {
            let id = field
                .id
                .map(|id| CString::new(id).expect("unable to create cstring for field id"));

            let field_type = match field.field_type {
                types::FieldType::Row(_) => FieldType_ROW,
                types::FieldType::Label(_) => FieldType_LABEL,
                types::FieldType::Text(_) => FieldType_TEXT,
                types::FieldType::Choice(_) => FieldType_CHOICE,
                types::FieldType::Unknown => panic!("unknown field type"),
            };

            // TODO: clean up this match
            let specific: Box<dyn Interoperable> = match field.field_type {
                types::FieldType::Row(metadata) => {
                    let owned_metadata: OwnedRowMetadata = metadata.into();
                    Box::new(owned_metadata)
                }
                types::FieldType::Label(metadata) => {
                    let owned_metadata: OwnedLabelMetadata = metadata.into();
                    Box::new(owned_metadata)
                }
                types::FieldType::Text(metadata) => {
                    let owned_metadata: OwnedTextMetadata = metadata.into();
                    Box::new(owned_metadata)
                }
                types::FieldType::Choice(metadata) => {
                    let owned_metadata: OwnedChoiceMetadata = metadata.into();
                    Box::new(owned_metadata)
                }
                types::FieldType::Unknown => panic!("unknown field type"),
            };

            Self {
                id,
                field_type,
                specific,
            }
        }
    }

    impl OwnedField {
        pub fn metadata(&self) -> FieldMetadata {
            let id_ptr = if let Some(id) = self.id.as_ref() {
                id.as_ptr()
            } else {
                null()
            };

            FieldMetadata {
                id: id_ptr,
                fieldType: self.field_type,
                specific: self.specific.as_ptr(),
            }
        }
    }

    struct OwnedLabelMetadata {
        text: CString,
        interop: Box<LabelMetadata>,
    }

    impl Interoperable for OwnedLabelMetadata {
        fn as_ptr(&self) -> *const c_void {
            std::ptr::from_ref::<LabelMetadata>(&(*self.interop)) as *const c_void
        }
    }

    impl From<types::LabelMetadata> for OwnedLabelMetadata {
        fn from(label_metadata: types::LabelMetadata) -> Self {
            let text =
                CString::new(label_metadata.text).expect("unable to convert label text to CString");
            let interop = Box::new(LabelMetadata {
                text: text.as_ptr(),
            });
            Self { text, interop }
        }
    }

    struct OwnedTextMetadata {
        default_text: CString,
        interop: Box<TextMetadata>,
    }

    impl Interoperable for OwnedTextMetadata {
        fn as_ptr(&self) -> *const c_void {
            std::ptr::from_ref::<TextMetadata>(&(*self.interop)) as *const c_void
        }
    }

    impl From<types::TextMetadata> for OwnedTextMetadata {
        fn from(text_metadata: types::TextMetadata) -> Self {
            let default_text = CString::new(text_metadata.default_text)
                .expect("unable to convert default text to CString");
            let interop = Box::new(TextMetadata {
                defaultText: default_text.as_ptr(),
                multiline: i32::from(text_metadata.multiline),
            });
            Self {
                default_text,
                interop,
            }
        }
    }

    struct OwnedChoiceMetadata {
        values: Vec<CString>,
        values_ptr_array: Vec<*const c_char>,
        default_value: CString,
        separator: CString,
        interop: Box<ChoiceMetadata>,
    }

    impl Interoperable for OwnedChoiceMetadata {
        fn as_ptr(&self) -> *const c_void {
            std::ptr::from_ref::<ChoiceMetadata>(&(*self.interop)) as *const c_void
        }
    }

    impl From<types::ChoiceMetadata> for OwnedChoiceMetadata {
        fn from(metadata: types::ChoiceMetadata) -> Self {
            let values: Vec<CString> = metadata
                .values
                .into_iter()
                .map(|value| CString::new(value).expect("unable to convert choice value to string"))
                .collect();

            let values_ptr_array: Vec<*const c_char> =
                values.iter().map(|value| value.as_ptr()).collect();

            let choice_type = match metadata.choice_type {
                types::ChoiceType::Dropdown => ChoiceType_DROPDOWN,
                types::ChoiceType::List => ChoiceType_LIST,
            };

            let default_value = CString::new(metadata.default_value)
                .expect("unable to convert default value to CString");

            let separator =
                CString::new(metadata.separator).expect("unable to convert separator to CString");

            let interop = Box::new(ChoiceMetadata {
                values: values_ptr_array.as_ptr(),
                valueSize: values.len() as c_int,
                choiceType: choice_type,
                defaultValue: default_value.as_ptr(),
                separator: separator.as_ptr(),
            });
            Self {
                values,
                values_ptr_array,
                default_value,
                separator,
                interop,
            }
        }
    }

    struct OwnedRowMetadata {
        fields: Vec<OwnedField>,

        metadata: Vec<FieldMetadata>,
        interop: Box<RowMetadata>,
    }

    impl Interoperable for OwnedRowMetadata {
        fn as_ptr(&self) -> *const c_void {
            std::ptr::from_ref::<RowMetadata>(&(*self.interop)) as *const c_void
        }
    }

    impl From<types::RowMetadata> for OwnedRowMetadata {
        fn from(row_metadata: types::RowMetadata) -> Self {
            let fields: Vec<OwnedField> = row_metadata.fields.into_iter().map(Into::into).collect();

            let metadata: Vec<FieldMetadata> = fields
                .iter()
                .map(sys::form::interop::OwnedField::metadata)
                .collect();

            let interop = Box::new(RowMetadata {
                fields: metadata.as_ptr(),
                fieldSize: metadata.len() as c_int,
            });

            Self {
                fields,
                metadata,
                interop,
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PreviewRequest {
    Live,
    Manual,
    Submit,
}

#[derive(Debug, Clone, Default)]
pub struct ComputedPreviewResult {
    pub text: String,
    pub values: HashMap<String, String>,
}

pub trait FormPreviewEvaluator {
    fn evaluate(
        &mut self,
        values: &HashMap<String, String>,
        request: PreviewRequest,
    ) -> Result<ComputedPreviewResult, String>;
}

struct PreviewCallbackState<'a> {
    evaluator: &'a mut dyn FormPreviewEvaluator,
    last_text: std::ffi::CString,
    last_success: Option<ComputedPreviewResult>,
}

extern "C" fn result_callback(
    values: *const super::interop::ValuePair,
    size: c_int,
    map: *mut std::ffi::c_void,
) {
    let values: &[super::interop::ValuePair] =
        unsafe { std::slice::from_raw_parts(values, size as usize) };
    let map = map as *mut HashMap<String, String>;
    let map = unsafe { &mut (*map) };
    for pair in values {
        unsafe {
            let id = CStr::from_ptr(pair.id);
            let value = CStr::from_ptr(pair.value);
            map.insert(
                id.to_string_lossy().to_string(),
                value.to_string_lossy().to_string(),
            );
        }
    }
}

extern "C" fn preview_callback(
    values: *const super::interop::ValuePair,
    size: c_int,
    request: c_int,
    status: *mut c_int,
    data: *mut std::ffi::c_void,
) -> *const std::os::raw::c_char {
    let state = unsafe { &mut *(data as *mut PreviewCallbackState<'_>) };
    let raw_values: &[super::interop::ValuePair] =
        unsafe { std::slice::from_raw_parts(values, size as usize) };
    let mut value_map = HashMap::new();
    for pair in raw_values {
        unsafe {
            value_map.insert(
                CStr::from_ptr(pair.id).to_string_lossy().to_string(),
                CStr::from_ptr(pair.value).to_string_lossy().to_string(),
            );
        }
    }

    let request = match request {
        2 => PreviewRequest::Manual,
        3 => PreviewRequest::Submit,
        _ => PreviewRequest::Live,
    };

    let (code, text) = match state.evaluator.evaluate(&value_map, request) {
        Ok(result) => {
            let text = result.text.clone();
            state.last_success = Some(result);
            (0, text)
        }
        Err(error) => {
            state.last_success = None;
            (2, format!("Не удалось рассчитать предпросмотр: {error}"))
        }
    };

    if !status.is_null() {
        unsafe { *status = code };
    }
    state.last_text = std::ffi::CString::new(text.replace('\0', " "))
        .unwrap_or_else(|_| std::ffi::CString::new("Ошибка предпросмотра").unwrap());
    state.last_text.as_ptr()
}

pub fn show(form: types::Form) -> HashMap<String, String> {
    show_internal(form, None)
}

pub fn show_with_preview(
    form: types::Form,
    evaluator: &mut dyn FormPreviewEvaluator,
) -> HashMap<String, String> {
    show_internal(form, Some(evaluator))
}

fn show_internal(
    form: types::Form,
    evaluator: Option<&mut dyn FormPreviewEvaluator>,
) -> HashMap<String, String> {
    use super::interop::{interop_show_form, FormMetadata, Interoperable};
    use std::os::raw::c_void;

    let owned_form: interop::OwnedForm = form.into();
    let metadata: *const FormMetadata = owned_form.as_ptr() as *const FormMetadata;
    let mut value_map: HashMap<String, String> = HashMap::new();

    if let Some(evaluator) = evaluator {
        let mut state = PreviewCallbackState {
            evaluator,
            last_text: std::ffi::CString::new("").unwrap(),
            last_success: None,
        };
        unsafe {
            interop_show_form(
                metadata,
                result_callback,
                std::ptr::from_mut::<HashMap<String, String>>(&mut value_map) as *mut c_void,
                Some(preview_callback),
                std::ptr::from_mut::<PreviewCallbackState<'_>>(&mut state) as *mut c_void,
            );
        }
        if !value_map.is_empty() {
            if let Some(result) = state.last_success.take() {
                value_map.extend(result.values);
            }
        }
    } else {
        unsafe {
            interop_show_form(
                metadata,
                result_callback,
                std::ptr::from_mut::<HashMap<String, String>>(&mut value_map) as *mut c_void,
                None,
                std::ptr::null_mut(),
            );
        }
    }

    value_map
}
