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

use std::{
    convert::TryInto,
    path::PathBuf,
    time::{Duration, Instant},
};

use espanso_clipboard::{Clipboard, ClipboardOperationOptions};
use espanso_inject::{keys::Key, InjectionOptions, Injector};
use log::{debug, error};

use espanso_engine::{
    dispatch::HtmlInjector,
    dispatch::{ImageInjector, TextInjector},
    process::SelectedTextProvider,
};

const SELECTION_COPY_TIMEOUT: Duration = Duration::from_millis(800);
const SELECTION_COPY_POLL_INTERVAL: Duration = Duration::from_millis(20);
const SELECTION_RESTORE_DELAY: Duration = Duration::from_millis(30);

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    #[link_name = "GetClipboardSequenceNumber"]
    fn get_clipboard_sequence_number() -> u32;
}

#[cfg(target_os = "windows")]
fn clipboard_sequence_number() -> u32 {
    // SAFETY: GetClipboardSequenceNumber has no parameters and does not retain pointers.
    unsafe { get_clipboard_sequence_number() }
}

#[cfg(not(target_os = "windows"))]
fn clipboard_sequence_number() -> u32 {
    0
}

pub trait ClipboardParamsProvider {
    fn get(&self) -> ClipboardParams;
}

pub struct ClipboardParams {
    pub pre_paste_delay: usize,
    pub paste_shortcut_event_delay: usize,
    pub paste_shortcut: Option<String>,
    pub disable_x11_fast_inject: bool,
    pub restore_clipboard: bool,
    pub restore_clipboard_delay: usize,
    pub x11_use_xclip_backend: bool,
    pub x11_use_xdotool_backend: bool,
}

pub struct ClipboardInjectorAdapter<'a> {
    injector: &'a dyn Injector,
    clipboard: &'a dyn Clipboard,
    params_provider: &'a dyn ClipboardParamsProvider,
}

impl<'a> ClipboardInjectorAdapter<'a> {
    pub fn new(
        injector: &'a dyn Injector,
        clipboard: &'a dyn Clipboard,
        params_provider: &'a dyn ClipboardParamsProvider,
    ) -> Self {
        Self {
            injector,
            clipboard,
            params_provider,
        }
    }

    fn send_paste_combination(&self) -> anyhow::Result<()> {
        let params = self.params_provider.get();

        std::thread::sleep(std::time::Duration::from_millis(
            params.pre_paste_delay.try_into().unwrap(),
        ));

        let mut custom_combination = None;
        if let Some(custom_shortcut) = params.paste_shortcut {
            if let Some(combination) = parse_combination(&custom_shortcut) {
                custom_combination = Some(combination);
            } else {
                error!("'{custom_shortcut}' is not a valid paste shortcut");
            }
        }

        let combination = if let Some(custom_combination) = custom_combination {
            custom_combination
        } else if cfg!(target_os = "macos") {
            vec![Key::Meta, Key::V]
        } else if cfg!(target_os = "linux") && cfg!(feature = "wayland") {
            // Because on Wayland we currently don't have app-specific configs (and therefore no patches)
            // we switch to the more supported SHIFT+INSERT combination
            // See: https://github.com/espanso/espanso/issues/899
            vec![Key::Shift, Key::Insert]
        } else {
            vec![Key::Control, Key::V]
        };

        self.injector.send_key_combination(
            &combination,
            InjectionOptions {
                delay: params.paste_shortcut_event_delay as i32,
                disable_fast_inject: params.disable_x11_fast_inject,
                x11_use_xdotool_fallback: params.x11_use_xdotool_backend,
                ..Default::default()
            },
        )?;

        Ok(())
    }

    fn send_copy_combination(&self) -> anyhow::Result<()> {
        let params = self.params_provider.get();
        let combination = if cfg!(target_os = "macos") {
            vec![Key::Meta, Key::C]
        } else {
            vec![Key::Control, Key::C]
        };

        self.injector.send_key_combination(
            &combination,
            InjectionOptions {
                delay: params.paste_shortcut_event_delay as i32,
                disable_fast_inject: params.disable_x11_fast_inject,
                x11_use_xdotool_fallback: params.x11_use_xdotool_backend,
                ..Default::default()
            },
        )?;

        Ok(())
    }

    fn restore_clipboard_guard(&self) -> Option<ClipboardRestoreGuard<'a>> {
        let params = self.params_provider.get();

        if params.restore_clipboard {
            Some(ClipboardRestoreGuard::lock(
                self.clipboard,
                params.restore_clipboard_delay.try_into().unwrap(),
                self.get_operation_options(),
            ))
        } else {
            None
        }
    }

    fn get_operation_options(&self) -> ClipboardOperationOptions {
        let params = self.params_provider.get();
        ClipboardOperationOptions {
            use_xclip_backend: params.x11_use_xclip_backend,
        }
    }

    fn wait_for_selected_text(
        &self,
        previous_text: Option<&str>,
        previous_sequence: u32,
        options: &ClipboardOperationOptions,
    ) -> Option<String> {
        let started_at = Instant::now();

        while started_at.elapsed() < SELECTION_COPY_TIMEOUT {
            let current_text = self.clipboard.get_text(options);
            let text_changed = current_text.as_deref() != previous_text;
            let sequence_changed =
                cfg!(target_os = "windows") && previous_sequence != clipboard_sequence_number();

            if !(text_changed || sequence_changed) {
                std::thread::sleep(SELECTION_COPY_POLL_INTERVAL);
                continue;
            }

            if let Some(text) = current_text {
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }

            std::thread::sleep(SELECTION_COPY_POLL_INTERVAL);
        }

        None
    }
}

impl SelectedTextProvider for ClipboardInjectorAdapter<'_> {
    fn get_selected_text(&self) -> Option<String> {
        let params = self.params_provider.get();
        let options = self.get_operation_options();
        let previous_text = self.clipboard.get_text(&options);
        let previous_sequence = clipboard_sequence_number();

        if let Err(error) = self.send_copy_combination() {
            error!("unable to copy selected text: {error}");
            return None;
        }

        let selected_text =
            self.wait_for_selected_text(previous_text.as_deref(), previous_sequence, &options);

        if !params.restore_clipboard {
            return selected_text;
        }

        if let Some(previous_text) = previous_text {
            std::thread::sleep(SELECTION_RESTORE_DELAY);
            if let Err(error) = self.clipboard.set_text(&previous_text, &options) {
                error!("unable to restore clipboard after reading selection: {error}");
            }
        }

        if selected_text.is_none() {
            debug!("selection copy timed out or returned an empty value");
        }

        selected_text
    }
}

impl TextInjector for ClipboardInjectorAdapter<'_> {
    fn name(&self) -> &'static str {
        "clipboard"
    }

    fn inject_text(&self, text: &str) -> anyhow::Result<()> {
        let _guard = self.restore_clipboard_guard();

        self.clipboard
            .set_text(text, &self.get_operation_options())?;

        self.send_paste_combination()?;

        Ok(())
    }
}

impl HtmlInjector for ClipboardInjectorAdapter<'_> {
    fn inject_html(&self, html: &str, fallback_text: &str) -> anyhow::Result<()> {
        let _guard = self.restore_clipboard_guard();

        self.clipboard
            .set_html(html, Some(fallback_text), &self.get_operation_options())?;

        self.send_paste_combination()?;

        Ok(())
    }
}

impl ImageInjector for ClipboardInjectorAdapter<'_> {
    fn inject_image(&self, image_path: &str) -> anyhow::Result<()> {
        let path = PathBuf::from(image_path);
        if !path.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "image can't be found in the given path",
            )
            .into());
        }

        let _guard = self.restore_clipboard_guard();

        self.clipboard
            .set_image(&path, &self.get_operation_options())?;

        self.send_paste_combination()?;

        Ok(())
    }
}

struct ClipboardRestoreGuard<'a> {
    clipboard: &'a dyn Clipboard,
    content: Option<String>,
    restore_delay: u64,
    clipboard_operation_options: ClipboardOperationOptions,
}

impl<'a> ClipboardRestoreGuard<'a> {
    pub fn lock(
        clipboard: &'a dyn Clipboard,
        restore_delay: u64,
        clipboard_operation_options: ClipboardOperationOptions,
    ) -> Self {
        let clipboard_content = clipboard.get_text(&clipboard_operation_options);

        Self {
            clipboard,
            content: clipboard_content,
            restore_delay,
            clipboard_operation_options,
        }
    }
}

impl Drop for ClipboardRestoreGuard<'_> {
    fn drop(&mut self) {
        if let Some(content) = self.content.take() {
            // Sometimes an expansion gets overwritten before pasting by the previous content
            // A delay is needed to mitigate the problem
            std::thread::sleep(std::time::Duration::from_millis(self.restore_delay));

            if let Err(error) = self
                .clipboard
                .set_text(&content, &self.clipboard_operation_options)
            {
                error!("unable to restore clipboard content after expansion: {error}");
            }
        }
    }
}

fn parse_combination(combination: &str) -> Option<Vec<Key>> {
    let tokens = combination.split('+');
    let mut keys: Vec<Key> = Vec::new();
    for token in tokens {
        keys.push(Key::parse(token)?);
    }

    Some(keys)
}
