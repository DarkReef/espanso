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

use super::super::Middleware;
use crate::event::{
    internal::{DetectedMatch, MatchesDetectedEvent},
    ui::ShowTextEvent,
    Event, EventType,
};

pub trait SelectedTextProvider {
    fn get_selected_text(&self) -> Option<String>;
}

pub trait SelectionMatchResolver {
    fn find_matches_from_selection(&self, selection: &str) -> Vec<DetectedMatch>;
}

pub struct SelectionMatchMiddleware<'a> {
    selected_text_provider: &'a dyn SelectedTextProvider,
    match_resolver: &'a dyn SelectionMatchResolver,
}

impl<'a> SelectionMatchMiddleware<'a> {
    pub fn new(
        selected_text_provider: &'a dyn SelectedTextProvider,
        match_resolver: &'a dyn SelectionMatchResolver,
    ) -> Self {
        Self {
            selected_text_provider,
            match_resolver,
        }
    }
}

impl Middleware for SelectionMatchMiddleware<'_> {
    fn name(&self) -> &'static str {
        "selection_match"
    }

    fn next(&self, event: Event, dispatch: &mut dyn FnMut(Event)) -> Event {
        if matches!(&event.etype, EventType::SelectionMatchRequested) {
            let selection = self
                .selected_text_provider
                .get_selected_text()
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty());

            let Some(selection) = selection else {
                return Event::caused_by(
                    event.source_id,
                    EventType::ShowText(ShowTextEvent {
                        title: "rEspanso".to_owned(),
                        text: "Не удалось получить выделенный текст.".to_owned(),
                    }),
                );
            };

            let matches = self.match_resolver.find_matches_from_selection(&selection);
            if matches.is_empty() {
                return Event::caused_by(
                    event.source_id,
                    EventType::ShowText(ShowTextEvent {
                        title: "rEspanso".to_owned(),
                        text: "Для выделенного текста не найден подходящий match.".to_owned(),
                    }),
                );
            }

            dispatch(Event::caused_by(
                event.source_id,
                EventType::MatchesDetected(MatchesDetectedEvent {
                    matches,
                    is_search: false,
                }),
            ));

            return Event::caused_by(event.source_id, EventType::NOOP);
        }

        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticSelectionProvider(Option<&'static str>);

    impl SelectedTextProvider for StaticSelectionProvider {
        fn get_selected_text(&self) -> Option<String> {
            self.0.map(str::to_owned)
        }
    }

    struct StaticResolver(Vec<DetectedMatch>);

    impl SelectionMatchResolver for StaticResolver {
        fn find_matches_from_selection(&self, _: &str) -> Vec<DetectedMatch> {
            self.0.clone()
        }
    }

    #[test]
    fn dispatches_matches_for_selected_text() {
        let provider = StaticSelectionProvider(Some("  I10  "));
        let resolver = StaticResolver(vec![DetectedMatch {
            id: 42,
            ..Default::default()
        }]);
        let middleware = SelectionMatchMiddleware::new(&provider, &resolver);
        let mut dispatched = Vec::new();

        let output = middleware.next(
            Event::caused_by(7, EventType::SelectionMatchRequested),
            &mut |event| dispatched.push(event),
        );

        assert!(matches!(output.etype, EventType::NOOP));
        assert_eq!(dispatched.len(), 1);
        match &dispatched[0].etype {
            EventType::MatchesDetected(event) => {
                assert_eq!(event.matches.len(), 1);
                assert_eq!(event.matches[0].id, 42);
            }
            _ => panic!("expected MatchesDetected event"),
        }
    }

    #[test]
    fn shows_error_when_selection_is_missing() {
        let provider = StaticSelectionProvider(None);
        let resolver = StaticResolver(Vec::new());
        let middleware = SelectionMatchMiddleware::new(&provider, &resolver);

        let output = middleware.next(
            Event::caused_by(7, EventType::SelectionMatchRequested),
            &mut |_| {},
        );

        match output.etype {
            EventType::ShowText(event) => {
                assert_eq!(event.text, "Не удалось получить выделенный текст.");
            }
            _ => panic!("expected ShowText event"),
        }
    }

    #[test]
    fn shows_error_when_no_match_is_found() {
        let provider = StaticSelectionProvider(Some("unknown"));
        let resolver = StaticResolver(Vec::new());
        let middleware = SelectionMatchMiddleware::new(&provider, &resolver);

        let output = middleware.next(
            Event::caused_by(7, EventType::SelectionMatchRequested),
            &mut |_| {},
        );

        match output.etype {
            EventType::ShowText(event) => {
                assert_eq!(
                    event.text,
                    "Для выделенного текста не найден подходящий match."
                );
            }
            _ => panic!("expected ShowText event"),
        }
    }
}
