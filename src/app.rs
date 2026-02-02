use arboard::Clipboard;
use eframe::egui;

use crate::editor::Editor;
use crate::syntax::SyntaxHighlighter;
use crate::ui::command_palette::{CommandId, CommandPalette};
use crate::ui::editor_view::EditorThemeKind;

const WINDOW_BG: egui::Color32 = egui::Color32::from_rgb(36, 37, 38);
const MENU_BG: egui::Color32 = egui::Color32::from_rgb(45, 45, 47);
const MENU_STROKE: egui::Stroke = egui::Stroke {
    width: 1.0,
    color: egui::Color32::from_rgb(65, 65, 67),
};
const TAB_BAR_BG: egui::Color32 = egui::Color32::from_rgb(33, 35, 37);
const TAB_BAR_BORDER: egui::Color32 = egui::Color32::from_rgb(25, 26, 28);
const TAB_ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(50, 52, 56);
const TAB_INACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(40, 41, 44);
const TAB_HOVER_BG: egui::Color32 = egui::Color32::from_rgb(47, 49, 53);
const TAB_HEIGHT: f32 = 28.0;
const TAB_MIN_WIDTH: f32 = 100.0;
const TAB_MAX_WIDTH: f32 = 200.0;
const TAB_PADDING_X: f32 = 14.0;
const TAB_CLOSE_SIZE: f32 = 12.0;
const ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 149, 89);

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
    pub editor_theme: EditorThemeKind,
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
            editor_theme: EditorThemeKind::Monokai,
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
        let should_save =
            ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S));
        let should_save_as =
            ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S));

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

    fn show_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("lux_menu_bar")
            .frame(
                egui::Frame::none()
                    .fill(MENU_BG)
                    .stroke(MENU_STROKE)
                    .inner_margin(egui::Margin::symmetric(12.0, 6.0)),
            )
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    let rich_label = |label: &str| {
                        egui::RichText::new(label)
                            .color(egui::Color32::from_rgb(215, 215, 215))
                            .size(13.0)
                    };

                    ui.menu_button(rich_label("File"), |ui| {
                        if ui.button("New Tab\tCtrl+N").clicked() {
                            self.new_tab();
                            ui.close_menu();
                        }
                        if ui.button("Open...\tCtrl+O").clicked() {
                            self.open_file();
                            ui.close_menu();
                        }
                        if ui.button("Save\tCtrl+S").clicked() {
                            self.save_file();
                            ui.close_menu();
                        }
                        if ui.button("Save As...\tCtrl+Shift+S").clicked() {
                            self.save_file_as();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Close Tab\tCtrl+W").clicked() {
                            self.close_tab();
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("Edit"), |ui| {
                        if ui.button("Undo\tCtrl+Z").clicked() {
                            self.active_editor().undo();
                            ui.close_menu();
                        }
                        if ui.button("Redo\tCtrl+Shift+Z").clicked() {
                            self.active_editor().redo();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Cut\tCtrl+X").clicked() {
                            let text = self.active_editor().cut_text();
                            if let Some(cb) = self.clipboard.as_mut() {
                                let _ = cb.set_text(&text);
                            }
                            ui.close_menu();
                        }
                        if ui.button("Copy\tCtrl+C").clicked() {
                            let text = self.active_editor().copy_text();
                            if let Some(cb) = self.clipboard.as_mut() {
                                let _ = cb.set_text(&text);
                            }
                            ui.close_menu();
                        }
                        if ui.button("Paste\tCtrl+V").clicked() {
                            let mut paste = None;
                            if let Some(cb) = self.clipboard.as_mut() {
                                if let Ok(text) = cb.get_text() {
                                    paste = Some(text);
                                }
                            }
                            if let Some(text) = paste {
                                self.active_editor().insert_text(&text);
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Select All\tCtrl+A").clicked() {
                            self.active_editor().select_all();
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("Find"), |ui| {
                        if ui.button("Find\tCtrl+F").clicked() {
                            self.show_search = true;
                            self.show_replace = false;
                            ui.close_menu();
                        }
                        if ui.button("Replace\tCtrl+H").clicked() {
                            self.show_search = true;
                            self.show_replace = true;
                            ui.close_menu();
                        }
                        if ui.button("Go to Line\tCtrl+G").clicked() {
                            self.show_goto_line = true;
                            self.show_search = false;
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("View"), |ui| {
                        if ui.button("Command Palette\tCtrl+Shift+P").clicked() {
                            self.command_palette.toggle();
                            ui.close_menu();
                        }
                        if ui.button("Toggle Search Panel").clicked() {
                            self.show_search = !self.show_search;
                            ui.close_menu();
                        }
                        if ui.button("Toggle Go To Line").clicked() {
                            self.show_goto_line = !self.show_goto_line;
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("Theme"), |ui| {
                        for theme_option in
                            [EditorThemeKind::Monokai, EditorThemeKind::SolarizedDark]
                        {
                            let selected = self.editor_theme == theme_option;
                            let label = theme_option.name();
                            if ui.selectable_label(selected, label).clicked() {
                                self.editor_theme = theme_option;
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
    }

    fn show_tab_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(TAB_BAR_BG)
            .stroke(egui::Stroke::new(1.0, TAB_BAR_BORDER))
            .inner_margin(egui::Margin::symmetric(8.0, 2.0))
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("tabs_scroll")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for i in 0..self.editors.len() {
                                let editor = &self.editors[i];
                                let is_active = i == self.active_tab;
                                let mut text = editor.title.clone();
                                if editor.modified {
                                    text.push('*');
                                }

                                let text_color = if is_active {
                                    egui::Color32::from_rgb(230, 230, 230)
                                } else {
                                    egui::Color32::from_rgb(170, 170, 170)
                                };
                                let font = egui::FontId::proportional(12.0);
                                let text_width = ui.fonts(|f| {
                                    f.layout_no_wrap(text.clone(), font.clone(), text_color)
                                        .rect
                                        .width()
                                });
                                let mut tab_width =
                                    text_width + TAB_PADDING_X * 2.0 + TAB_CLOSE_SIZE + 6.0;
                                if self.editors.len() <= 1 {
                                    tab_width -= TAB_CLOSE_SIZE;
                                }
                                let tab_width = tab_width.clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH);
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::Vec2::new(tab_width, TAB_HEIGHT),
                                    egui::Sense::click(),
                                );

                                let bg = if is_active {
                                    TAB_ACTIVE_BG
                                } else if response.hovered() {
                                    TAB_HOVER_BG
                                } else {
                                    TAB_INACTIVE_BG
                                };
                                let rounding = egui::Rounding::same(4.0);
                                ui.painter().rect_filled(rect, rounding, bg);
                                ui.painter().rect_stroke(
                                    rect,
                                    rounding,
                                    egui::Stroke::new(1.0, TAB_BAR_BORDER),
                                );
                                if is_active {
                                    ui.painter().rect_filled(
                                        egui::Rect::from_min_size(
                                            egui::Pos2::new(rect.left(), rect.top()),
                                            egui::Vec2::new(rect.width(), 2.0),
                                        ),
                                        egui::Rounding::ZERO,
                                        ACCENT_COLOR,
                                    );
                                }

                                let text_pos =
                                    egui::Pos2::new(rect.left() + TAB_PADDING_X, rect.center().y);
                                ui.painter().text(
                                    text_pos,
                                    egui::Align2::LEFT_CENTER,
                                    text,
                                    font.clone(),
                                    text_color,
                                );

                                if response.clicked() {
                                    self.active_tab = i;
                                }
                                if response.middle_clicked() && self.editors.len() > 1 {
                                    self.close_tab_idx(i);
                                    break;
                                }

                                if self.editors.len() > 1 {
                                    let close_rect = egui::Rect::from_min_size(
                                        egui::Pos2::new(
                                            rect.right() - TAB_CLOSE_SIZE - 4.0,
                                            rect.center().y - TAB_CLOSE_SIZE / 2.0,
                                        ),
                                        egui::Vec2::new(TAB_CLOSE_SIZE, TAB_CLOSE_SIZE),
                                    );
                                    let close_resp = ui.interact(
                                        close_rect,
                                        ui.id().with(("tab_close", i)),
                                        egui::Sense::click(),
                                    );
                                    let mut close_color = egui::Color32::from_rgb(150, 150, 150);
                                    if close_resp.hovered() {
                                        close_color = egui::Color32::from_rgb(255, 94, 94);
                                    }
                                    ui.painter().text(
                                        close_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "×",
                                        egui::FontId::proportional(11.5),
                                        close_color,
                                    );
                                    if close_resp.clicked() {
                                        self.close_tab_idx(i);
                                        break;
                                    }
                                }

                                ui.add_space(4.0);
                            }

                            let new_tab_resp = ui.add_sized(
                                [28.0, TAB_HEIGHT],
                                egui::Button::new(
                                    egui::RichText::new("+")
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(190, 190, 190)),
                                )
                                .frame(false),
                            );
                            if new_tab_resp.clicked() {
                                self.new_tab();
                            }
                        });
                    });
            });
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_search {
            return;
        }

        ui.add_space(6.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(52, 53, 55))
            .rounding(egui::Rounding::same(6.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(65, 65, 70)))
            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Find:")
                            .color(egui::Color32::from_rgb(220, 220, 220))
                            .size(13.0),
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_input)
                            .desired_width(250.0)
                            .font(egui::FontId::monospace(13.0))
                            .text_color(egui::Color32::WHITE)
                            .hint_text("Search..."),
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
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
                        .add(egui::Button::new(
                            egui::RichText::new("\u{2715}").size(12.0),
                        ))
                        .clicked()
                    {
                        self.show_search = false;
                        self.show_replace = false;
                    }
                });
            });

        // Replace row
        if self.show_replace {
            ui.add_space(4.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(52, 53, 55))
                .rounding(egui::Rounding::same(6.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(65, 65, 70)))
                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Replace:")
                                .color(egui::Color32::from_rgb(220, 220, 220))
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
                            .add(egui::Button::new(
                                egui::RichText::new("Replace All").size(12.0),
                            ))
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

    fn show_goto_line_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_goto_line {
            return;
        }

        ui.add_space(6.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(52, 53, 55))
            .rounding(egui::Rounding::same(6.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(65, 65, 70)))
            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Go to Line:")
                            .color(egui::Color32::from_rgb(220, 220, 220))
                            .size(13.0),
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.goto_line_input)
                            .desired_width(100.0)
                            .font(egui::FontId::monospace(13.0))
                            .text_color(egui::Color32::WHITE)
                            .hint_text("Line number"),
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
                });
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

        self.show_menu_bar(ctx);

        // Main panel
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(WINDOW_BG)
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

                let mut editor_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(editor_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                let auto_focus = !self.show_search
                    && !self.show_goto_line
                    && !self.command_palette.visible
                    && self.confirm_close_tab.is_none();
                let editor_theme = self.editor_theme.palette();
                crate::ui::editor_view::show(
                    &mut editor_ui,
                    &mut self.editors[self.active_tab],
                    &mut self.clipboard,
                    &self.highlighter,
                    &editor_theme,
                    auto_focus,
                );

                // Status bar
                crate::ui::status_bar::show(ui, &self.editors[self.active_tab]);
            });

        // Unsaved changes confirmation dialog
        if let Some(tab_idx) = self.confirm_close_tab {
            let title = self
                .editors
                .get(tab_idx)
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
