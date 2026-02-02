use eframe::egui;

use crate::editor::Editor;

const BAR_HEIGHT: f32 = 24.0;
const BAR_BG: egui::Color32 = egui::Color32::from_rgb(53, 54, 56);
const BAR_TEXT: egui::Color32 = egui::Color32::from_rgb(220, 220, 220);
const BAR_BORDER: egui::Color32 = egui::Color32::from_rgb(30, 30, 30);

pub fn show(ui: &mut egui::Ui, editor: &Editor) {
    let rect = ui.available_rect_before_wrap();
    let bar_rect = egui::Rect::from_min_size(
        egui::Pos2::new(rect.left(), rect.bottom() - BAR_HEIGHT),
        egui::Vec2::new(rect.width(), BAR_HEIGHT),
    );

    ui.painter().rect_filled(bar_rect, 0.0, BAR_BG);
    ui.painter().line_segment(
        [
            egui::Pos2::new(bar_rect.left(), bar_rect.top()),
            egui::Pos2::new(bar_rect.right(), bar_rect.top()),
        ],
        egui::Stroke::new(1.0, BAR_BORDER),
    );
    ui.allocate_rect(bar_rect, egui::Sense::hover());

    let primary = &editor.cursors[0];

    // Left side: file info
    let file_info = if let Some(path) = &editor.file_path {
        path.to_string_lossy().to_string()
    } else {
        "Untitled".into()
    };

    let modified_marker = if editor.modified { " [Modified]" } else { "" };

    ui.painter().text(
        egui::Pos2::new(bar_rect.left() + 12.0, bar_rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{}{}", file_info, modified_marker),
        egui::FontId::proportional(12.0),
        BAR_TEXT,
    );

    // Middle: syntax / encoding info similar to Sublime
    let syntax = editor
        .file_path
        .as_ref()
        .and_then(|p| p.extension().and_then(|e| e.to_str()))
        .map(|ext| ext.to_uppercase())
        .unwrap_or_else(|| "PLAIN TEXT".into());
    let center_info = format!("{}   UTF-8   Spaces: 4", syntax);
    ui.painter().text(
        egui::Pos2::new(bar_rect.center().x, bar_rect.center().y),
        egui::Align2::CENTER_CENTER,
        center_info,
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(190, 190, 190),
    );

    // Right side: cursor position + cursor count
    let cursor_info = if editor.cursors.len() > 1 {
        format!(
            "Ln {}, Col {} ({} cursors)",
            primary.pos.line + 1,
            primary.pos.col + 1,
            editor.cursors.len()
        )
    } else {
        format!("Ln {}, Col {}", primary.pos.line + 1, primary.pos.col + 1)
    };

    ui.painter().text(
        egui::Pos2::new(bar_rect.right() - 12.0, bar_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        cursor_info,
        egui::FontId::proportional(12.0),
        BAR_TEXT,
    );
}
