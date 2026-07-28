use eframe::egui;
use std::time::Duration;

pub fn show(ui: &mut egui::Ui) -> egui::Response {
    let size = egui::vec2(46.0, 42.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.ctx().request_repaint_after(Duration::from_millis(120));

    if ui.is_rect_visible(rect) {
        let time = ui.input(|input| input.time);
        let phase = ((time / 0.12) as usize) % 8;
        let pulse = [0.45_f32, 0.62, 0.82, 1.0, 0.78, 0.58, 0.42, 0.62][phase];
        let painter = ui.painter_at(rect);
        let center = rect.center() + egui::vec2(0.0, -1.0);

        let glow = egui::Color32::from_rgba_unmultiplied(35, 196, 255, (45.0 + 75.0 * pulse) as u8);
        painter.circle_filled(center + egui::vec2(-10.0, -2.0), 13.0, glow);
        painter.circle_filled(center + egui::vec2(1.0, -8.0), 15.0, glow);
        painter.circle_filled(center + egui::vec2(13.0, -1.0), 12.0, glow);

        let rim = egui::Color32::from_rgb(42, 151, 231);
        let cloud = egui::Color32::from_rgb(11, 26, 54);
        for (offset, radius) in [
            (egui::vec2(-10.0, -2.0), 10.5),
            (egui::vec2(1.0, -8.0), 12.5),
            (egui::vec2(13.0, -1.0), 9.5),
        ] {
            painter.circle_filled(center + offset, radius + 1.4, rim);
            painter.circle_filled(center + offset, radius, cloud);
        }
        painter.rect_filled(
            egui::Rect::from_min_max(
                center + egui::vec2(-18.0, -2.0),
                center + egui::vec2(21.0, 8.0),
            ),
            3.0,
            cloud,
        );

        let shift = [0.0_f32, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 1.0][phase];
        let bolt = [
            center + egui::vec2(-1.0 + shift, 2.0),
            center + egui::vec2(8.0 + shift, 2.0),
            center + egui::vec2(3.0 + shift, 9.0),
            center + egui::vec2(8.0 + shift, 9.0),
            center + egui::vec2(-8.0 + shift, 21.0),
            center + egui::vec2(-3.0 + shift, 11.0),
            center + egui::vec2(-8.0 + shift, 11.0),
        ];
        let bolt_glow =
            egui::Color32::from_rgba_unmultiplied(76, 218, 255, (65.0 + 90.0 * pulse) as u8);
        painter.add(egui::Shape::convex_polygon(
            bolt.iter()
                .map(|point| *point + egui::vec2(0.0, 1.0))
                .collect(),
            bolt_glow,
            egui::Stroke::new(3.0, bolt_glow),
        ));
        painter.add(egui::Shape::convex_polygon(
            bolt.to_vec(),
            egui::Color32::from_rgb(255, (220.0 + 30.0 * pulse) as u8, 104),
            egui::Stroke::new(0.8, egui::Color32::WHITE),
        ));

        let rain_offset = (phase % 4) as f32;
        let rain = egui::Stroke::new(
            1.5,
            egui::Color32::from_rgba_unmultiplied(74, 188, 255, 150),
        );
        painter.line_segment(
            [
                center + egui::vec2(-14.0, 10.0 + rain_offset),
                center + egui::vec2(-16.0, 15.0 + rain_offset),
            ],
            rain,
        );
        painter.line_segment(
            [
                center + egui::vec2(15.0, 9.0 + ((rain_offset + 2.0) % 4.0)),
                center + egui::vec2(13.0, 14.0 + ((rain_offset + 2.0) % 4.0)),
            ],
            rain,
        );
    }

    response.on_hover_text("rEspanso — грозовой движок подстановок")
}
