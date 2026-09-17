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

use crate::Injector;

use anyhow::{bail, ensure, Result};
use log::{debug, error, info, warn};
use std::{
    ptr,
    sync::atomic::{AtomicU64, Ordering},
};

mod default;
mod ffi;
mod xdotool;

// The renderer can temporarily move X11 focus to a form/modulo window. Keep the
// window that owned the trigger immediately before rendering and consume it on
// the first post-render injection. This makes focus pinning one-shot, so an old
// expansion can never redirect a later trigger typed in another application.
static PINNED_TARGET_WINDOW: AtomicU64 = AtomicU64::new(0);

const REVERT_TO_PARENT: i32 = 2;
const CURRENT_TIME: ffi::Time = 0;

pub(crate) fn pin_target_window() -> Option<ffi::Window> {
    unsafe {
        let display = ffi::XOpenDisplay(ptr::null());
        if display.is_null() {
            warn!("[rESP-FOCUS] unable to pin target: XOpenDisplay failed");
            PINNED_TARGET_WINDOW.store(0, Ordering::SeqCst);
            return None;
        }

        let mut window: ffi::Window = 0;
        let mut revert_to = 0;
        ffi::XGetInputFocus(display, &mut window, &mut revert_to);
        ffi::XCloseDisplay(display);

        // 0=None and 1=PointerRoot are X11 sentinel values, not useful text
        // targets. Clear an older pin rather than accidentally reusing it.
        if window <= 1 {
            PINNED_TARGET_WINDOW.store(0, Ordering::SeqCst);
            warn!("[rESP-FOCUS] unable to pin usable target: window={}", window);
            None
        } else {
            PINNED_TARGET_WINDOW.store(window, Ordering::SeqCst);
            info!("[rESP-FOCUS] pinned target window={}", window);
            Some(window)
        }
    }
}

fn restore_pinned_target_window() {
    // One-shot by design: trigger compensation happens before renderer pinning,
    // while the first actual text/paste operation after rendering consumes it.
    let target = PINNED_TARGET_WINDOW.swap(0, Ordering::SeqCst);
    if target <= 1 {
        return;
    }

    unsafe {
        let display = ffi::XOpenDisplay(ptr::null());
        if display.is_null() {
            warn!(
                "[rESP-FOCUS] unable to restore target window={}: XOpenDisplay failed",
                target
            );
            return;
        }

        let mut current: ffi::Window = 0;
        let mut revert_to = 0;
        ffi::XGetInputFocus(display, &mut current, &mut revert_to);

        if current != target {
            info!(
                "[rESP-FOCUS] restoring focus current={} target={}",
                current, target
            );
            ffi::XSetInputFocus(display, target, REVERT_TO_PARENT, CURRENT_TIME);
            ffi::XSync(display, 0);

            let mut restored: ffi::Window = 0;
            ffi::XGetInputFocus(display, &mut restored, &mut revert_to);
            if restored != target {
                warn!(
                    "[rESP-FOCUS] focus restore verification failed: target={} actual={}",
                    target, restored
                );
            } else {
                info!("[rESP-FOCUS] focus restored target={}", target);
            }
        } else {
            debug!("[rESP-FOCUS] target already focused window={}", target);
        }

        ffi::XCloseDisplay(display);
    }
}

pub struct X11ProxyInjector {
    default_injector: Option<default::X11DefaultInjector>,
    xdotool_injector: Option<xdotool::X11XDOToolInjector>,
}

impl X11ProxyInjector {
    pub fn new() -> Result<Self> {
        // The native X11 injector builds its reverse keyboard map through
        // XOpenIM/XCreateIC/Xutf8LookupString. On some hardened Astra/KDE X11
        // installations that path can terminate the process inside Xlib/XIM
        // before Rust can return an error. The portable Astra build therefore
        // defaults to the simpler libxdo backend. Developers can opt back into
        // the historical behaviour with RESPANSO_X11_INJECTOR=auto.
        let requested_mode = std::env::var("RESPANSO_X11_INJECTOR")
            .unwrap_or_else(|_| "xdotool".to_string())
            .to_ascii_lowercase();
        let try_default = matches!(requested_mode.as_str(), "auto" | "native" | "default");

        info!(
            "[rESP-DIAG] X11ProxyInjector init begin: requested_mode={}",
            requested_mode
        );

        let default_injector = if try_default {
            info!("[rESP-DIAG] X11DefaultInjector init begin");
            match default::X11DefaultInjector::new() {
                Ok(injector) => {
                    info!("[rESP-DIAG] X11DefaultInjector init success");
                    Some(injector)
                }
                Err(err) => {
                    error!("X11DefaultInjector could not be initialized: {:?}", err);
                    warn!("falling back to xdotool injector");
                    None
                }
            }
        } else {
            info!("[rESP-DIAG] X11DefaultInjector skipped; using xdotool-safe mode");
            None
        };

        info!("[rESP-DIAG] X11XDOToolInjector init begin");
        let xdotool_injector = match xdotool::X11XDOToolInjector::new() {
            Ok(injector) => {
                info!("[rESP-DIAG] X11XDOToolInjector init success");
                Some(injector)
            }
            Err(err) => {
                error!("X11XDOToolInjector could not be initialized: {:?}", err);
                None
            }
        };

        if default_injector.is_none() && xdotool_injector.is_none() {
            bail!("unable to initialize injectors, neither the default or xdotool fallback could be initialized");
        }

        info!(
            "[rESP-DIAG] X11ProxyInjector init complete: default={}, xdotool={}",
            default_injector.is_some(),
            xdotool_injector.is_some()
        );

        Ok(X11ProxyInjector {
            default_injector,
            xdotool_injector,
        })
    }

    fn get_active_injector(&self, options: &crate::InjectionOptions) -> Result<&dyn Injector> {
        ensure!(
            self.default_injector.is_some() || self.xdotool_injector.is_some(),
            "unable to get active injector, neither default or xdotool fallback are available."
        );

        if options.x11_use_xdotool_fallback {
            if let Some(xdotool_injector) = self.xdotool_injector.as_ref() {
                return Ok(xdotool_injector);
            } else if let Some(default_injector) = self.default_injector.as_ref() {
                return Ok(default_injector);
            }
        } else if let Some(default_injector) = self.default_injector.as_ref() {
            return Ok(default_injector);
        } else if let Some(xdotool_injector) = self.xdotool_injector.as_ref() {
            return Ok(xdotool_injector);
        }

        unreachable!()
    }
}

impl Injector for X11ProxyInjector {
    fn send_string(&self, string: &str, options: crate::InjectionOptions) -> Result<()> {
        restore_pinned_target_window();
        self.get_active_injector(&options)?.send_string(string, options)
    }

    fn send_keys(&self, keys: &[crate::keys::Key], options: crate::InjectionOptions) -> Result<()> {
        restore_pinned_target_window();
        self.get_active_injector(&options)?.send_keys(keys, options)
    }

    fn send_key_combination(
        &self,
        keys: &[crate::keys::Key],
        options: crate::InjectionOptions,
    ) -> Result<()> {
        // Ctrl+C is used by the selected-text provider to copy the selection
        // from the application that is focused *now*. It must never consume a
        // renderer focus pin left by a previous form/expansion, otherwise the
        // copy is sent to the wrong window and Ctrl+Alt+M/Alt+L cannot obtain
        // the selected text. Clear a stale pin and keep the current X11 focus.
        if matches!(keys, [crate::keys::Key::Control, crate::keys::Key::C]) {
            let stale_target = PINNED_TARGET_WINDOW.swap(0, Ordering::SeqCst);
            if stale_target > 1 {
                debug!(
                    "[rESP-FOCUS] cleared stale target={} before selection Ctrl+C",
                    stale_target
                );
            }
        } else {
            restore_pinned_target_window();
        }

        self.get_active_injector(&options)?
            .send_key_combination(keys, options)
    }
}
