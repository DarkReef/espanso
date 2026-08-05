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

use espanso_engine::event::EventType;

use crate::cli::worker::builtin::generate_next_builtin_id;

use super::BuiltInMatch;

// CTRL+ALT+M is commonly reserved by other Windows applications and may cause
// RegisterHotKey to fail during startup. Keep the built-in action available on
// a less collision-prone combination instead of emitting an avoidable error.
pub const DEFAULT_SELECTION_MATCH_SHORTCUT: &str = "CTRL+ALT+SHIFT+M";

pub fn create_match_execute_selection() -> BuiltInMatch {
    BuiltInMatch {
        id: generate_next_builtin_id(),
        label: "Execute match from selected text",
        triggers: Vec::new(),
        hotkey: Some(DEFAULT_SELECTION_MATCH_SHORTCUT.to_owned()),
        action: |_| EventType::SelectionMatchRequested,
    }
}
