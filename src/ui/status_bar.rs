use eframe::egui;

use crate::editor::Editor;

const BAR_HEIGHT: f32 = 22.0;
const BAR_ITEM_HOVER_ALPHA: u8 = 30;

const BAR_BG: egui::Color32 = egui::Color32::from_rgb(0, 122, 204); // VS Code Blue for status bar
const BAR_TEXT: egui::Color32 = egui::Color32::WHITE;

#[derive(Clone, Debug, Default)]
pub struct GitInfo {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: bool,
}

pub fn show(ui: &mut egui::Ui, editor: &Editor, git_info: Option<&GitInfo>) {
    let rect = ui.available_rect_before_wrap();
    let bar_rect = egui::Rect::from_min_size(
        egui::Pos2::new(rect.left(), rect.bottom() - BAR_HEIGHT),
        egui::Vec2::new(rect.width(), BAR_HEIGHT),
    );

    ui.painter().rect_filled(bar_rect, 0.0, BAR_BG);
    
    // Allocate the area
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center))
    );
    ui.spacing_mut().item_spacing = egui::Vec2::new(0.0, 0.0);
    
    // Left Side (Git)
    if let Some(info) = git_info {
        let mut status = info.branch.clone();
        if info.dirty {
            status.push('*');
        }
        if info.ahead > 0 {
            status.push_str(&format!(" ↑{}", info.ahead));
        }
        if info.behind > 0 {
            status.push_str(&format!(" ↓{}", info.behind));
        }
        
        status_item(&mut ui, &format!("\u{E0A0} {}", status)); //  branch symbol (requires nerd font, fallback text)
        // Note: \u{E0A0} is Nerd Font git branch. 
        // If not available, we can use "git: "
    }

    // Spacer
    ui.label(""); 

    // Right Side
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // Cursor Position
        let primary = &editor.cursors[0];
        let cursor_text = if editor.cursors.len() > 1 {
            format!("Ln {}, Col {} ({} cursors)", primary.pos.line + 1, primary.pos.col + 1, editor.cursors.len())
        } else {
            format!("Ln {}, Col {}", primary.pos.line + 1, primary.pos.col + 1)
        };
        status_item(ui, &cursor_text);

        // Encoding / Spaces
        status_item(ui, "UTF-8"); // Placeholder
        status_item(ui, "Spaces: 4"); // Placeholder

        // Language
         let syntax = editor
            .file_path
            .as_ref()
            .and_then(|p| p.extension().and_then(|e| e.to_str()))
            .map(|ext| ext.to_uppercase())
            .unwrap_or_else(|| "PLAIN TEXT".into());
        status_item(ui, &syntax);
    });
}

fn status_item(ui: &mut egui::Ui, text: &str) {
    let font = egui::FontId::proportional(12.0);
    let text_color = BAR_TEXT;
    
    // Calculate size
    let padding = egui::vec2(10.0, 0.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), text_color);
    let (rect, resp) = ui.allocate_exact_size(
        egui::Vec2::new(galley.rect.width() + padding.x * 2.0, BAR_HEIGHT),
        egui::Sense::click()
    );
    
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_white_alpha(BAR_ITEM_HOVER_ALPHA));
    }
    
    ui.painter().galley(
        egui::Pos2::new(rect.left() + padding.x, rect.center().y - galley.rect.height() / 2.0), 
        galley,
        text_color
    );
}
