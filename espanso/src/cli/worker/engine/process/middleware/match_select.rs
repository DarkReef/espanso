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

use espanso_engine::process::{MatchSelection, MatchSelector};
use log::{error, warn};
use std::{path::PathBuf, sync::OnceLock};

use crate::gui::{SearchCategory, SearchItem, SearchUI};

const MAX_LABEL_LEN: usize = 100;
const MAX_ICD_LABEL_LEN: usize = 160;
const ICD_RESULT_PREFIX: &str = "icd:";

#[derive(Debug, Clone, PartialEq, Eq)]
struct IcdEntry {
    code: String,
    name: String,
}

static ICD_CATALOG: OnceLock<Vec<IcdEntry>> = OnceLock::new();

pub trait MatchProvider<'a> {
    fn get_matches(&self, ids: &[i32]) -> Vec<MatchSummary<'a>>;
}

pub struct MatchSummary<'a> {
    pub id: i32,
    pub label: &'a str,
    pub tag: Option<&'a str>,
    pub additional_search_terms: Vec<&'a str>,
    pub is_builtin: bool,
}

pub struct MatchSelectorAdapter<'a> {
    search_ui: &'a dyn SearchUI,
    match_provider: &'a dyn MatchProvider<'a>,
}

impl<'a> MatchSelectorAdapter<'a> {
    pub fn new(search_ui: &'a dyn SearchUI, match_provider: &'a dyn MatchProvider<'a>) -> Self {
        Self {
            search_ui,
            match_provider,
        }
    }
}

impl MatchSelector for MatchSelectorAdapter<'_> {
    fn select(&self, matches_ids: &[i32], is_search: bool) -> Option<MatchSelection> {
        let matches = self.match_provider.get_matches(matches_ids);
        let mut search_items: Vec<SearchItem> = matches
            .into_iter()
            .map(|m| {
                let clipped_label: String = m.label.chars().take(MAX_LABEL_LEN).collect();

                SearchItem {
                    id: m.id.to_string(),
                    label: clipped_label,
                    tag: m.tag.map(String::from),
                    additional_search_terms: m
                        .additional_search_terms
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    is_builtin: m.is_builtin,
                    category: SearchCategory::Triggers,
                }
            })
            .collect();

        if is_search {
            search_items.extend(icd_catalog().iter().map(icd_search_item));
        }

        let hint = if is_search {
            Some("Триггеры: поиск по содержимому или триггеру. Коды: поиск МКБ-10 по коду или названию. Ctrl+Tab — сменить вкладку.")
        } else {
            None
        };

        match self.search_ui.show(&search_items, hint) {
            Ok(Some(selected_id)) => selection_from_id(&selected_id),
            Ok(None) => None,
            Err(err) => {
                error!("SearchUI reported an error: {err}");
                None
            }
        }
    }
}

fn selection_from_id(selected_id: &str) -> Option<MatchSelection> {
    if let Some(code) = selected_id.strip_prefix(ICD_RESULT_PREFIX) {
        let code = code.trim();
        if !code.is_empty() {
            return Some(MatchSelection::Text(code.to_owned()));
        }
        error!("match selector received an empty ICD code from SearchUI");
        return None;
    }

    match selected_id.parse::<i32>() {
        Ok(id) => Some(MatchSelection::Match(id)),
        Err(err) => {
            error!("match selector received an invalid id from SearchUI: {err}");
            None
        }
    }
}

fn icd_search_item(entry: &IcdEntry) -> SearchItem {
    let name: String = entry.name.chars().take(MAX_ICD_LABEL_LEN).collect();
    SearchItem {
        id: format!("{ICD_RESULT_PREFIX}{}", entry.code),
        label: format!("{} — {name}", entry.code),
        tag: None,
        additional_search_terms: vec![entry.code.clone(), entry.name.clone()],
        is_builtin: false,
        category: SearchCategory::Codes,
    }
}

fn icd_catalog() -> &'static [IcdEntry] {
    ICD_CATALOG
        .get_or_init(|| match load_icd_catalog() {
            Ok(entries) => entries,
            Err(error) => {
                warn!("ICD-10 catalogue is unavailable: {error}");
                Vec::new()
            }
        })
        .as_slice()
}

fn load_icd_catalog() -> Result<Vec<IcdEntry>, String> {
    let path = icd_catalog_path().ok_or_else(|| {
        "data/icd10-ru.tsv was not found next to the rEspanso executable".to_owned()
    })?;
    let content =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let entries = parse_icd_tsv(&content);
    if entries.is_empty() {
        return Err(format!("{} contains no ICD-10 entries", path.display()));
    }
    Ok(entries)
}

fn icd_catalog_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RESPANSO_ICD10_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let executable = std::env::current_exe().ok()?;
    let root = executable.parent()?;
    [
        root.join("data/icd10-ru.tsv"),
        root.join("icd10-ru.tsv"),
        root.join("../../bundled/icd10/icd10-ru.tsv"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn parse_icd_tsv(content: &str) -> Vec<IcdEntry> {
    content
        .lines()
        .filter_map(|line| {
            let (code, name) = line.split_once('\t')?;
            let code = code.trim();
            let name = name.trim();
            if code.is_empty() || name.is_empty() {
                return None;
            }
            Some(IcdEntry {
                code: code.to_owned(),
                name: name.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod icd_tests {
    use super::*;

    #[test]
    fn parses_icd_snapshot_rows() {
        let entries = parse_icd_tsv(
            "I10\tЭссенциальная [первичная] гипертензия\nI48.0\tПароксизмальная форма фибрилляции предсердий\n",
        );
        assert_eq!(
            entries,
            vec![
                IcdEntry {
                    code: "I10".to_owned(),
                    name: "Эссенциальная [первичная] гипертензия".to_owned(),
                },
                IcdEntry {
                    code: "I48.0".to_owned(),
                    name: "Пароксизмальная форма фибрилляции предсердий".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn icd_selection_becomes_plain_text_injection() {
        assert_eq!(
            selection_from_id("icd:M42.1"),
            Some(MatchSelection::Text("M42.1".to_owned()))
        );
        assert_eq!(
            selection_from_id("123"),
            Some(MatchSelection::Match(123))
        );
    }

    #[test]
    fn icd_search_item_uses_codes_tab_and_code_as_tag() {
        let item = icd_search_item(&IcdEntry {
            code: "I10".to_owned(),
            name: "Эссенциальная гипертензия".to_owned(),
        });
        assert_eq!(item.id, "icd:I10");
        assert_eq!(item.label, "I10 — Эссенциальная гипертензия");
        assert!(item.tag.is_none());
        assert_eq!(item.category, SearchCategory::Codes);
    }
}
