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

use std::{collections::HashSet, sync::Arc};

use espanso_config::{
    config::{AppProperties, Config, ConfigStore, X11InjectorProfile},
    matches::store::{MatchSet, MatchStore},
};
use espanso_info::{AppInfo, AppInfoProvider};

use super::{
    builtin::is_builtin_match,
    engine::{
        dispatch::executor::X11InjectParams,
        process::middleware::render::extension::clipboard::ClipboardOperationOptionsProvider,
    },
};

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn x11_inject_params(config: &dyn Config) -> X11InjectParams {
    let safety = config.x11_safe_injector();
    X11InjectParams {
        // Safe deliberately routes control-key injection through XTest even if
        // the legacy fast path is enabled in the YAML.
        disable_fast_inject: config.disable_x11_fast_inject()
            || matches!(safety.profile, X11InjectorProfile::Safe),
        use_xdotool_backend: config.x11_use_xdotool_backend(),
        wait_for_modifiers: safety.wait_for_modifiers,
        modifier_release_timeout_ms: saturating_u32(safety.modifier_release_timeout_ms),
        focus_guard: safety.focus_guard,
        focus_retry_count: saturating_u32(safety.focus_retry_count),
        focus_retry_delay_ms: saturating_u32(safety.focus_retry_delay_ms),
        circuit_breaker: safety.circuit_breaker,
        fast_failure_threshold: saturating_u32(safety.fast_failure_threshold).max(1),
        reinitialize_on_failure: safety.reinitialize_on_failure,
        legacy_release_all_keys: safety.legacy_release_all_keys,
    }
}

pub struct ConfigManager<'a> {
    config_store: &'a dyn ConfigStore,
    match_store: &'a dyn MatchStore,
    app_info_provider: &'a dyn AppInfoProvider,
}

impl<'a> ConfigManager<'a> {
    pub fn new(
        config_store: &'a dyn ConfigStore,
        match_store: &'a dyn MatchStore,
        app_info_provider: &'a dyn AppInfoProvider,
    ) -> Self {
        Self {
            config_store,
            match_store,
            app_info_provider,
        }
    }

    pub fn active(&self) -> Arc<dyn Config> {
        let current_app = self.app_info_provider.get_info();
        let info = to_app_properties(&current_app);
        self.config_store.active(&info)
    }

    pub fn active_context(&'_ self) -> (Arc<dyn Config>, MatchSet<'_>) {
        let config = self.active();
        let match_paths = config.match_paths();
        (config.clone(), self.match_store.query(match_paths))
    }

    pub fn default(&self) -> Arc<dyn Config> {
        self.config_store.default()
    }
}

fn to_app_properties(info: &'_ AppInfo) -> AppProperties<'_> {
    AppProperties {
        title: info.title.as_deref(),
        class: info.class.as_deref(),
        exec: info.exec.as_deref(),
    }
}

impl espanso_engine::process::MatchFilter for ConfigManager<'_> {
    fn filter_active(&self, matches_ids: &[i32]) -> Vec<i32> {
        let ids_set: HashSet<i32> = matches_ids.iter().copied().collect::<HashSet<_>>();
        let (_, match_set) = self.active_context();

        let active_user_defined_matches: Vec<i32> = match_set
            .matches
            .iter()
            .filter(|m| ids_set.contains(&m.id))
            .map(|m| m.id)
            .collect();

        let builtin_matches: Vec<i32> = matches_ids
            .iter()
            .filter(|id| is_builtin_match(**id))
            .copied()
            .collect();

        let mut output = active_user_defined_matches;
        output.extend(builtin_matches);
        output
    }
}

impl<'a> super::engine::process::middleware::render::ConfigProvider<'a> for ConfigManager<'a> {
    fn configs(&'_ self) -> Vec<(Arc<dyn Config>, MatchSet<'_>)> {
        self.config_store
            .configs()
            .into_iter()
            .map(|config| {
                let match_set = self.match_store.query(config.match_paths());
                (config, match_set)
            })
            .collect()
    }

    fn active(&'_ self) -> (Arc<dyn Config>, MatchSet<'_>) {
        self.active_context()
    }
}

impl espanso_engine::dispatch::ModeProvider for ConfigManager<'_> {
    fn active_mode(&self) -> espanso_engine::dispatch::Mode {
        let config = self.active();
        match config.backend() {
            espanso_config::config::Backend::Inject => espanso_engine::dispatch::Mode::Event,
            espanso_config::config::Backend::Clipboard => espanso_engine::dispatch::Mode::Clipboard,
            espanso_config::config::Backend::Auto => {
                let x11_safe = config.x11_safe_injector();
                let clipboard_threshold = if cfg!(target_os = "linux") && !cfg!(feature = "wayland") {
                    config.clipboard_threshold().min(x11_safe.clipboard_threshold)
                } else {
                    config.clipboard_threshold()
                };
                espanso_engine::dispatch::Mode::Auto { clipboard_threshold }
            },
        }
    }
}

impl super::engine::dispatch::executor::clipboard_injector::ClipboardParamsProvider
    for ConfigManager<'_>
{
    fn get(&self) -> super::engine::dispatch::executor::clipboard_injector::ClipboardParams {
        let active = self.active();
        super::engine::dispatch::executor::clipboard_injector::ClipboardParams {
            pre_paste_delay: active.pre_paste_delay(),
            paste_shortcut_event_delay: active.paste_shortcut_event_delay(),
            paste_shortcut: active.paste_shortcut(),
            restore_clipboard: active.preserve_clipboard(),
            restore_clipboard_delay: active.restore_clipboard_delay(),
            x11_use_xclip_backend: active.x11_use_xclip_backend(),
            x11: x11_inject_params(active.as_ref()),
        }
    }
}

impl ClipboardOperationOptionsProvider for ConfigManager<'_> {
    fn get_operation_options(&self) -> espanso_clipboard::ClipboardOperationOptions {
        let active = self.active();
        espanso_clipboard::ClipboardOperationOptions {
            use_xclip_backend: active.x11_use_xclip_backend(),
        }
    }
}

impl super::engine::dispatch::executor::InjectParamsProvider for ConfigManager<'_> {
    fn get(&self) -> super::engine::dispatch::executor::InjectParams {
        let active = self.active();
        super::engine::dispatch::executor::InjectParams {
            inject_delay: active.inject_delay(),
            key_delay: active.key_delay(),
            evdev_modifier_delay: active.evdev_modifier_delay(),
            x11: x11_inject_params(active.as_ref()),
        }
    }
}

impl espanso_engine::process::MatcherMiddlewareConfigProvider for ConfigManager<'_> {
    fn max_history_size(&self) -> usize {
        self.default().backspace_limit()
    }
}

impl espanso_engine::process::UndoEnabledProvider for ConfigManager<'_> {
    fn is_undo_enabled(&self) -> bool {
        // Disable undo_backspace on Wayland for now as it's not stable
        if cfg!(feature = "wayland") {
            return false;
        }

        // Because we cannot filter out espanso-generated events when using the X11 record injection
        // method, we need to disable undo_backspace to avoid looping (espanso picks up its own
        // injections, causing the program to misbehave)
        if cfg!(target_os = "linux") {
            let active = self.active();
            let profile = active.x11_safe_injector().profile;
            if active.disable_x11_fast_inject()
                || matches!(
                    profile,
                    espanso_config::config::X11InjectorProfile::Safe
                        | espanso_config::config::X11InjectorProfile::Balanced
                )
            {
                return false;
            }
            return active.undo_backspace();
        }

        self.active().undo_backspace()
    }
}

impl espanso_engine::process::EnabledStatusProvider for ConfigManager<'_> {
    fn is_config_enabled(&self) -> bool {
        self.active().enable()
    }
}

impl crate::gui::modulo::form::ModuloFormUIOptionProvider for ConfigManager<'_> {
    fn get_post_form_delay(&self) -> usize {
        self.active().post_form_delay()
    }

    fn get_max_form_width(&self) -> usize {
        self.active().max_form_width()
    }

    fn get_max_form_height(&self) -> usize {
        self.active().max_form_height()
    }
}

impl crate::gui::modulo::search::ModuloSearchUIOptionProvider for ConfigManager<'_> {
    fn get_post_search_delay(&self) -> usize {
        self.active().post_search_delay()
    }
}

impl espanso_engine::process::AltCodeSynthEnabledProvider for ConfigManager<'_> {
    fn is_alt_code_synthesizer_enabled(&self) -> bool {
        self.active().emulate_alt_codes()
    }
}
