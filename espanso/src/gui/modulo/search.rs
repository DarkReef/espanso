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

use serde::{Deserialize, Serialize};
use std::{
    convert::TryInto,
    path::{Path, PathBuf},
};

use crate::{
    cli::search_stats::{self, SearchStatsSnapshot},
    gui::{SearchItem, SearchUI},
};

use super::manager::ModuloManager;

pub trait ModuloSearchUIOptionProvider {
    fn get_post_search_delay(&self) -> usize;
}

pub struct ModuloSearchUI<'a> {
    manager: &'a ModuloManager,
    option_provider: &'a dyn ModuloSearchUIOptionProvider,
    stats_dir: PathBuf,
}

impl<'a> ModuloSearchUI<'a> {
    pub fn new(
        manager: &'a ModuloManager,
        option_provider: &'a dyn ModuloSearchUIOptionProvider,
        stats_dir: &Path,
    ) -> Self {
        Self {
            manager,
            option_provider,
            stats_dir: stats_dir.to_path_buf(),
        }
    }
}

impl SearchUI for ModuloSearchUI<'_> {
    fn show(&self, items: &[SearchItem], hint: Option<&str>) -> anyhow::Result<Option<String>> {
        let snapshot = search_stats::snapshot(&self.stats_dir);
        let modulo_config = ModuloSearchConfig {
            title: "rEspanso",
            hint,
            items: convert_items(items, &snapshot),
        };

        let json_config = serde_json::to_string(&modulo_config)?;
        let output = self
            .manager
            .invoke(&["search", "-j", "-i", "-"], &json_config)?;
        let result: ModuloSearchResult = serde_json::from_str(&output)?;

        // Favorite changes are returned by the child search process when it
        // closes. Resolve its stable match id back to the trigger identifier
        // before persisting it; no replacement contents or patient text enter
        // the statistics database.
        for change in result.favorite_changes {
            if let Some(item) = items.iter().find(|item| item.id == change.id) {
                if let Some(trigger) = item.tag.as_deref() {
                    if let Err(err) =
                        search_stats::set_favorite(&self.stats_dir, trigger, change.favorite)
                    {
                        log::warn!("stats: unable to update favorite: {err}");
                    }
                }
            }
        }

        if let Some(selected_id) = result.selected.as_deref() {
            if let Some(item) = items.iter().find(|item| item.id == selected_id) {
                if let Some(trigger) = item.tag.as_deref() {
                    search_stats::record_search_selection(trigger);
                }
            }
        }

        let post_search_delay = self.option_provider.get_post_search_delay();
        if post_search_delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(
                post_search_delay.try_into().unwrap(),
            ));
        }

        Ok(result.selected)
    }
}

#[derive(Debug, Serialize)]
struct ModuloSearchConfig<'a> {
    title: &'a str,
    hint: Option<&'a str>,
    items: Vec<ModuloSearchItemConfig<'a>>,
}

#[derive(Debug, Serialize)]
struct ModuloSearchItemConfig<'a> {
    id: &'a str,
    label: &'a str,
    trigger: Option<&'a str>,
    search_terms: Vec<&'a str>,
    is_builtin: bool,
    usage_count: i64,
    favorite: bool,
}

#[derive(Debug, Deserialize)]
struct ModuloSearchResult {
    selected: Option<String>,
    #[serde(default)]
    favorite_changes: Vec<FavoriteChange>,
}

#[derive(Debug, Deserialize)]
struct FavoriteChange {
    id: String,
    favorite: bool,
}

fn convert_items<'a>(
    items: &'a [SearchItem],
    snapshot: &SearchStatsSnapshot,
) -> Vec<ModuloSearchItemConfig<'a>> {
    items
        .iter()
        .map(|item| {
            let trigger = item.tag.as_deref();
            let usage_count = trigger
                .and_then(|trigger| snapshot.monthly_counts.get(trigger).copied())
                .unwrap_or(0);
            let favorite = trigger
                .map(|trigger| snapshot.favorites.contains(trigger))
                .unwrap_or(false);

            ModuloSearchItemConfig {
                id: &item.id,
                label: &item.label,
                trigger,
                search_terms: if item.additional_search_terms.is_empty() {
                    vec![]
                } else {
                    item.additional_search_terms
                        .iter()
                        .map(String::as_str)
                        .collect()
                },
                is_builtin: item.is_builtin,
                usage_count,
                favorite,
            }
        })
        .collect()
}
