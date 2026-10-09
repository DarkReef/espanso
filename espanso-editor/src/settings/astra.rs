//! Match Studio controls for Astra/Linux X11 injector safety.
//!
//! Profile defaults live in `espanso-config`; this module only renders and
//! persists user overrides. Do not duplicate runtime preset values here.

use eframe::egui;
use espanso_config::config::{X11InjectorProfile, X11SafeInjectorConfig};

use super::{
    read_top_level_bool, read_top_level_string, read_top_level_usize, remove_top_level_keys,
    set_top_level_bool, set_top_level_scalar, validate_yaml,
};

const PROFILE_CHOICES: &[(X11InjectorProfile, &str)] = &[
    (X11InjectorProfile::Safe, "Safe"),
    (X11InjectorProfile::Balanced, "Balanced"),
    (X11InjectorProfile::Fast, "Fast"),
    (X11InjectorProfile::Legacy, "Legacy"),
];

const OVERRIDE_KEYS: &[&str] = &[
    "x11_wait_for_modifiers",
    "x11_modifier_release_timeout",
    "x11_focus_guard",
    "x11_focus_retry_count",
    "x11_focus_retry_delay",
    "x11_circuit_breaker",
    "x11_fast_failure_threshold",
    "x11_reinitialize_on_failure",
    "x11_safe_clipboard_threshold",
];

pub(super) fn draw(ui: &mut egui::Ui, text: &mut String, error: &mut Option<String>) {
    ui.heading("Astra / Linux X11 — устойчивость инжектора");
    ui.label(
        egui::RichText::new(
            "Параметры действуют только для X11. Для рабочей Astra рекомендуется Safe; Windows их игнорирует.",
        )
        .weak(),
    );

    let mut profile = read_top_level_string(text, "x11_injector_profile")
        .as_deref()
        .and_then(X11InjectorProfile::from_name)
        .unwrap_or(X11InjectorProfile::Balanced);

    ui.horizontal_wrapped(|ui| {
        ui.label("Профиль:");
        for &(candidate, title) in PROFILE_CHOICES {
            if ui.selectable_label(profile == candidate, title).clicked() && profile != candidate {
                profile = candidate;
                *text = set_top_level_scalar(text, "x11_injector_profile", candidate.name());
                *error = validate_yaml(text).err();
            }
        }
    });

    ui.label(
        egui::RichText::new(profile_description(profile))
            .small()
            .strong(),
    );

    let defaults = X11SafeInjectorConfig::for_profile(profile);
    egui::CollapsingHeader::new("Расширенные параметры AstraSafeInjector")
        .default_open(false)
        .show(ui, |ui| {
            bool_override(
                ui,
                text,
                "x11_wait_for_modifiers",
                defaults.wait_for_modifiers,
                "Ждать отпускания Ctrl/Alt/Shift/Meta перед инъекцией",
            );
            bool_override(
                ui,
                text,
                "x11_focus_guard",
                defaults.focus_guard,
                "Не печатать, если целевое окно не удалось вернуть в focus",
            );
            bool_override(
                ui,
                text,
                "x11_circuit_breaker",
                defaults.circuit_breaker,
                "Circuit breaker: отключать fast backend после ошибок",
            );
            bool_override(
                ui,
                text,
                "x11_reinitialize_on_failure",
                defaults.reinitialize_on_failure,
                "Переинициализировать libxdo после ошибки",
            );

            ui.separator();
            usize_override(
                ui,
                text,
                "x11_modifier_release_timeout",
                defaults.modifier_release_timeout_ms,
                "Ожидание модификаторов:",
                0..=5_000,
                " ms",
            );
            usize_override(
                ui,
                text,
                "x11_focus_retry_count",
                defaults.focus_retry_count,
                "Повторы восстановления focus:",
                0..=20,
                "",
            );
            usize_override(
                ui,
                text,
                "x11_focus_retry_delay",
                defaults.focus_retry_delay_ms,
                "Пауза между попытками focus:",
                0..=1_000,
                " ms",
            );
            usize_override(
                ui,
                text,
                "x11_fast_failure_threshold",
                defaults.fast_failure_threshold,
                "Ошибок до отключения fast backend:",
                1..=20,
                "",
            );

            // usize::MAX means that Fast/Legacy never force clipboard by size.
            // The editor keeps the control finite; leaving the key absent retains
            // the profile default exactly.
            let clipboard_default = defaults.clipboard_threshold.min(10_000);
            usize_override(
                ui,
                text,
                "x11_safe_clipboard_threshold",
                clipboard_default,
                "Clipboard для ASCII длиннее:",
                1..=10_000,
                " симв.",
            );

            if ui.button("Сбросить расширенные overrides к профилю").clicked() {
                *text = remove_top_level_keys(text, OVERRIDE_KEYS);
            }
            *error = validate_yaml(text).err();
        });
}

fn profile_description(profile: X11InjectorProfile) -> &'static str {
    match profile {
        X11InjectorProfile::Safe => {
            "XTest для управляющих клавиш, строгий focus guard, ожидание модификаторов, ранний clipboard и self-healing."
        }
        X11InjectorProfile::Balanced => {
            "Быстрый путь разрешён, но включены focus guard, ожидание модификаторов, circuit breaker и self-healing."
        }
        X11InjectorProfile::Fast => {
            "Минимум защитных проверок; подходит только для уже проверенной X11-среды."
        }
        X11InjectorProfile::Legacy => {
            "Старое поведение с synthetic release всех нажатых клавиш. Использовать только как аварийный откат."
        }
    }
}

fn bool_override(
    ui: &mut egui::Ui,
    text: &mut String,
    key: &str,
    default: bool,
    label: &str,
) {
    let mut value = read_top_level_bool(text, key).unwrap_or(default);
    if ui.checkbox(&mut value, label).changed() {
        *text = set_top_level_bool(text, key, value);
    }
}

fn usize_override(
    ui: &mut egui::Ui,
    text: &mut String,
    key: &str,
    default: usize,
    label: &str,
    range: std::ops::RangeInclusive<usize>,
    suffix: &str,
) {
    let mut value = read_top_level_usize(text, key).unwrap_or(default);
    ui.horizontal(|ui| {
        ui.label(label);
        let editor = egui::DragValue::new(&mut value).range(range).suffix(suffix);
        if ui.add(editor).changed() {
            *text = set_top_level_scalar(text, key, &value.to_string());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_profile_defaults_are_shared_with_studio() {
        let safe = X11SafeInjectorConfig::for_profile(X11InjectorProfile::Safe);
        assert_eq!(safe.modifier_release_timeout_ms, 150);
        assert_eq!(safe.clipboard_threshold, 48);

        let balanced = X11SafeInjectorConfig::for_profile(X11InjectorProfile::Balanced);
        assert_eq!(balanced.fast_failure_threshold, 2);
        assert!(balanced.focus_guard);
    }
}
