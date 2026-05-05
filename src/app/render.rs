use eframe::egui;
use super::*;

impl LuxApp {
    pub(super) fn render_main_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.focus_mode {
            self.show_activity_bar(ui);
            self.show_left_sidebar(ui);
            self.show_git_panel_ui(ui);
        }
        self.show_dock_panel(ui, ctx);

        if !self.zen_mode {
            self.show_tab_bar(ui);
            self.show_breadcrumbs(ui);
            self.show_search_bar(ui);
            self.show_goto_line_bar(ui);
        }

        ui.add_space(0.0);

        // Editor area (takes remaining space minus status bar)
        let status_bar_height = if self.zen_mode { 0.0 } else { 24.0 };
        let available = ui.available_rect_before_wrap();
        let editor_rect = egui::Rect::from_min_max(
            available.min,
            egui::Pos2::new(available.max.x, available.max.y - status_bar_height),
        );
        let editor_idx = self.active_tab;
        let show_preview = {
            let editor = &self.editors[editor_idx];
            editor.is_markdown() && editor.markdown_preview
        };
        let preview_text = if show_preview {
            Some(self.editors[editor_idx].rope.to_string())
        } else {
            None
        };

        let auto_focus = !self.show_search
            && !self.show_goto_line
            && !self.command_palette.visible
            && self.confirm_close_tab.is_none();
        let mut editor_theme = self.editor_theme.palette();
        if let Some(bg) = self.theme_override_bg {
            editor_theme.background = bg;
            editor_theme.gutter_bg = bg;
        }
        if let Some(text) = self.theme_override_text {
            editor_theme.text = text;
            editor_theme.cursor = text;
        }
        let font_settings = self.current_font_settings();
        let search_query_owned = if self.search_input.trim().is_empty() {
            None
        } else {
            Some(self.search_input.clone())
        };
        let search_query = search_query_owned.as_deref();

        let min_editor_width = 360.0;
        let min_preview_width = 260.0;
        let can_split = editor_rect.width() > (min_editor_width + min_preview_width);

        self.ensure_split_secondary();
        if let Some(secondary_idx) = self.split_secondary_tab {
            if self.split_mode != SplitMode::None
                && secondary_idx < self.editors.len()
                && secondary_idx != self.active_tab
            {
                let (first_rect, second_rect) = if self.split_mode == SplitMode::Vertical {
                    let w = editor_rect.width() / 2.0;
                    (
                        egui::Rect::from_min_max(
                            editor_rect.min,
                            egui::Pos2::new(editor_rect.min.x + w - 1.0, editor_rect.max.y),
                        ),
                        egui::Rect::from_min_max(
                            egui::Pos2::new(editor_rect.min.x + w + 1.0, editor_rect.min.y),
                            editor_rect.max,
                        ),
                    )
                } else {
                    let h = editor_rect.height() / 2.0;
                    (
                        egui::Rect::from_min_max(
                            editor_rect.min,
                            egui::Pos2::new(editor_rect.max.x, editor_rect.min.y + h - 1.0),
                        ),
                        egui::Rect::from_min_max(
                            egui::Pos2::new(editor_rect.min.x, editor_rect.min.y + h + 1.0),
                            editor_rect.max,
                        ),
                    )
                };
                let active_idx = self.active_tab;
                let (editors, clipboard, highlighter) =
                    (&mut self.editors, &mut self.clipboard, &self.highlighter);
                let (first_editor, second_editor) = if active_idx < secondary_idx {
                    let (left, right) = editors.split_at_mut(secondary_idx);
                    (&mut left[active_idx], &mut right[0])
                } else {
                    let (left, right) = editors.split_at_mut(active_idx);
                    (&mut right[0], &mut left[secondary_idx])
                };
                let mut first_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(first_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                crate::ui::editor_view::show(
                    &mut first_ui,
                    first_editor,
                    clipboard,
                    highlighter,
                    &editor_theme,
                    &font_settings,
                    search_query,
                    auto_focus,
                );
                let mut second_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(second_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                crate::ui::editor_view::show(
                    &mut second_ui,
                    second_editor,
                    clipboard,
                    highlighter,
                    &editor_theme,
                    &font_settings,
                    search_query,
                    false,
                );
            }
        } else if show_preview && can_split {
            let preview_width = (editor_rect.width() * 0.42)
                .clamp(min_preview_width, editor_rect.width() - min_editor_width);
            let editor_width = editor_rect.width() - preview_width;
            let editor_rect = egui::Rect::from_min_max(
                editor_rect.min,
                egui::Pos2::new(editor_rect.min.x + editor_width, editor_rect.max.y),
            );
            let preview_rect = egui::Rect::from_min_max(
                egui::Pos2::new(editor_rect.max.x, editor_rect.min.y),
                egui::Pos2::new(editor_rect.max.x + preview_width, editor_rect.max.y),
            );

            let separator_rect = egui::Rect::from_min_max(
                egui::Pos2::new(preview_rect.min.x - 1.0, preview_rect.min.y),
                egui::Pos2::new(preview_rect.min.x, preview_rect.max.y),
            );
            ui.painter()
                .rect_filled(separator_rect, 0.0, egui::Color32::from_rgb(55, 55, 58));

            let mut editor_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(editor_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            crate::ui::editor_view::show(
                &mut editor_ui,
                &mut self.editors[self.active_tab],
                &mut self.clipboard,
                &self.highlighter,
                &editor_theme,
                &font_settings,
                search_query,
                auto_focus,
            );

            if let Some(text) = preview_text.as_deref() {
                let mut preview_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(preview_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                markdown_preview::show(&mut preview_ui, text);
            }
        } else {
            let mut editor_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(editor_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            crate::ui::editor_view::show(
                &mut editor_ui,
                &mut self.editors[self.active_tab],
                &mut self.clipboard,
                &self.highlighter,
                &editor_theme,
                &font_settings,
                search_query,
                auto_focus,
            );
        }

        if !self.zen_mode {
            crate::ui::status_bar::show(
                ui,
                &mut self.editors[self.active_tab],
                self.git_info.as_ref(),
                &self.highlighter,
            );
        }
    }
}

