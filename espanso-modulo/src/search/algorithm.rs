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

use std::collections::HashSet;

use crate::sys::search::types::SearchItem;

type FilterCallback = dyn Fn(&str, &[SearchItem]) -> Vec<usize>;

const TAB_QUERY_PREFIX: &str = "__RESPANSO_TAB__:";

pub fn get_algorithm(name: &str, use_command_filter: bool) -> Box<FilterCallback> {
    let search_algorithm: Box<FilterCallback> = match name {
        "exact" => Box::new(exact_match),
        "iexact" => Box::new(case_insensitive_exact_match),
        "ikey" => Box::new(case_insensitive_keyword),
        _ => panic!("unknown search algorithm: {name}"),
    };

    let search_algorithm = if use_command_filter {
        command_filter(search_algorithm)
    } else {
        search_algorithm
    };

    category_filter(search_algorithm)
}

fn exact_match(query: &str, items: &[SearchItem]) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.label.contains(query)
                || item.trigger.as_deref().is_some_and(|t| t.contains(query))
                || item.search_terms.iter().any(|term| term.contains(query))
        })
        .map(|(i, _)| i)
        .collect()
}

fn case_insensitive_exact_match(query: &str, items: &[SearchItem]) -> Vec<usize> {
    let lowercase_query = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.label.to_lowercase().contains(&lowercase_query)
                || item
                    .trigger
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains(query))
                || item
                    .search_terms
                    .iter()
                    .any(|term| term.to_lowercase().contains(&lowercase_query))
        })
        .map(|(i, _)| i)
        .collect()
}

fn case_insensitive_keyword(query: &str, items: &[SearchItem]) -> Vec<usize> {
    let lowercase_query = query.to_lowercase();
    let keywords: Vec<&str> = lowercase_query.split_whitespace().collect();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            for keyword in &keywords {
                if !item.label.to_lowercase().contains(keyword)
                    && !item
                        .trigger
                        .as_deref()
                        .is_some_and(|t| t.to_lowercase().contains(keyword))
                    && !item
                        .search_terms
                        .iter()
                        .any(|term| term.to_lowercase().contains(keyword))
                {
                    return false;
                }
            }

            true
        })
        .map(|(i, _)| i)
        .collect()
}

fn category_filter(search_algorithm: Box<FilterCallback>) -> Box<FilterCallback> {
    Box::new(move |query, items| {
        let Some(scoped) = query.strip_prefix(TAB_QUERY_PREFIX) else {
            return search_algorithm(query, items);
        };
        let Some((category, inner_query)) = scoped.split_once(':') else {
            return search_algorithm(query, items);
        };

        search_algorithm(inner_query, items)
            .into_iter()
            .filter(|index| {
                items
                    .get(*index)
                    .is_some_and(|item| item.category == category)
            })
            .collect()
    })
}

fn command_filter(search_algorithm: Box<FilterCallback>) -> Box<FilterCallback> {
    Box::new(move |query, items| {
        let (valid_ids, trimmed_query) = if query.starts_with('>') {
            (
                items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| item.is_builtin)
                    .map(|(i, _)| i)
                    .collect::<HashSet<usize>>(),
                query.trim_start_matches('>'),
            )
        } else {
            (
                items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| !item.is_builtin)
                    .map(|(i, _)| i)
                    .collect::<HashSet<usize>>(),
                query,
            )
        };

        let results = search_algorithm(trimmed_query, items);

        results
            .into_iter()
            .filter(|id| valid_ids.contains(id))
            .collect()
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, category: &str, is_builtin: bool) -> SearchItem {
        SearchItem {
            id: label.to_owned(),
            label: label.to_owned(),
            trigger: None,
            search_terms: Vec::new(),
            is_builtin,
            category: category.to_owned(),
        }
    }

    #[test]
    fn scoped_query_only_returns_active_tab_items() {
        let items = vec![
            item("гипертензия trigger", "triggers", false),
            item("Эссенциальная гипертензия", "codes", false),
        ];
        let algorithm = get_algorithm("ikey", true);

        assert_eq!(
            algorithm("__RESPANSO_TAB__:codes:гипертензия", &items),
            vec![1]
        );
        assert_eq!(
            algorithm("__RESPANSO_TAB__:triggers:гипертензия", &items),
            vec![0]
        );
    }

    #[test]
    fn command_prefix_still_works_inside_trigger_tab() {
        let items = vec![
            item("ordinary", "triggers", false),
            item("restart respanso", "triggers", true),
            item("restart diagnosis", "codes", false),
        ];
        let algorithm = get_algorithm("ikey", true);

        assert_eq!(
            algorithm("__RESPANSO_TAB__:triggers:>restart", &items),
            vec![1]
        );
    }

    #[test]
    fn unscoped_search_keeps_legacy_behavior() {
        let items = vec![
            item("one", "triggers", false),
            item("two", "codes", false),
        ];
        let algorithm = get_algorithm("ikey", false);

        assert_eq!(algorithm("", &items), vec![0, 1]);
    }
}
