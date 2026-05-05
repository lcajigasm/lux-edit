use eframe::egui;
use super::*;

impl LuxApp {
    pub(super) fn show_search_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_search {
            return;
        }

        ui.add_space(8.0);
        // Floating panel look
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(37, 37, 38))
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 122, 204))) // Active border
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Find")
                            .color(egui::Color32::from_rgb(200, 200, 200))
                            .size(13.0),
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_input)
                            .desired_width(200.0)
                            .font(egui::FontId::monospace(13.0))
                            .text_color(egui::Color32::WHITE)
                            .margin(egui::Vec2::new(4.0, 2.0))
                            .hint_text("Search..."),
                    );

                    if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let query = self.search_input.clone();
                        if ui.input(|i| i.modifiers.shift) {
                            self.active_editor().find_prev(&query);
                        } else {
                            self.active_editor().find_next(&query);
                        }
                        response.request_focus();
                    }

                    if ui
                        .add(egui::Button::new(egui::RichText::new("↑").size(14.0)))
                        .on_hover_text("Previous Match")
                        .clicked()
                    {
                        let query = self.search_input.clone();
                        self.active_editor().find_prev(&query);
                    }

                    if ui
                        .add(egui::Button::new(egui::RichText::new("↓").size(14.0)))
                        .on_hover_text("Next Match")
                        .clicked()
                    {
                        let query = self.search_input.clone();
                        self.active_editor().find_next(&query);
                    }

                    // Toggles for options could go here

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("\u{1F5D9}").size(14.0), // Cancel X
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.show_search = false;
                        self.show_replace = false;
                    }
                });
            });

        // Replace row
        if self.show_replace {
            ui.add_space(2.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(37, 37, 38))
                .rounding(egui::Rounding::same(4.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60)))
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Replace")
                                .color(egui::Color32::from_rgb(200, 200, 200))
                                .size(13.0),
                        );

                        ui.add(
                            egui::TextEdit::singleline(&mut self.replace_input)
                                .desired_width(200.0)
                                .font(egui::FontId::monospace(13.0))
                                .text_color(egui::Color32::WHITE)
                                .margin(egui::Vec2::new(4.0, 2.0))
                                .hint_text("Replace..."),
                        );

                        if ui
                            .add(egui::Button::new(egui::RichText::new("Replace").size(12.0)))
                            .clicked()
                        {
                            let find = self.search_input.clone();
                            let replace = self.replace_input.clone();
                            self.active_editor().replace_next(&find, &replace);
                        }

                        if ui
                            .add(egui::Button::new(egui::RichText::new("All").size(12.0)))
                            .clicked()
                        {
                            let find = self.search_input.clone();
                            let replace = self.replace_input.clone();
                            self.active_editor().replace_all(&find, &replace);
                        }
                    });
                });
        }
    }

    pub(super) fn show_goto_line_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_goto_line {
            return;
        }

        ui.add_space(8.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(37, 37, 38))
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 122, 204)))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Go to Line")
                            .color(egui::Color32::from_rgb(200, 200, 200))
                            .size(13.0),
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.goto_line_input)
                            .desired_width(100.0)
                            .font(egui::FontId::monospace(13.0))
                            .text_color(egui::Color32::WHITE)
                            .margin(egui::Vec2::new(4.0, 2.0))
                            .hint_text("Line..."),
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(line) = self.goto_line_input.trim().parse::<usize>() {
                            self.active_editor().goto_line(line);
                        }
                        self.show_goto_line = false;
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.show_goto_line = false;
                    }

                    if ui.button("Go").clicked() {
                        if let Ok(line) = self.goto_line_input.trim().parse::<usize>() {
                            self.active_editor().goto_line(line);
                        }
                        self.show_goto_line = false;
                    }
                });
            });
    }

    pub(super) fn show_breadcrumbs(&mut self, ui: &mut egui::Ui) {
        let editor = &self.editors[self.active_tab];
        let crumbs = if let Some(path) = &editor.file_path {
            let mut parts: Vec<String> = path
                .components()
                .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
                .collect();
            if parts.is_empty() {
                vec![editor.title.clone()]
            } else {
                let file = parts.pop().unwrap_or_else(|| editor.title.clone());
                parts.push(file);
                parts
            }
        } else {
            vec![editor.title.clone()]
        };

        // Minimal breadcrumbs (transparent background)
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(16.0, 6.0)) // Indent start
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::new(4.0, 0.0);
                    for (idx, part) in crumbs.iter().enumerate() {
                        let is_last = idx == crumbs.len() - 1;
                        let color = if is_last {
                            egui::Color32::from_rgb(220, 220, 220)
                        } else {
                            egui::Color32::from_rgb(120, 120, 120)
                        };

                        // Clickable logic could go here later
                        ui.label(egui::RichText::new(part).color(color).size(12.0));

                        if !is_last {
                            ui.label(
                                egui::RichText::new("›")
                                    .color(egui::Color32::from_rgb(80, 80, 80))
                                    .size(12.0),
                            );
                        }
                    }
                });
            });
    }
}
