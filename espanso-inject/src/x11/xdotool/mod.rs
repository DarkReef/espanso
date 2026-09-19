/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2022 Federico Terzi
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
    ffi::{CStr, CString},
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering},
    time::{Duration, Instant},
};

use crate::Injector;
use anyhow::{bail, ensure, Context, Result};
use log::{debug, info, warn};

mod ffi;
use self::ffi::{fast_send_keysequence_window, xdo_send_keysequence_window, xdo_t, CURRENTWINDOW};

use super::ffi::{
    Display, Window, XGetInputFocus, XKeycodeToKeysym, XKeysymToKeycode, XKeysymToString,
    XQueryKeymap, XTestFakeKeyEvent,
};

const XDO_SUCCESS: libc::c_int = 0;
const KEY_POLL_INTERVAL: Duration = Duration::from_millis(2);
const MODIFIER_KEYSYMS: &[u64] = &[
    0xFFE1, // Shift_L
    0xFFE2, // Shift_R
    0xFFE3, // Control_L
    0xFFE4, // Control_R
    0xFFE7, // Meta_L
    0xFFE8, // Meta_R
    0xFFE9, // Alt_L
    0xFFEA, // Alt_R
    0xFFEB, // Super_L
    0xFFEC, // Super_R
];

fn checked_delay_micros(options: &crate::InjectionOptions) -> Result<libc::c_uint> {
    let delay_ms = i64::from(options.delay.max(0));
    let delay_micros = delay_ms
        .checked_mul(1_000)
        .context("X11 injection delay overflow")?;
    delay_micros
        .try_into()
        .context("X11 injection delay does not fit useconds_t")
}

fn check_xdo_status(operation: &str, status: libc::c_int) -> Result<()> {
    ensure!(
        status == XDO_SUCCESS,
        "{operation} failed with libxdo status {status}"
    );
    Ok(())
}

pub struct X11XDOToolInjector {
    xdo: AtomicPtr<xdo_t>,
    fast_failures: AtomicU32,
    fast_disabled: AtomicBool,
}

impl X11XDOToolInjector {
    pub fn new() -> Result<Self> {
        let xdo = unsafe { ffi::xdo_new(ptr::null()) };
        if xdo.is_null() {
            bail!("unable to initialize xdo_t instance");
        }

        debug!("initialized xdo_t object");

        Ok(Self {
            xdo: AtomicPtr::new(xdo),
            fast_failures: AtomicU32::new(0),
            fast_disabled: AtomicBool::new(false),
        })
    }

    fn xdo(&self) -> *mut xdo_t {
        self.xdo.load(Ordering::SeqCst)
    }

    fn display(&self) -> *mut Display {
        unsafe { (*self.xdo()).xdpy }
    }

    fn reinitialize_backend(&self) -> Result<()> {
        let replacement = unsafe { ffi::xdo_new(ptr::null()) };
        if replacement.is_null() {
            bail!("unable to reinitialize xdo_t instance");
        }
        let old = self.xdo.swap(replacement, Ordering::SeqCst);
        if !old.is_null() {
            unsafe { ffi::xdo_free(old) };
        }
        info!("[rESP-INJECT-HEAL] libxdo backend reinitialized");
        Ok(())
    }

    fn effective_disable_fast(&self, options: &crate::InjectionOptions) -> bool {
        options.disable_fast_inject
            || (options.x11_circuit_breaker && self.fast_disabled.load(Ordering::SeqCst))
    }

    fn finish_backend_result(
        &self,
        mode: &str,
        result: Result<()>,
        options: crate::InjectionOptions,
    ) -> Result<()> {
        match result {
            Ok(()) => {
                if mode == "fast" {
                    self.fast_failures.store(0, Ordering::SeqCst);
                }
                Ok(())
            }
            Err(error) => {
                if mode == "fast" && options.x11_circuit_breaker {
                    let failures = self.fast_failures.fetch_add(1, Ordering::SeqCst) + 1;
                    if failures >= options.x11_fast_failure_threshold.max(1) {
                        self.fast_disabled.store(true, Ordering::SeqCst);
                        warn!(
                            "[rESP-INJECT-BREAKER] disabling fast X11 injection after {} consecutive failure(s); subsequent operations use XTest",
                            failures
                        );
                    }
                }
                if options.x11_reinitialize_on_failure {
                    if let Err(reinit_error) = self.reinitialize_backend() {
                        warn!("[rESP-INJECT-HEAL] backend reinitialize failed: {reinit_error:?}");
                    }
                }
                Err(error)
            }
        }
    }

    fn pressed_modifier_keycodes(&self) -> Vec<u8> {
        let display = self.display();
        if display.is_null() {
            return Vec::new();
        }
        let mut keymap = [0u8; 32];
        unsafe { XQueryKeymap(display, keymap.as_mut_ptr()) };

        MODIFIER_KEYSYMS
            .iter()
            .filter_map(|keysym| {
                let code = unsafe { XKeysymToKeycode(display, *keysym) };
                if code == 0 {
                    return None;
                }
                let index = usize::from(code / 8);
                let mask = 1u8 << (code % 8);
                if keymap[index] & mask != 0 { Some(code) } else { None }
            })
            .collect()
    }

    fn wait_for_modifiers_release(&self, timeout_ms: u32) -> Result<()> {
        let started = Instant::now();
        let timeout = Duration::from_millis(u64::from(timeout_ms));
        loop {
            let pressed = self.pressed_modifier_keycodes();
            if pressed.is_empty() {
                return Ok(());
            }
            if started.elapsed() >= timeout {
                bail!(
                    "physical X11 modifier keys are still pressed after {} ms (keycodes={pressed:?})",
                    timeout_ms
                );
            }
            std::thread::sleep(KEY_POLL_INTERVAL);
        }
    }

    fn prepare_keyboard_state(
        &self,
        options: &crate::InjectionOptions,
        disable_fast: bool,
    ) -> Result<()> {
        if options.x11_legacy_release_all_keys {
            if disable_fast {
                self.xfake_release_all_keys();
            } else {
                self.fast_release_all_keys();
            }
            return Ok(());
        }

        if options.x11_wait_for_modifiers {
            self.wait_for_modifiers_release(options.x11_modifier_release_timeout_ms)?;
        }
        Ok(())
    }

    fn xfake_release_all_keys(&self) {
        let mut keys: [u8; 32] = [0; 32];
        unsafe {
            XQueryKeymap((*self.xdo()).xdpy, keys.as_mut_ptr());
        }

        #[allow(clippy::needless_range_loop)]
        for i in 0..32 {
            // Only those that are pressed should be changed
            if keys[i] != 0 {
                for k in 0..8 {
                    if (keys[i] & (1 << k)) != 0 {
                        let key_code = i * 8 + k;
                        unsafe {
                            XTestFakeKeyEvent((*self.xdo()).xdpy, key_code as u32, 0, 0);
                        }
                    }
                }
            }
        }
    }

    fn get_focused_window(&self) -> Window {
        let mut focused_window: Window = 0;
        let mut revert_to = 0;
        unsafe {
            XGetInputFocus((*self.xdo()).xdpy, &mut focused_window, &mut revert_to);
        }
        focused_window
    }

    fn xfake_send_string(
        &self,
        string: &str,
        options: crate::InjectionOptions,
    ) -> anyhow::Result<()> {
        // It may happen that when an expansion is triggered, some keys are still pressed.
        // This causes a problem if the expanded match contains that character, as the injection
        // will not be able to register that keypress (as it is already pressed).
        // To solve the problem, before an expansion we get which keys are currently pressed
        // and inject a key_release event so that they can be further registered.
        let c_string = CString::new(string).context("unable to create CString")?;
        let delay = checked_delay_micros(&options)?;

        let status = unsafe {
            ffi::xdo_enter_text_window(
                self.xdo(),
                CURRENTWINDOW,
                c_string.as_ptr(),
                delay,
            )
        };
        check_xdo_status("xdo_enter_text_window", status)?;

        Ok(())
    }

    fn fast_release_all_keys(&self) {
        let mut keys: [u8; 32] = [0; 32];
        unsafe {
            XQueryKeymap((*self.xdo()).xdpy, keys.as_mut_ptr());
        }

        let focused_window = self.get_focused_window();

        #[allow(clippy::needless_range_loop)]
        for i in 0..32 {
            // Only those that are pressed should be changed
            if keys[i] != 0 {
                for k in 0..8 {
                    if (keys[i] & (1 << k)) != 0 {
                        let key_code = i * 8 + k;
                        unsafe {
                            ffi::fast_send_event(
                                self.xdo(),
                                focused_window,
                                key_code.try_into().unwrap(),
                                0,
                            );
                        }
                    }
                }
            }
        }
    }

    fn fast_send_string(
        &self,
        string: &str,
        options: crate::InjectionOptions,
    ) -> anyhow::Result<()> {
        // It may happen that when an expansion is triggered, some keys are still pressed.
        // This causes a problem if the expanded match contains that character, as the injection
        // will not be able to register that keypress (as it is already pressed).
        // To solve the problem, before an expansion we get which keys are currently pressed
        // and inject a key_release event so that they can be further registered.
        let c_string = CString::new(string).context("unable to create CString")?;
        let delay = checked_delay_micros(&options)?;

        let status = unsafe {
            ffi::fast_enter_text_window(
                self.xdo(),
                self.get_focused_window(),
                c_string.as_ptr(),
                delay,
            )
        };
        check_xdo_status("fast_enter_text_window", status)?;

        Ok(())
    }
}

impl Injector for X11XDOToolInjector {
    fn send_string(&self, string: &str, options: crate::InjectionOptions) -> anyhow::Result<()> {
        let disable_fast = self.effective_disable_fast(&options);
        let mode = if disable_fast { "xtest" } else { "fast" };
        self.prepare_keyboard_state(&options, disable_fast)?;
        debug!(
            "[rESP-INJECT] xdotool send_string begin mode={} bytes={} delay_ms={}",
            mode,
            string.len(),
            options.delay.max(0)
        );
        let result = if disable_fast {
            self.xfake_send_string(string, options)
        } else {
            self.fast_send_string(string, options)
        };
        let result = self.finish_backend_result(mode, result, options);
        if let Err(ref error) = result {
            log::error!("[rESP-INJECT] xdotool send_string failed: {:?}", error);
        } else {
            debug!("[rESP-INJECT] xdotool send_string complete mode={}", mode);
        }
        result
    }

    fn send_keys(
        &self,
        keys: &[crate::keys::Key],
        options: crate::InjectionOptions,
    ) -> anyhow::Result<()> {
        let disable_fast = self.effective_disable_fast(&options);
        let mode = if disable_fast { "xtest" } else { "fast" };
        self.prepare_keyboard_state(&options, disable_fast)?;
        debug!(
            "[rESP-INJECT] xdotool send_keys begin mode={} count={} delay_ms={}",
            mode,
            keys.len(),
            options.delay.max(0)
        );
        let display = self.display();
        let key_syms: Vec<String> = keys
            .iter()
            .filter_map(|key| unsafe { convert_key_to_keysym(display, key) })
            .collect();
        let delay = checked_delay_micros(&options)?;
        let mut result = Ok(());
        for key in key_syms {
            let c_str = CString::new(key).context("unable to generate CString")?;
            let status = unsafe {
                if disable_fast {
                    xdo_send_keysequence_window(self.xdo(), CURRENTWINDOW, c_str.as_ptr(), delay)
                } else {
                    fast_send_keysequence_window(
                        self.xdo(),
                        self.get_focused_window(),
                        c_str.as_ptr(),
                        delay,
                    )
                }
            };
            if let Err(error) = check_xdo_status(
                if disable_fast { "xdo_send_keysequence_window" } else { "fast_send_keysequence_window" },
                status,
            ) {
                result = Err(error);
                break;
            }
        }
        self.finish_backend_result(mode, result, options)
    }

    fn send_key_combination(
        &self,
        keys: &[crate::keys::Key],
        options: crate::InjectionOptions,
    ) -> anyhow::Result<()> {
        let disable_fast = self.effective_disable_fast(&options);
        let mode = if disable_fast { "xtest" } else { "fast" };
        self.prepare_keyboard_state(&options, disable_fast)?;
        let display = self.display();
        let key_syms: Vec<String> = keys
            .iter()
            .filter_map(|key| unsafe { convert_key_to_keysym(display, key) })
            .collect();
        let key_combination = key_syms.join("+");
        debug!(
            "[rESP-INJECT] xdotool send_key_combination begin mode={} keys={} delay_ms={}",
            mode,
            key_combination,
            options.delay.max(0)
        );
        let delay = checked_delay_micros(&options)?;
        let c_key_combination =
            CString::new(key_combination).context("unable to generate CString")?;
        let status = unsafe {
            if disable_fast {
                xdo_send_keysequence_window(
                    self.xdo(),
                    CURRENTWINDOW,
                    c_key_combination.as_ptr(),
                    delay,
                )
            } else {
                fast_send_keysequence_window(
                    self.xdo(),
                    self.get_focused_window(),
                    c_key_combination.as_ptr(),
                    delay,
                )
            }
        };
        let result = check_xdo_status(
            if disable_fast { "xdo_send_keysequence_window" } else { "fast_send_keysequence_window" },
            status,
        );
        self.finish_backend_result(mode, result, options)
    }
}

impl Drop for X11XDOToolInjector {
    fn drop(&mut self) {
        let xdo = self.xdo.swap(ptr::null_mut(), Ordering::SeqCst);
        if !xdo.is_null() {
            unsafe { ffi::xdo_free(xdo) }
        }
    }
}

fn convert_key_to_keysym(display: *mut Display, key: &crate::keys::Key) -> Option<String> {
    match key {
        crate::keys::Key::Alt => Some("Alt_L".to_string()),
        crate::keys::Key::CapsLock => Some("Caps_Lock".to_string()),
        crate::keys::Key::Control => Some("Control_L".to_string()),
        crate::keys::Key::Meta => Some("Meta_L".to_string()),
        crate::keys::Key::NumLock => Some("Num_Lock".to_string()),
        crate::keys::Key::Shift => Some("Shift_L".to_string()),
        crate::keys::Key::Enter => Some("Return".to_string()),
        crate::keys::Key::Tab => Some("Tab".to_string()),
        crate::keys::Key::Space => Some("space".to_string()),
        crate::keys::Key::ArrowDown => Some("downarrow".to_string()),
        crate::keys::Key::ArrowLeft => Some("leftarrow".to_string()),
        crate::keys::Key::ArrowRight => Some("rightarrow".to_string()),
        crate::keys::Key::ArrowUp => Some("uparrow".to_string()),
        crate::keys::Key::End => Some("End".to_string()),
        crate::keys::Key::Home => Some("Home".to_string()),
        crate::keys::Key::PageDown => Some("Page_Down".to_string()),
        crate::keys::Key::PageUp => Some("Page_Up".to_string()),
        crate::keys::Key::Escape => Some("Escape".to_string()),
        crate::keys::Key::Backspace => Some("BackSpace".to_string()),
        crate::keys::Key::Insert => Some("Insert".to_string()),
        crate::keys::Key::Delete => Some("Delete".to_string()),
        crate::keys::Key::F1 => Some("F1".to_string()),
        crate::keys::Key::F2 => Some("F2".to_string()),
        crate::keys::Key::F3 => Some("F3".to_string()),
        crate::keys::Key::F4 => Some("F4".to_string()),
        crate::keys::Key::F5 => Some("F5".to_string()),
        crate::keys::Key::F6 => Some("F6".to_string()),
        crate::keys::Key::F7 => Some("F7".to_string()),
        crate::keys::Key::F8 => Some("F8".to_string()),
        crate::keys::Key::F9 => Some("F9".to_string()),
        crate::keys::Key::F10 => Some("F10".to_string()),
        crate::keys::Key::F11 => Some("F11".to_string()),
        crate::keys::Key::F12 => Some("F12".to_string()),
        crate::keys::Key::F13 => Some("F13".to_string()),
        crate::keys::Key::F14 => Some("F14".to_string()),
        crate::keys::Key::F15 => Some("F15".to_string()),
        crate::keys::Key::F16 => Some("F16".to_string()),
        crate::keys::Key::F17 => Some("F17".to_string()),
        crate::keys::Key::F18 => Some("F18".to_string()),
        crate::keys::Key::F19 => Some("F19".to_string()),
        crate::keys::Key::F20 => Some("F20".to_string()),
        crate::keys::Key::A => Some("a".to_string()),
        crate::keys::Key::B => Some("b".to_string()),
        crate::keys::Key::C => Some("c".to_string()),
        crate::keys::Key::D => Some("d".to_string()),
        crate::keys::Key::E => Some("e".to_string()),
        crate::keys::Key::F => Some("f".to_string()),
        crate::keys::Key::G => Some("g".to_string()),
        crate::keys::Key::H => Some("h".to_string()),
        crate::keys::Key::I => Some("i".to_string()),
        crate::keys::Key::J => Some("j".to_string()),
        crate::keys::Key::K => Some("k".to_string()),
        crate::keys::Key::L => Some("l".to_string()),
        crate::keys::Key::M => Some("m".to_string()),
        crate::keys::Key::N => Some("n".to_string()),
        crate::keys::Key::O => Some("o".to_string()),
        crate::keys::Key::P => Some("p".to_string()),
        crate::keys::Key::Q => Some("q".to_string()),
        crate::keys::Key::R => Some("r".to_string()),
        crate::keys::Key::S => Some("s".to_string()),
        crate::keys::Key::T => Some("t".to_string()),
        crate::keys::Key::U => Some("u".to_string()),
        crate::keys::Key::V => Some("v".to_string()),
        crate::keys::Key::W => Some("w".to_string()),
        crate::keys::Key::X => Some("x".to_string()),
        crate::keys::Key::Y => Some("y".to_string()),
        crate::keys::Key::Z => Some("z".to_string()),
        crate::keys::Key::N0 => Some("0".to_string()),
        crate::keys::Key::N1 => Some("1".to_string()),
        crate::keys::Key::N2 => Some("2".to_string()),
        crate::keys::Key::N3 => Some("3".to_string()),
        crate::keys::Key::N4 => Some("4".to_string()),
        crate::keys::Key::N5 => Some("5".to_string()),
        crate::keys::Key::N6 => Some("6".to_string()),
        crate::keys::Key::N7 => Some("7".to_string()),
        crate::keys::Key::N8 => Some("8".to_string()),
        crate::keys::Key::N9 => Some("9".to_string()),
        crate::keys::Key::Numpad0 => Some("KP_0".to_string()),
        crate::keys::Key::Numpad1 => Some("KP_1".to_string()),
        crate::keys::Key::Numpad2 => Some("KP_2".to_string()),
        crate::keys::Key::Numpad3 => Some("KP_3".to_string()),
        crate::keys::Key::Numpad4 => Some("KP_4".to_string()),
        crate::keys::Key::Numpad5 => Some("KP_5".to_string()),
        crate::keys::Key::Numpad6 => Some("KP_6".to_string()),
        crate::keys::Key::Numpad7 => Some("KP_7".to_string()),
        crate::keys::Key::Numpad8 => Some("KP_8".to_string()),
        crate::keys::Key::Numpad9 => Some("KP_9".to_string()),
        crate::keys::Key::Raw(key_code) => unsafe {
            let key_sym = XKeycodeToKeysym(display, (*key_code).try_into().unwrap(), 0);
            let string = XKeysymToString(key_sym);
            if string.is_null() {
                None
            } else {
                let c_str = CStr::from_ptr(string);
                Some(c_str.to_string_lossy().to_string())
            }
        },
    }
}
