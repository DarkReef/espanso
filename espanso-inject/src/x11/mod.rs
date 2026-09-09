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
use log::{error, info, warn};

mod default;
mod ffi;
mod xdotool;

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
            info!(
                "[rESP-DIAG] X11DefaultInjector skipped; using xdotool-safe mode"
            );
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
        if xdotool_injector.is_some() {
            // Espanso's patched "fast" libxdo path sends synthetic KeyPress /
            // KeyRelease events with XSendEvent. Hardened Astra/KDE clients can
            // ignore those events even though libxdo itself initializes fine.
            // The original libxdo path uses the XTEST extension instead and is
            // what the xdotool command-line utility normally relies on. Force
            // that path whenever this proxy selects xdotool.
            info!(
                "[rESP-INJECT] xdotool backend will use libxdo/XTest; fast XSendEvent disabled"
            );
        }

        Ok(X11ProxyInjector {
            default_injector,
            xdotool_injector,
        })
    }

    fn get_active_injector(
        &self,
        options: &crate::InjectionOptions,
    ) -> Result<(&dyn Injector, bool)> {
        ensure!(
            self.default_injector.is_some() || self.xdotool_injector.is_some(),
            "unable to get active injector, neither default or xdotool fallback are available."
        );

        if options.x11_use_xdotool_fallback {
            if let Some(xdotool_injector) = self.xdotool_injector.as_ref() {
                return Ok((xdotool_injector, true));
            } else if let Some(default_injector) = self.default_injector.as_ref() {
                return Ok((default_injector, false));
            }
        } else if let Some(default_injector) = self.default_injector.as_ref() {
            return Ok((default_injector, false));
        } else if let Some(xdotool_injector) = self.xdotool_injector.as_ref() {
            return Ok((xdotool_injector, true));
        }

        unreachable!()
    }

    fn prepare_options(
        &self,
        options: crate::InjectionOptions,
    ) -> Result<(&dyn Injector, crate::InjectionOptions)> {
        let (injector, is_xdotool) = self.get_active_injector(&options)?;
        let mut safe_options = options;
        if is_xdotool {
            safe_options.disable_fast_inject = true;
        }
        Ok((injector, safe_options))
    }
}

impl Injector for X11ProxyInjector {
    fn send_string(&self, string: &str, options: crate::InjectionOptions) -> Result<()> {
        let (injector, options) = self.prepare_options(options)?;
        info!(
            "[rESP-INJECT] send_string backend={} bytes={}",
            if options.disable_fast_inject { "xdotool-xtest" } else { "native" },
            string.len()
        );
        injector.send_string(string, options)
    }

    fn send_keys(&self, keys: &[crate::keys::Key], options: crate::InjectionOptions) -> Result<()> {
        let (injector, options) = self.prepare_options(options)?;
        info!(
            "[rESP-INJECT] send_keys backend={} count={}",
            if options.disable_fast_inject { "xdotool-xtest" } else { "native" },
            keys.len()
        );
        injector.send_keys(keys, options)
    }

    fn send_key_combination(
        &self,
        keys: &[crate::keys::Key],
        options: crate::InjectionOptions,
    ) -> Result<()> {
        let (injector, options) = self.prepare_options(options)?;
        info!(
            "[rESP-INJECT] send_key_combination backend={} count={}",
            if options.disable_fast_inject { "xdotool-xtest" } else { "native" },
            keys.len()
        );
        injector.send_key_combination(keys, options)
    }
}
