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
        let mut visuals = match self {
            Self::Light => egui::Visuals::light(),
            Self::Dark => egui::Visuals::dark(),
        };

        let palette = self.palette();
        visuals.override_text_color = Some(palette.text);
        visuals.panel_fill = palette.background;
        visuals.window_fill = palette.surface;
        visuals.extreme_bg_color = palette.input;
        visuals.faint_bg_color = palette.surface_alt;
        visuals.code_bg_color = palette.code;
        visuals.hyperlink_color = palette.accent;
        visuals.selection.bg_fill = palette.selection;
        visuals.selection.stroke = egui::Stroke::new(1.0, palette.accent);
        visuals.window_stroke = egui::Stroke::new(1.0, palette.border);
        visuals.warn_fg_color = palette.warning;
        visuals.error_fg_color = palette.error;
        context.set_visuals(visuals);
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
                code: egui::Color32::from_rgb(238, 242, 247),
                text: egui::Color32::from_rgb(24, 33, 47),
                border: egui::Color32::from_rgb(205, 214, 225),
                accent: egui::Color32::from_rgb(47, 111, 237),
                selection: egui::Color32::from_rgb(190, 211, 252),
                success: egui::Color32::from_rgb(31, 122, 71),
                warning: egui::Color32::from_rgb(154, 101, 0),
                error: egui::Color32::from_rgb(180, 35, 24),
            },
            Self::Dark => Palette {
                background: egui::Color32::from_rgb(20, 24, 31),
                surface: egui::Color32::from_rgb(28, 34, 43),
                surface_alt: egui::Color32::from_rgb(36, 44, 55),
                input: egui::Color32::from_rgb(23, 29, 37),
                code: egui::Color32::from_rgb(17, 22, 29),
                text: egui::Color32::from_rgb(242, 245, 248),
                border: egui::Color32::from_rgb(54, 66, 83),
                accent: egui::Color32::from_rgb(119, 167, 255),
                selection: egui::Color32::from_rgb(47, 83, 140),
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
    code: egui::Color32,
    text: egui::Color32,
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
}
