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

use crate::warn_eprintln;
use espanso_modulo::wizard::DetectedOS;

pub fn is_wrong_edition() -> (bool, DetectedOS) {
    if !cfg!(target_os = "linux") {
        return (false, DetectedOS::Unknown);
    }

    match get_session_type().as_deref() {
        Some("x11") if cfg!(feature = "wayland") => return (true, DetectedOS::X11),
        Some("wayland") if !cfg!(feature = "wayland") => return (true, DetectedOS::Wayland),
        None => {
            warn_eprintln!("could not automatically determine the session type (X11/Wayland), so make sure you have the correct espanso version!");
        }
        _ => {}
    }

    (false, DetectedOS::Unknown)
}

fn get_session_type() -> Option<String> {
    if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
        if session == "x11" || session == "wayland" {
            return Some(session);
        }
    }
    // Do not block portable startup on logind (often absent in Fly sessions).
    if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty()) {
        return Some("wayland".to_string());
    }
    if std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty()) {
        return Some("x11".to_string());
    }
    None
}
