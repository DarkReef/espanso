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

use espanso_config::matches::{Match, MatchEffect};

use crate::cli::worker::{builtin::BuiltInMatch, context::Context};
use espanso_engine::{
    event::{
        internal::DetectedMatch,
        internal::{ImageRequestedEvent, RenderingRequestedEvent, TextFormat},
        ui::ShowTextEvent,
        EventType,
    },
    process::Multiplexer,
};

const DIALOG_DIRECTIVE: &str = "@dialog";
const DEFAULT_DIALOG_TITLE: &str = "rEspanso";

pub trait MatchProvider<'a> {
    fn get(&self, match_id: i32) -> Option<MatchResult<'a>>;
}

pub enum MatchResult<'a> {
    User(&'a Match),
    Builtin(&'a BuiltInMatch),
}

pub struct MultiplexAdapter<'a> {
    provider: &'a dyn MatchProvider<'a>,
    context: &'a dyn Context,
}

impl<'a> MultiplexAdapter<'a> {
    pub fn new(provider: &'a dyn MatchProvider<'a>, context: &'a dyn Context) -> Self {
        Self { provider, context }
    }
}

impl Multiplexer for MultiplexAdapter<'_> {
    fn convert(&self, detected_match: DetectedMatch) -> Option<EventType> {
        match self.provider.get(detected_match.id)? {
            MatchResult::User(m) => match &m.effect {
                MatchEffect::Text(effect) => {
                    if let Some(dialog) = parse_dialog_directive(&effect.replace) {
                        return Some(EventType::ShowText(ShowTextEvent {
                            title: dialog.title,
                            text: dialog.text,
                        }));
                    }

                    Some(EventType::RenderingRequested(RenderingRequestedEvent {
                        match_id: detected_match.id,
                        trigger: detected_match.trigger,
                        left_separator: detected_match.left_separator,
                        right_separator: detected_match.right_separator,
                        trigger_args: detected_match.args,
                        format: convert_format(&effect.format),
                    }))
                }
                MatchEffect::Image(effect) => {
                    Some(EventType::ImageRequested(ImageRequestedEvent {
                        match_id: detected_match.id,
                        image_path: effect.path.clone(),
                        trigger: detected_match.trigger,
                    }))
                }
                MatchEffect::None => None,
            },
            MatchResult::Builtin(m) => Some((m.action)(self.context)),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DialogDirective {
    title: String,
    text: String,
}

fn parse_dialog_directive(replace: &str) -> Option<DialogDirective> {
    let mut lines = replace.lines();
    let header = lines.next()?.trim();
    let suffix = header.strip_prefix(DIALOG_DIRECTIVE)?;

    let title = if suffix.is_empty() {
        DEFAULT_DIALOG_TITLE
    } else if let Some(title) = suffix.strip_prefix(':') {
        let title = title.trim();
        if title.is_empty() {
            DEFAULT_DIALOG_TITLE
        } else {
            title
        }
    } else {
        return None;
    };

    Some(DialogDirective {
        title: title.to_owned(),
        text: lines.collect::<Vec<_>>().join("\n"),
    })
}

fn convert_format(format: &espanso_config::matches::TextFormat) -> TextFormat {
    match format {
        espanso_config::matches::TextFormat::Plain => TextFormat::Plain,
        espanso_config::matches::TextFormat::Markdown => TextFormat::Markdown,
        espanso_config::matches::TextFormat::Html => TextFormat::Html,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dialog_with_custom_title() {
        assert_eq!(
            parse_dialog_directive("@dialog: Medical reminder\nCheck the patient data."),
            Some(DialogDirective {
                title: "Medical reminder".to_owned(),
                text: "Check the patient data.".to_owned(),
            })
        );
    }

    #[test]
    fn parses_dialog_with_default_title() {
        assert_eq!(
            parse_dialog_directive("@dialog\nLine one\nLine two"),
            Some(DialogDirective {
                title: DEFAULT_DIALOG_TITLE.to_owned(),
                text: "Line one\nLine two".to_owned(),
            })
        );
    }

    #[test]
    fn does_not_treat_regular_replacement_as_dialog() {
        assert_eq!(parse_dialog_directive("@dialogue is not a directive"), None);
        assert_eq!(parse_dialog_directive("ordinary replacement"), None);
    }
}
