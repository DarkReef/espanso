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

pub mod clipboard_injector;
pub mod context_menu;
pub mod event_injector;
pub mod icon;
pub mod key_injector;
pub mod secure_input;
pub mod text_ui;

pub trait InjectParamsProvider {
    fn get(&self) -> InjectParams;
}

/// X11-specific runtime policy shared by event, key and clipboard injection.
///
/// Keeping this policy in one value avoids subtle drift where one injection path
/// receives a safety flag while another path silently keeps the old behaviour.
#[derive(Clone, Copy)]
pub struct X11InjectParams {
    pub disable_fast_inject: bool,
    pub use_xdotool_backend: bool,
    pub wait_for_modifiers: bool,
    pub modifier_release_timeout_ms: u32,
    pub focus_guard: bool,
    pub focus_retry_count: u32,
    pub focus_retry_delay_ms: u32,
    pub circuit_breaker: bool,
    pub fast_failure_threshold: u32,
    pub reinitialize_on_failure: bool,
    pub legacy_release_all_keys: bool,
}

impl X11InjectParams {
    pub fn apply_to(self, options: &mut espanso_inject::InjectionOptions) {
        options.disable_fast_inject = self.disable_fast_inject;
        options.x11_use_xdotool_fallback = self.use_xdotool_backend;
        options.x11_wait_for_modifiers = self.wait_for_modifiers;
        options.x11_modifier_release_timeout_ms = self.modifier_release_timeout_ms;
        options.x11_focus_guard = self.focus_guard;
        options.x11_focus_retry_count = self.focus_retry_count;
        options.x11_focus_retry_delay_ms = self.focus_retry_delay_ms;
        options.x11_circuit_breaker = self.circuit_breaker;
        options.x11_fast_failure_threshold = self.fast_failure_threshold;
        options.x11_reinitialize_on_failure = self.reinitialize_on_failure;
        options.x11_legacy_release_all_keys = self.legacy_release_all_keys;
    }
}

pub struct InjectParams {
    pub inject_delay: Option<usize>,
    pub key_delay: Option<usize>,
    pub evdev_modifier_delay: Option<usize>,
    pub x11: X11InjectParams,
}

impl InjectParams {
    pub fn options(&self, configured_delay: Option<usize>) -> espanso_inject::InjectionOptions {
        let mut options = espanso_inject::InjectionOptions::default();
        if let Some(delay) = configured_delay {
            options.delay = i32::try_from(delay).unwrap_or(i32::MAX);
        }
        if let Some(delay) = self.evdev_modifier_delay {
            options.evdev_modifier_delay = u32::try_from(delay).unwrap_or(u32::MAX);
        }
        self.x11.apply_to(&mut options);
        options
    }
}
