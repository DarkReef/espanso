use eframe::egui;
use std::{fs, path::Path};

const THEME_FILE: &str = ".match-studio-theme";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioTheme {
    Light,
    Dark,
}

impl Default for StudioTheme {
    fn default() -> Self {
        Self::Light
    }
}

impl StudioTheme {
    pub fn load(config_root: &Path) -> Self {
        let Ok(value) = fs::read_to_string(config_root.join(THEME_FILE)) else {
            return Self::default();
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "dark" => Self::Dark,
            _ => Self::Light,
        }
    }

    pub fn save(self, config_root: &Path) -> Result<(), String> {
        let value = match self {
            Self::Light => "light\n",
            Self::Dark => "dark\n",
        };
        fs::write(config_root.join(THEME_FILE), value)
            .map_err(|error| format!("Не удалось сохранить тему Match Studio: {error}"))
    }

    pub fn apply(self, context: &egui::Context) {
        context.set_visuals(self.visuals());
    }

    /// Apply to both the current Ui and the Context. Updating the current Ui is
    /// important when the user switches theme mid-frame: child widgets then
    /// inherit the new palette immediately instead of mixing old panel colors
    /// with new text/input colors until another frame is created.
    pub fn apply_to_ui(self, ui: &mut egui::Ui) {
        let visuals = self.visuals();
        ui.style_mut().visuals = visuals.clone();
        ui.ctx().set_visuals(visuals);
    }

    fn visuals(self) -> egui::Visuals {
        let mut visuals = match self {
            Self::Light => egui::Visuals::light(),
            Self::Dark => egui::Visuals::dark(),
        };
        let palette = self.palette();
        let border = egui::Stroke::new(1.0, palette.border);
        let accent = egui::Stroke::new(1.0, palette.accent);
        let text = egui::Stroke::new(1.0, palette.text);

        // Global surfaces and text.
        visuals.dark_mode = self == Self::Dark;
        visuals.override_text_color = Some(palette.text);
        visuals.weak_text_color = Some(palette.weak_text);
        visuals.panel_fill = palette.background;
        visuals.window_fill = palette.surface;
        visuals.window_stroke = border;
        visuals.faint_bg_color = palette.surface_alt;
        visuals.extreme_bg_color = palette.input;
        // egui 0.35 has a dedicated TextEdit fill. Setting it explicitly avoids
        // a light input on a dark page (or the reverse) after a theme switch.
        visuals.text_edit_bg_color = Some(palette.input);
        visuals.code_bg_color = palette.code;
        visuals.hyperlink_color = palette.accent;
        visuals.selection.bg_fill = palette.selection;
        visuals.selection.stroke = accent;
        visuals.warn_fg_color = palette.warning;
        visuals.error_fg_color = palette.error;

        // Non-interactive frames: groups, separators and ordinary text.
        visuals.widgets.noninteractive.bg_fill = palette.surface;
        visuals.widgets.noninteractive.weak_bg_fill = palette.surface;
        visuals.widgets.noninteractive.bg_stroke = border;
        visuals.widgets.noninteractive.fg_stroke = text;

        // Resting interactive controls: buttons, checkboxes, radio buttons,
        // sliders and the frame around text fields.
        visuals.widgets.inactive.bg_fill = palette.input;
        visuals.widgets.inactive.weak_bg_fill = palette.surface_alt;
        visuals.widgets.inactive.bg_stroke = border;
        visuals.widgets.inactive.fg_stroke = text;

        // Hover/focus states remain readable in both palettes.
        visuals.widgets.hovered.bg_fill = palette.hover;
        visuals.widgets.hovered.weak_bg_fill = palette.hover;
        visuals.widgets.hovered.bg_stroke = accent;
        visuals.widgets.hovered.fg_stroke = text;

        visuals.widgets.active.bg_fill = palette.selection;
        visuals.widgets.active.weak_bg_fill = palette.selection;
        visuals.widgets.active.bg_stroke = accent;
        visuals.widgets.active.fg_stroke = text;

        visuals.widgets.open.bg_fill = palette.hover;
        visuals.widgets.open.weak_bg_fill = palette.hover;
        visuals.widgets.open.bg_stroke = accent;
        visuals.widgets.open.fg_stroke = text;

        visuals
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "Светлая",
            Self::Dark => "Тёмная",
        }
    }

    pub const fn success(self) -> egui::Color32 {
        self.palette().success
    }

    pub const fn warning(self) -> egui::Color32 {
        self.palette().warning
    }

    pub const fn error(self) -> egui::Color32 {
        self.palette().error
    }

    const fn palette(self) -> Palette {
        match self {
            Self::Light => Palette {
                background: egui::Color32::from_rgb(245, 247, 250),
                surface: egui::Color32::from_rgb(255, 255, 255),
                surface_alt: egui::Color32::from_rgb(235, 240, 246),
                input: egui::Color32::from_rgb(255, 255, 255),
                hover: egui::Color32::from_rgb(224, 233, 247),
                code: egui::Color32::from_rgb(238, 242, 247),
                text: egui::Color32::from_rgb(24, 33, 47),
                weak_text: egui::Color32::from_rgb(82, 94, 112),
                border: egui::Color32::from_rgb(190, 202, 217),
                accent: egui::Color32::from_rgb(47, 111, 237),
                selection: egui::Color32::from_rgb(190, 211, 252),
                success: egui::Color32::from_rgb(31, 122, 71),
                warning: egui::Color32::from_rgb(154, 101, 0),
                error: egui::Color32::from_rgb(180, 35, 24),
            },
            Self::Dark => Palette {
                background: egui::Color32::from_rgb(18, 22, 28),
                surface: egui::Color32::from_rgb(27, 33, 42),
                surface_alt: egui::Color32::from_rgb(35, 43, 54),
                input: egui::Color32::from_rgb(22, 28, 36),
                hover: egui::Color32::from_rgb(45, 56, 71),
                code: egui::Color32::from_rgb(15, 20, 27),
                text: egui::Color32::from_rgb(242, 245, 248),
                weak_text: egui::Color32::from_rgb(177, 188, 202),
                border: egui::Color32::from_rgb(65, 78, 97),
                accent: egui::Color32::from_rgb(119, 167, 255),
                selection: egui::Color32::from_rgb(48, 82, 137),
                success: egui::Color32::from_rgb(87, 199, 133),
                warning: egui::Color32::from_rgb(231, 180, 90),
                error: egui::Color32::from_rgb(255, 123, 114),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    background: egui::Color32,
    surface: egui::Color32,
    surface_alt: egui::Color32,
    input: egui::Color32,
    hover: egui::Color32,
    code: egui::Color32,
    text: egui::Color32,
    weak_text: egui::Color32,
    border: egui::Color32,
    accent: egui::Color32,
    selection: egui::Color32,
    success: egui::Color32,
    warning: egui::Color32,
    error: egui::Color32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_localized() {
        assert_eq!(StudioTheme::Light.label(), "Светлая");
        assert_eq!(StudioTheme::Dark.label(), "Тёмная");
    }

    #[test]
    fn text_edits_follow_each_theme() {
        let light = StudioTheme::Light.visuals();
        let dark = StudioTheme::Dark.visuals();
        assert_eq!(light.text_edit_bg_color, Some(StudioTheme::Light.palette().input));
        assert_eq!(dark.text_edit_bg_color, Some(StudioTheme::Dark.palette().input));
        assert_eq!(light.override_text_color, Some(StudioTheme::Light.palette().text));
        assert_eq!(dark.override_text_color, Some(StudioTheme::Dark.palette().text));
        assert_ne!(light.text_edit_bg_color, dark.text_edit_bg_color);
        assert_ne!(light.override_text_color, dark.override_text_color);
    }
}
