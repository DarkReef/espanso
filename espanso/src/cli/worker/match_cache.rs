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

use espanso_config::{
    config::ConfigStore,
    matches::{store::MatchStore, Match, MatchCause, MatchEffect},
};
use espanso_engine::event::internal::DetectedMatch;
use regex::Regex;

use super::{builtin::BuiltInMatch, engine::process::middleware::match_select::MatchSummary};

pub struct MatchCache<'a> {
    cache: HashMap<i32, &'a Match>,
}

impl<'a> MatchCache<'a> {
    pub fn load(config_store: &'a dyn ConfigStore, match_store: &'a dyn MatchStore) -> Self {
        let mut cache = HashMap::new();

        let paths = config_store.get_all_match_paths();
        let global_set = match_store.query(&paths.into_iter().collect::<Vec<_>>());

        for m in global_set.matches {
            cache.insert(m.id, m);
        }

        Self { cache }
    }

    fn ids(&self) -> Vec<i32> {
        self.cache.keys().copied().collect()
    }
}

impl<'a> super::engine::process::middleware::render::MatchProvider<'a> for MatchCache<'a> {
    fn matches(&self) -> Vec<&'a Match> {
        self.cache.values().copied().collect()
    }

    fn get(&self, id: i32) -> Option<&'a Match> {
        self.cache.get(&id).copied()
    }
}

impl espanso_engine::process::MatchInfoProvider for MatchCache<'_> {
    fn get_force_mode(
        &self,
        match_id: i32,
    ) -> Option<espanso_engine::event::effect::TextInjectMode> {
        let m = self.cache.get(&match_id)?;
        if let MatchEffect::Text(text_effect) = &m.effect {
            if let Some(force_mode) = &text_effect.force_mode {
                match force_mode {
                    espanso_config::matches::TextInjectMode::Keys => {
                        return Some(espanso_engine::event::effect::TextInjectMode::Keys)
                    }
                    espanso_config::matches::TextInjectMode::Clipboard => {
                        return Some(espanso_engine::event::effect::TextInjectMode::Clipboard)
                    }
                }
            }
        }

        None
    }
}

pub struct CombinedMatchCache<'a> {
    user_match_cache: &'a MatchCache<'a>,
    builtin_match_cache: HashMap<i32, &'a BuiltInMatch>,
}

pub enum MatchVariant<'a> {
    User(&'a Match),
    Builtin(&'a BuiltInMatch),
}

impl<'a> CombinedMatchCache<'a> {
    pub fn load(user_match_cache: &'a MatchCache<'a>, builtin_matches: &'a [BuiltInMatch]) -> Self {
        let mut builtin_match_cache = HashMap::new();

        for m in builtin_matches {
            builtin_match_cache.insert(m.id, m);
        }

        Self {
            user_match_cache,
            builtin_match_cache,
        }
    }

    pub fn get(&self, match_id: i32) -> Option<MatchVariant<'a>> {
        if let Some(user_match) = self.user_match_cache.cache.get(&match_id).copied() {
            return Some(MatchVariant::User(user_match));
        }

        if let Some(builtin_match) = self.builtin_match_cache.get(&match_id).copied() {
            return Some(MatchVariant::Builtin(builtin_match));
        }

        None
    }
}

impl<'a> super::engine::process::middleware::match_select::MatchProvider<'a>
    for CombinedMatchCache<'a>
{
    fn get_matches(&self, ids: &[i32]) -> Vec<MatchSummary<'a>> {
        ids.iter()
            .filter_map(|id| self.get(*id))
            .map(|m| match m {
                MatchVariant::User(m) => MatchSummary {
                    id: m.id,
                    label: m.description(),
                    tag: m.cause_description(),
                    additional_search_terms: m.search_terms(),
                    is_builtin: false,
                },
                MatchVariant::Builtin(m) => MatchSummary {
                    id: m.id,
                    label: m.label,
                    tag: m.triggers.first().map(String::as_ref),
                    additional_search_terms: vec![],
                    is_builtin: true,
                },
            })
            .collect()
    }
}

impl<'a> super::engine::process::middleware::multiplex::MatchProvider<'a>
    for CombinedMatchCache<'a>
{
    fn get(
        &self,
        match_id: i32,
    ) -> Option<super::engine::process::middleware::multiplex::MatchResult<'a>> {
        Some(match self.get(match_id)? {
            MatchVariant::User(m) => {
                super::engine::process::middleware::multiplex::MatchResult::User(m)
            }
            MatchVariant::Builtin(m) => {
                super::engine::process::middleware::multiplex::MatchResult::Builtin(m)
            }
        })
    }
}

impl espanso_engine::process::MatchProvider for CombinedMatchCache<'_> {
    fn get_all_matches_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.builtin_match_cache.keys().copied().collect();
        ids.extend(self.user_match_cache.ids());
        ids
    }
}

impl espanso_engine::process::MatchResolver for CombinedMatchCache<'_> {
    fn find_matches_from_trigger(&self, trigger: &str) -> Vec<DetectedMatch> {
        let user_matches: Vec<DetectedMatch> = self
            .user_match_cache
            .cache
            .values()
            .filter_map(|m| {
                if let MatchCause::Trigger(trigger_cause) = &m.cause {
                    if trigger_cause.triggers.iter().any(|t| t == trigger) {
                        Some(DetectedMatch {
                            id: m.id,
                            trigger: Some(trigger.to_string()),
                            ..Default::default()
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        let builtin_matches: Vec<DetectedMatch> = self
            .builtin_match_cache
            .values()
            .filter_map(|m| {
                if m.triggers.iter().any(|t| t == trigger) {
                    Some(DetectedMatch {
                        id: m.id,
                        trigger: Some(trigger.to_string()),
                        ..Default::default()
                    })
                } else {
                    None
                }
            })
            .collect();

        let mut matches = Vec::with_capacity(user_matches.len() + builtin_matches.len());
        matches.extend(user_matches);
        matches.extend(builtin_matches);

        matches
    }
}

impl espanso_engine::process::SelectionMatchResolver for CombinedMatchCache<'_> {
    fn find_matches_from_selection(&self, selection: &str) -> Vec<DetectedMatch> {
        let selection = selection.trim();
        let mut detected = Vec::new();

        for m in self.user_match_cache.cache.values() {
            match &m.cause {
                MatchCause::Trigger(cause) => {
                    let matched = cause.triggers.iter().any(|trigger| {
                        if cause.propagate_case {
                            trigger.to_lowercase() == selection.to_lowercase()
                        } else {
                            trigger == selection
                        }
                    });

                    if matched {
                        let mut args = HashMap::new();
                        args.insert("selection".to_owned(), selection.to_owned());
                        detected.push(DetectedMatch {
                            id: m.id,
                            trigger: None,
                            args,
                            ..Default::default()
                        });
                    }
                }
                MatchCause::Regex(cause) => {
                    let pattern = format!("^(?:{})$", cause.regex);
                    let Ok(regex) = Regex::new(&pattern) else {
                        continue;
                    };
                    let Some(captures) = regex.captures(selection) else {
                        continue;
                    };

                    let mut args = HashMap::new();
                    args.insert("selection".to_owned(), selection.to_owned());
                    for name in regex.capture_names().flatten() {
                        if let Some(value) = captures.name(name) {
                            args.insert(name.to_owned(), value.as_str().to_owned());
                        }
                    }

                    detected.push(DetectedMatch {
                        id: m.id,
                        trigger: None,
                        args,
                        ..Default::default()
                    });
                }
                MatchCause::None => {}
            }
        }

        detected
    }
}
