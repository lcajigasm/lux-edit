use arboard::Clipboard;
use eframe::egui;

use crate::editor::Editor;
use crate::syntax::SyntaxHighlighter;
use crate::ui::command_palette::{CommandId, CommandPalette};

pub struct LuxApp {
    pub editors: Vec<Editor>,
    pub active_tab: usize,
    pub command_palette: CommandPalette,
    pub show_search: bool,
    pub show_replace: bool,
    pub search_input: String,
    pub replace_input: String,
    pub show_goto_line: bool,
    pub goto_line_input: String,
    pub clipboard: Option<Clipboard>,
    pub highlighter: SyntaxHighlighter,
    /// If Some, show a "save before closing?" dialog for this tab index.
    pub confirm_close_tab: Option<usize>,
}

impl LuxApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            editors: vec![Editor::new()],
            active_tab: 0,
            command_palette: CommandPalette::new(),
            show_search: false,
            show_replace: false,
            search_input: String::new(),
            replace_input: String::new(),
            show_goto_line: false,
            goto_line_input: String::new(),
            clipboard: Clipboard::new().ok(),
            highlighter: SyntaxHighlighter::new(),
            confirm_close_tab: None,
        }
    }

    fn active_editor(&mut self) -> &mut Editor {
        &mut self.editors[self.active_tab]
    }

    fn new_tab(&mut self) {
        self.editors.push(Editor::new());
        self.active_tab = self.editors.len() - 1;
    }

    fn close_tab(&mut self) {
        self.close_tab_idx(self.active_tab);
    }

    fn close_tab_idx(&mut self, idx: usize) {
        if self.editors.len() <= 1 {
            return;
        }
        if self.editors[idx].modified {
            self.confirm_close_tab = Some(idx);
        } else {
            self.force_close_tab(idx);
        }
    }

    fn force_close_tab(&mut self, idx: usize) {
        if self.editors.len() > 1 {
            self.editors.remove(idx);
            if self.active_tab >= self.editors.len() {
                self.active_tab = self.editors.len() - 1;
            }
        }
        self.confirm_close_tab = None;
    }

    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            match Editor::from_file(path) {
                Ok(editor) => {
                    self.editors.push(editor);
                    self.active_tab = self.editors.len() - 1;
                }
                Err(e) => {
                    eprintln!("Failed to open file: {}", e);
                }
            }
        }
    }

    fn save_file(&mut self) {
        let editor = &mut self.editors[self.active_tab];
        if editor.file_path.is_some() {
            if let Err(e) = editor.save() {
                eprintln!("Failed to save: {}", e);
            }
        } else {
            self.save_file_as();
        }
    }

    fn save_file_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new().save_file() {
            if let Err(e) = self.editors[self.active_tab].save_as(path) {
                eprintln!("Failed to save: {}", e);
            }
        }
    }

    fn handle_command(&mut self, cmd: CommandId) {
        match cmd {
            CommandId::NewTab => self.new_tab(),
            CommandId::OpenFile => self.open_file(),
            CommandId::SaveFile => self.save_file(),
            CommandId::SaveFileAs => self.save_file_as(),
            CommandId::CloseTab => self.close_tab(),
            CommandId::Find => {
                self.show_search = true;
                self.show_goto_line = false;
            }
            CommandId::GoToLine => {
                self.show_goto_line = true;
                self.show_search = false;
            }
            CommandId::SelectAll => {
                self.active_editor().select_all();
            }
            CommandId::Undo => self.active_editor().undo(),
            CommandId::Redo => self.active_editor().redo(),
        }
    }

    fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            let ctrl = i.modifiers.command;
            let shift = i.modifiers.shift;

            if ctrl && shift && i.key_pressed(egui::Key::P) {
                self.command_palette.toggle();
            } else if ctrl && i.key_pressed(egui::Key::N) {
                self.new_tab();
            } else if ctrl && i.key_pressed(egui::Key::O) {
                // Defer file dialog to avoid borrow issues
            } else if ctrl && i.key_pressed(egui::Key::S) {
                if shift {
                    // save as - defer
                } else {
                    // save - defer
                }
            } else if ctrl && i.key_pressed(egui::Key::W) {
                self.close_tab();
            } else if ctrl && i.key_pressed(egui::Key::F) {
                self.show_search = !self.show_search;
                self.show_replace = false;
                self.show_goto_line = false;
            } else if ctrl && i.key_pressed(egui::Key::H) {
                self.show_search = true;
                self.show_replace = !self.show_replace;
                self.show_goto_line = false;
            } else if ctrl && i.key_pressed(egui::Key::G) {
                self.show_goto_line = !self.show_goto_line;
                self.show_search = false;
            }
        });

        // Handle open/save outside of input closure to avoid borrow issues
        let should_open = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
        let should_save = ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S));
        let should_save_as = ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S));

        if should_open {
            self.open_file();
        }
        if should_save {
            self.save_file();
        }
        if should_save_as {
            self.save_file_as();
        }
    }

    fn show_tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 0.0;

            for i in 0..self.editors.len() {
                let title = &self.editors[i].title;
                let modified = self.editors[i].modified;
                let is_active = i == self.active_tab;

                let label = if modified {
                    format!(" {} \u{25CF}", title) // ● dot for modified
                } else {
                    format!(" {}", title)
                };

                let bg = if is_active {
                    egui::Color32::from_rgb(30, 30, 30)
                } else {
                    egui::Color32::from_rgb(45, 45, 45)
                };
                let text_color = if is_active {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_rgb(160, 160, 160)
                };

                let tab_rounding = egui::Rounding {
                    nw: 4.0,
                    ne: 4.0,
                    sw: 0.0,
                    se: 0.0,
                };
                let tab_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60));

                // Tab label button
                let response = ui.add(
                    egui::Button::new(
                        egui::RichText::new(&label)
                            .color(text_color)
                            .size(12.0),
                    )
                    .fill(bg)
                    .rounding(tab_rounding)
                    .stroke(tab_stroke),
                );

                if response.clicked() {
                    self.active_tab = i;
                }
                if response.middle_clicked() && self.editors.len() > 1 {
                    self.close_tab_idx(i);
                    break;
                }

                // Close "x" button (only if more than 1 tab)
                if self.editors.len() > 1 {
                    let x_resp = ui.add(
                        egui::Button::new(
                            egui::RichText::new("\u{00D7}") // ×
                                .color(egui::Color32::from_rgb(140, 140, 140))
                                .size(12.0),
                        )
                        .fill(bg)
                        .rounding(egui::Rounding::ZERO)
                        .stroke(egui::Stroke::NONE),
                    );
                    if x_resp.clicked() {
                        self.close_tab_idx(i);
                        break;
                    }
                }

                ui.add_space(2.0);
            }

            // "+" button for new tab
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(" + ")
                            .color(egui::Color32::from_rgb(160, 160, 160))
                            .size(12.0),
                    )
                    .fill(egui::Color32::from_rgb(45, 45, 45))
                    .rounding(egui::Rounding {
                        nw: 4.0,
                        ne: 4.0,
                        sw: 0.0,
                        se: 0.0,
                    }),
                )
                .clicked()
            {
                self.new_tab();
            }
        });
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_search {
            return;
        }

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Find:")
                    .color(egui::Color32::from_rgb(200, 200, 200))
                    .size(13.0),
            );

            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search_input)
                    .desired_width(250.0)
                    .font(egui::FontId::monospace(13.0))
                    .text_color(egui::Color32::WHITE)
                    .hint_text("Search..."),
            );

            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                let query = self.search_input.clone();
                self.active_editor().find_and_select(&query);
                response.request_focus();
            }

            if ui
                .add(egui::Button::new(egui::RichText::new("Next").size(12.0)))
                .clicked()
            {
                let query = self.search_input.clone();
                self.active_editor().find_and_select(&query);
            }

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_search = false;
                self.show_replace = false;
            }

            if ui
                .add(egui::Button::new(egui::RichText::new("\u{2715}").size(12.0)))
                .clicked()
            {
                self.show_search = false;
                self.show_replace = false;
            }
        });

        // Replace row
        if self.show_replace {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Replace:")
                        .color(egui::Color32::from_rgb(200, 200, 200))
                        .size(13.0),
                );

                ui.add(
                    egui::TextEdit::singleline(&mut self.replace_input)
                        .desired_width(250.0)
                        .font(egui::FontId::monospace(13.0))
                        .text_color(egui::Color32::WHITE)
                        .hint_text("Replace with..."),
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
                    .add(egui::Button::new(egui::RichText::new("Replace All").size(12.0)))
                    .clicked()
                {
                    let find = self.search_input.clone();
                    let replace = self.replace_input.clone();
                    self.active_editor().replace_all(&find, &replace);
                }
            });
        }
    }

    fn show_goto_line_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_goto_line {
            return;
        }

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Go to Line:")
                    .color(egui::Color32::from_rgb(200, 200, 200))
                    .size(13.0),
            );

            let response = ui.add(
                egui::TextEdit::singleline(&mut self.goto_line_input)
                    .desired_width(100.0)
                    .font(egui::FontId::monospace(13.0))
                    .text_color(egui::Color32::WHITE)
                    .hint_text("Line number"),
            );

            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                if let Ok(line) = self.goto_line_input.trim().parse::<usize>() {
                    self.active_editor().goto_line(line);
                }
                self.show_goto_line = false;
            }

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_goto_line = false;
            }
        });
    }
}

impl eframe::App for LuxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());

        // Global shortcuts (handled before UI to avoid conflicts)
        if !self.command_palette.visible {
            self.handle_global_shortcuts(ctx);
        }

        // Command palette (rendered as overlay)
        if let Some(cmd) = self.command_palette.show(ctx) {
            self.handle_command(cmd);
        }

        // Main panel
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(45, 45, 45))
                    .inner_margin(egui::Margin::same(0.0)),
            )
            .show(ctx, |ui| {
                // Tab bar
                self.show_tab_bar(ui);

                // Search / goto line bar
                self.show_search_bar(ui);
                self.show_goto_line_bar(ui);

                ui.add_space(0.0);

                // Editor area (takes remaining space minus status bar)
                let status_bar_height = 24.0;
                let available = ui.available_rect_before_wrap();
                let editor_rect = egui::Rect::from_min_max(
                    available.min,
                    egui::Pos2::new(available.max.x, available.max.y - status_bar_height),
                );

                let mut editor_ui = ui.new_child(egui::UiBuilder::new().max_rect(editor_rect).layout(egui::Layout::top_down(egui::Align::LEFT)));
                let auto_focus = !self.show_search && !self.show_goto_line && !self.command_palette.visible && self.confirm_close_tab.is_none();
                crate::ui::editor_view::show(&mut editor_ui, &mut self.editors[self.active_tab], &mut self.clipboard, &self.highlighter, auto_focus);

                // Status bar
                crate::ui::status_bar::show(ui, &self.editors[self.active_tab]);
            });

        // Unsaved changes confirmation dialog
        if let Some(tab_idx) = self.confirm_close_tab {
            let title = self.editors.get(tab_idx)
                .map(|e| e.title.clone())
                .unwrap_or_else(|| "file".into());
            let mut close_action: Option<bool> = None;

            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("\"{}\" has unsaved changes.", title));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save & Close").clicked() {
                            close_action = Some(true);
                        }
                        if ui.button("Discard").clicked() {
                            close_action = Some(false);
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_close_tab = None;
                        }
                    });
                });

            match close_action {
                Some(true) => {
                    // Save then close
                    let _ = self.editors[tab_idx].save();
                    self.force_close_tab(tab_idx);
                }
                Some(false) => {
                    self.force_close_tab(tab_idx);
                }
                None => {}
            }
        }

        ctx.request_repaint();
    }
}
