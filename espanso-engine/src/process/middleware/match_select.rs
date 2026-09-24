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

use log::{debug, error};

use super::super::Middleware;
use crate::{
    event::{
        effect::TextInjectRequest,
        internal::{DiscardBetweenEvent, MatchSelectedEvent},
        Event, EventType,
    },
    process::EventSequenceProvider,
};

pub trait MatchFilter {
    fn filter_active(&self, matches_ids: &[i32]) -> Vec<i32>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchSelection {
    Match(i32),
    Text(String),
}

pub trait MatchSelector {
    fn select(&self, matches_ids: &[i32], is_search: bool) -> Option<MatchSelection>;
}

pub struct MatchSelectMiddleware<'a> {
    match_filter: &'a dyn MatchFilter,
    match_selector: &'a dyn MatchSelector,
    event_sequence_provider: &'a dyn EventSequenceProvider,
}

impl<'a> MatchSelectMiddleware<'a> {
    pub fn new(
        match_filter: &'a dyn MatchFilter,
        match_selector: &'a dyn MatchSelector,
        event_sequence_provider: &'a dyn EventSequenceProvider,
    ) -> Self {
        Self {
            match_filter,
            match_selector,
            event_sequence_provider,
        }
    }
}

impl Middleware for MatchSelectMiddleware<'_> {
    fn name(&self) -> &'static str {
        "match_select"
    }

    fn next(&self, event: Event, dispatch: &mut dyn FnMut(Event)) -> Event {
        if let EventType::MatchesDetected(m_event) = event.etype {
            let matches_ids: Vec<i32> = m_event.matches.iter().map(|m| m.id).collect();

            // Find the matches that are actually valid in the current context.
            let valid_ids = self.match_filter.filter_active(&matches_ids);
            if valid_ids.is_empty() {
                return Event::caused_by(event.source_id, EventType::NOOP);
            }

            // Normal trigger collisions still bypass the dialog when there is
            // only one valid match. The explicit search palette is different:
            // even with one user match it must open because it also contains
            // non-match sources such as the ICD-10 code catalogue.
            if !m_event.is_search && valid_ids.len() == 1 {
                let selected_id = *valid_ids.first().expect("non-empty valid match list");
                let selected = m_event
                    .matches
                    .into_iter()
                    .find(|m| m.id == selected_id);
                return if let Some(chosen) = selected {
                    Event::caused_by(
                        event.source_id,
                        EventType::MatchSelected(MatchSelectedEvent { chosen }),
                    )
                } else {
                    error!("MatchSelectMiddleware could not find the correspondent match");
                    Event::caused_by(event.source_id, EventType::NOOP)
                };
            }

            let start_event_id = self.event_sequence_provider.get_next_id();
            let selection = self.match_selector.select(&valid_ids, m_event.is_search);

            let next_event = match selection {
                Some(MatchSelection::Match(selected_id)) => {
                    let selected = m_event
                        .matches
                        .into_iter()
                        .find(|m| m.id == selected_id);
                    if let Some(chosen) = selected {
                        Event::caused_by(
                            event.source_id,
                            EventType::MatchSelected(MatchSelectedEvent { chosen }),
                        )
                    } else {
                        error!("MatchSelectMiddleware could not find the correspondent match");
                        Event::caused_by(event.source_id, EventType::NOOP)
                    }
                }
                Some(MatchSelection::Text(text)) => Event::caused_by(
                    event.source_id,
                    EventType::TextInject(TextInjectRequest {
                        text,
                        force_mode: None,
                    }),
                ),
                None => {
                    debug!("MatchSelectMiddleware did not receive any match selection");
                    Event::caused_by(event.source_id, EventType::NOOP)
                }
            };

            let end_event_id = self.event_sequence_provider.get_next_id();

            // Prevent keyboard events generated while the palette is open from
            // being replayed after the palette closes.
            dispatch(Event::caused_by(
                event.source_id,
                EventType::DiscardBetween(DiscardBetweenEvent {
                    start_id: start_event_id,
                    end_id: end_event_id,
                }),
            ));

            return next_event;
        }

        event
    }
}
