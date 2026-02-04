use arboard::Clipboard;
use eframe::egui;
use std::path::Path;
use std::process::Command;

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
const TAB_BAR_BG: egui::Color32 = egui::Color32::from_rgb(25, 25, 26);
const TAB_ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(37, 37, 38); // Should match generic editor bg
const TAB_INACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(45, 45, 48);
const TAB_HOVER_BG: egui::Color32 = egui::Color32::from_rgb(50, 50, 53);
const TAB_HEIGHT: f32 = 32.0; // Slightly taller for modern feel
const TAB_MIN_WIDTH: f32 = 120.0;
const TAB_MAX_WIDTH: f32 = 220.0;
const TAB_PADDING_X: f32 = 12.0;
const TAB_CLOSE_SIZE: f32 = 14.0;
const ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(0, 122, 204); // VS Code-ish blue active line

// GitInfo moved to ui::status_bar

#[derive(Clone, Copy)]
enum TabAction {
    Activate(usize),
    Close(usize),
    CloseOthers(usize),
    ReopenClosed,
    TogglePin(usize, bool),
    Reorder(usize, usize),
    NewTab,
}

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
    pub pending_close_others: Option<usize>,
    pub closed_tabs: Vec<Editor>,
    pub dragging_tab: Option<usize>,
    pub editor_theme: EditorThemeKind,
    git_info: Option<crate::ui::status_bar::GitInfo>,
    git_last_check: f64,
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
            pending_close_others: None,
            closed_tabs: Vec::new(),
            dragging_tab: None,
            editor_theme: EditorThemeKind::Monokai,
            git_info: None,
            git_last_check: 0.0,
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
            let closed = self.editors.remove(idx);
            self.closed_tabs.push(closed);
            if self.closed_tabs.len() > 50 {
                self.closed_tabs.remove(0);
            }
            if self.active_tab >= self.editors.len() {
                self.active_tab = self.editors.len() - 1;
            }
        }
        self.confirm_close_tab = None;
    }

    fn reopen_closed_tab(&mut self) {
        if let Some(editor) = self.closed_tabs.pop() {
            self.editors.push(editor);
            self.active_tab = self.editors.len() - 1;
        }
    }

    fn close_other_tabs(&mut self, keep_idx: usize) {
        if self.editors.len() <= 1 {
            return;
        }
        let mut idx = 0;
        let mut keep_idx = keep_idx;
        while idx < self.editors.len() {
            if idx == keep_idx || self.editors[idx].pinned {
                idx += 1;
                continue;
            }
            if self.editors[idx].modified {
                self.confirm_close_tab = Some(idx);
                self.pending_close_others = Some(keep_idx);
                return;
            }
            if idx < keep_idx {
                keep_idx = keep_idx.saturating_sub(1);
            }
            self.force_close_tab(idx);
        }
        self.pending_close_others = None;
    }

    fn pin_tab(&mut self, idx: usize, pinned: bool) {
        if let Some(editor) = self.editors.get_mut(idx) {
            editor.pinned = pinned;
        }
        let active_path = self.editors[self.active_tab].file_path.clone();
        let active_title = self.editors[self.active_tab].title.clone();
        let mut pinned_tabs = Vec::new();
        let mut regular_tabs = Vec::new();
        for editor in self.editors.drain(..) {
            if editor.pinned {
                pinned_tabs.push(editor);
            } else {
                regular_tabs.push(editor);
            }
        }
        pinned_tabs.append(&mut regular_tabs);
        self.editors = pinned_tabs;
        if let Some(path) = active_path {
            if let Some((idx, _)) = self
                .editors
                .iter()
                .enumerate()
                .find(|(_, e)| e.file_path == Some(path.clone()))
            {
                self.active_tab = idx;
            }
        } else if let Some((idx, _)) = self
            .editors
            .iter()
            .enumerate()
            .find(|(_, e)| e.title == active_title)
        {
            self.active_tab = idx;
        }
    }

    fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.editors.len() || to >= self.editors.len() {
            return;
        }
        let tab = self.editors.remove(from);
        self.editors.insert(to, tab);
        if self.active_tab == from {
            self.active_tab = to;
        } else if from < self.active_tab && to >= self.active_tab {
            self.active_tab = self.active_tab.saturating_sub(1);
        } else if from > self.active_tab && to <= self.active_tab {
            self.active_tab += 1;
        }
    }

    fn update_git_info(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.git_last_check < 1.0 {
            return;
        }
        self.git_last_check = now;
        let path = self
            .editors
            .get(self.active_tab)
            .and_then(|e| e.file_path.as_ref())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        self.git_info = read_git_info(path.as_deref());
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
                        ui.separator();
                        let minimap_enabled = self.active_editor().minimap_enabled;
                        if ui
                            .selectable_label(minimap_enabled, "Toggle Minimap")
                            .clicked()
                        {
                            self.active_editor().minimap_enabled = !minimap_enabled;
                            ui.close_menu();
                        }
                        if ui.button("Minimap Width +").clicked() {
                            let editor = self.active_editor();
                            editor.minimap_width = (editor.minimap_width + 10.0).clamp(80.0, 200.0);
                            ui.close_menu();
                        }
                        if ui.button("Minimap Width -").clicked() {
                            let editor = self.active_editor();
                            editor.minimap_width = (editor.minimap_width - 10.0).clamp(80.0, 200.0);
                            ui.close_menu();
                        }
                        if ui.button("Minimap Opacity +").clicked() {
                            let editor = self.active_editor();
                            editor.minimap_opacity = (editor.minimap_opacity + 0.1).clamp(0.2, 1.0);
                            ui.close_menu();
                        }
                        if ui.button("Minimap Opacity -").clicked() {
                            let editor = self.active_editor();
                            editor.minimap_opacity = (editor.minimap_opacity - 0.1).clamp(0.2, 1.0);
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("Theme"), |ui| {
                        for theme_option in [
                            EditorThemeKind::Dark,
                            EditorThemeKind::Light,
                            EditorThemeKind::Monokai,
                            EditorThemeKind::SolarizedDark,
                        ] {
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
            .inner_margin(egui::Margin::same(0.0)) // No margin, full bleed
            .show(ui, |ui| {
                let mut tab_rects: Vec<(usize, egui::Rect, bool)> = Vec::new();
                let mut tab_action: Option<TabAction> = None;
                let pointer_pos = ui.ctx().input(|i| i.pointer.latest_pos());
                let pointer_released = ui.ctx().input(|i| i.pointer.any_released());

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO; // Connect tabs

                    egui::ScrollArea::horizontal()
                        .id_salt("tabs_scroll")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::new(1.0, 0.0); // 1px gap

                                for i in 0..self.editors.len() {
                                    let (title, modified, pinned) = {
                                        let editor = &self.editors[i];
                                        (editor.title.clone(), editor.modified, editor.pinned)
                                    };
                                    let is_active = i == self.active_tab;
                                    let text = title.clone();

                                    let text_color = if is_active {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::from_rgb(180, 180, 180)
                                    };
                                    
                                    // Calculate Width
                                    let font = egui::FontId::proportional(12.5); // Slightly larger
                                    let text_width = ui.fonts(|f| {
                                        f.layout_no_wrap(text.clone(), font.clone(), text_color)
                                            .rect
                                            .width()
                                    });
                                    let tab_width =
                                        text_width + TAB_PADDING_X * 2.0 + TAB_CLOSE_SIZE + 8.0;
                                    if self.editors.len() <= 1 {
                                       // Even single tab might want a close button in modern designs, 
                                       // but let's keep it optional if space is tight? 
                                       // Actuall VS code usually shows it on hover.
                                       // For now, let's include space for it to avoid jumping.
                                    }
                                    let tab_width = tab_width.clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH);
                                    
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::Vec2::new(tab_width, TAB_HEIGHT),
                                        egui::Sense::click(),
                                    );
                                    tab_rects.push((i, rect, pinned));

                                    // Background
                                    let bg = if is_active {
                                        TAB_ACTIVE_BG
                                    } else if response.hovered() {
                                        TAB_HOVER_BG
                                    } else {
                                        TAB_INACTIVE_BG
                                    };
                                    
                                    // Custom Tab Painting
                                    let painter = ui.painter();
                                    
                                    // Main tab shape
                                    painter.rect_filled(rect, 0.0, bg);
                                    
                                    // Active top border
                                    if is_active {
                                        painter.rect_filled(
                                            egui::Rect::from_min_size(
                                                rect.min,
                                                egui::Vec2::new(rect.width(), 2.0),
                                            ),
                                            0.0,
                                            ACCENT_COLOR,
                                        );
                                    } else {
                                        // Separator line at bottom for inactive tabs implies the active one overlaps
                                        // But here we can just do nothing for now, simplest flat look
                                    }

                                    // Content Layout
                                    // Modified dot or Icon
                                    let mut text_x = rect.left() + TAB_PADDING_X;
                                    if modified {
                                        // A prettier "unsaved" circle
                                        painter.circle_filled(
                                            egui::Pos2::new(text_x + 3.0, rect.center().y),
                                            4.0,
                                            egui::Color32::WHITE, // Or generic fg
                                        );
                                        // And a smaller inner dot for "modified"
                                        painter.circle_filled(
                                            egui::Pos2::new(text_x + 3.0, rect.center().y),
                                            2.5,
                                            egui::Color32::from_rgb(255, 165, 90), // Orange
                                        );
                                        text_x += 14.0;
                                    } else {
                                         // Maybe an icon here? For now just text.
                                    }

                                    // File Name
                                    painter.text(
                                        egui::Pos2::new(text_x, rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        text,
                                        font.clone(),
                                        text_color,
                                    );

                                    // Interactions
                                    if response.clicked() {
                                        tab_action = Some(TabAction::Activate(i));
                                    }
                                    if response.middle_clicked() {
                                        tab_action = Some(TabAction::Close(i));
                                    }

                                    // Close Button - Show on hover or if active
                                    if response.hovered() || is_active {
                                        let close_rect = egui::Rect::from_min_size(
                                            egui::Pos2::new(
                                                rect.right() - TAB_CLOSE_SIZE - 6.0,
                                                rect.center().y - TAB_CLOSE_SIZE / 2.0,
                                            ),
                                            egui::Vec2::new(TAB_CLOSE_SIZE, TAB_CLOSE_SIZE),
                                        );
                                        
                                        // Check interaction specifically on the small close rect
                                        let close_resp = ui.interact(
                                            close_rect,
                                            ui.id().with(("tab_close", i)),
                                            egui::Sense::click()
                                        );
                                        
                                        let close_hovered = close_resp.hovered();
                                        
                                        // Draw 'x'
                                        // Rotate 45deg +
                                        let center = close_rect.center();
                                        if close_hovered {
                                             painter.rect_filled(close_rect, 2.0, egui::Color32::from_white_alpha(30));
                                        }
                                        
                                        let stroke = egui::Stroke::new(1.0, if close_hovered { egui::Color32::WHITE } else { egui::Color32::from_gray(150) });
                                        // Manually draw X for better control than text
                                        let r = TAB_CLOSE_SIZE / 3.0;
                                        painter.line_segment([
                                            center + egui::Vec2::new(-r, -r),
                                            center + egui::Vec2::new(r, r)
                                        ], stroke);
                                        painter.line_segment([
                                            center + egui::Vec2::new(-r, r),
                                            center + egui::Vec2::new(r, -r)
                                        ], stroke);

                                        if close_resp.clicked() {
                                            tab_action = Some(TabAction::Close(i));
                                        }
                                    }

                                    // Context Menu
                                    response.context_menu(|ui| {
                                        if ui.button("Close").clicked() {
                                            tab_action = Some(TabAction::Close(i));
                                            ui.close_menu();
                                        }
                                        if ui.button("Close Others").clicked() {
                                            tab_action = Some(TabAction::CloseOthers(i));
                                            ui.close_menu();
                                        }
                                        if ui.button("Reopen Closed Tab").clicked() {
                                            tab_action = Some(TabAction::ReopenClosed);
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        let pin_label = if pinned { "Unpin" } else { "Pin" };
                                        if ui.button(pin_label).clicked() {
                                            tab_action = Some(TabAction::TogglePin(i, !pinned));
                                            ui.close_menu();
                                        }
                                    });

                                    if response.drag_started() {
                                        self.dragging_tab = Some(i);
                                    }
                                }
                                
                                // New Tab Button (small)
                                let new_tab_resp = ui.allocate_response(egui::Vec2::new(32.0, TAB_HEIGHT), egui::Sense::click());
                                if new_tab_resp.hovered() {
                                    ui.painter().rect_filled(new_tab_resp.rect, 0.0, TAB_HOVER_BG);
                                }
                                ui.painter().text(
                                    new_tab_resp.rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "+",
                                    egui::FontId::proportional(16.0),
                                    egui::Color32::GRAY,
                                );
                                if new_tab_resp.clicked() {
                                    tab_action = Some(TabAction::NewTab);
                                }
                            });
                        });
                });

                if pointer_released {
                    if let (Some(drag_idx), Some(pos)) = (self.dragging_tab, pointer_pos) {
                        if let Some(target_idx) = find_drop_target(drag_idx, pos, &tab_rects) {
                            tab_action = Some(TabAction::Reorder(drag_idx, target_idx));
                        }
                    }
                    self.dragging_tab = None;
                }

                if let Some(action) = tab_action {
                    match action {
                        TabAction::Activate(idx) => self.active_tab = idx,
                        TabAction::Close(idx) => self.close_tab_idx(idx),
                        TabAction::CloseOthers(idx) => self.close_other_tabs(idx),
                        TabAction::ReopenClosed => self.reopen_closed_tab(),
                        TabAction::TogglePin(idx, pinned) => self.pin_tab(idx, pinned),
                        TabAction::Reorder(from, to) => self.move_tab(from, to),
                        TabAction::NewTab => self.new_tab(),
                    }
                }
            });
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui) {
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

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let query = self.search_input.clone();
                        self.active_editor().find_and_select(&query);
                        response.request_focus();
                    }

                    if ui
                        .add(egui::Button::new(egui::RichText::new("↓").size(14.0)))
                        .on_hover_text("Next Match")
                        .clicked()
                    {
                        let query = self.search_input.clone();
                        self.active_editor().find_and_select(&query);
                    }

                    // Toggles for options could go here

                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("\u{1F5D9}").size(14.0), // Cancel X
                        ).frame(false))
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
                            .add(egui::Button::new(
                                egui::RichText::new("All").size(12.0),
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

    fn show_breadcrumbs(&mut self, ui: &mut egui::Ui) {
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

impl eframe::App for LuxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());

        self.update_git_info(ctx);

        // Global shortcuts (handled before UI to avoid conflicts)
        if !self.command_palette.visible {
            self.handle_global_shortcuts(ctx);
        }

        // Command palette (rendered as overlay)
        if let Some(cmd) = self.command_palette.show(ctx) {
            self.command_palette.register_use(cmd);
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

                // Breadcrumbs
                self.show_breadcrumbs(ui);

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
                let search_query = if self.search_input.trim().is_empty() {
                    None
                } else {
                    Some(self.search_input.as_str())
                };
                crate::ui::editor_view::show(
                    &mut editor_ui,
                    &mut self.editors[self.active_tab],
                    &mut self.clipboard,
                    &self.highlighter,
                    &editor_theme,
                    search_query,
                    auto_focus,
                );

                // Status bar
                crate::ui::status_bar::show(
                    ui,
                    &mut self.editors[self.active_tab],
                    self.git_info.as_ref(),
                );
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
                            self.pending_close_others = None;
                        }
                    });
                });

            match close_action {
                Some(true) => {
                    // Save then close
                    let _ = self.editors[tab_idx].save();
                    self.force_close_tab(tab_idx);
                    if let Some(keep_idx) = self.pending_close_others {
                        self.close_other_tabs(keep_idx.min(self.editors.len().saturating_sub(1)));
                    }
                }
                Some(false) => {
                    self.force_close_tab(tab_idx);
                    if let Some(keep_idx) = self.pending_close_others {
                        self.close_other_tabs(keep_idx.min(self.editors.len().saturating_sub(1)));
                    }
                }
                None => {}
            }
        }

        ctx.request_repaint();
    }
}

fn find_drop_target(
    dragging_idx: usize,
    pointer: egui::Pos2,
    rects: &[(usize, egui::Rect, bool)],
) -> Option<usize> {
    let mut drag_pinned = None;
    for (idx, _, pinned) in rects {
        if *idx == dragging_idx {
            drag_pinned = Some(*pinned);
            break;
        }
    }
    let drag_pinned = drag_pinned?;
    let mut best: Option<(usize, f32)> = None;
    for (idx, rect, pinned) in rects {
        if *pinned != drag_pinned {
            continue;
        }
        let center_x = rect.center().x;
        let dist = (pointer.x - center_x).abs();
        if best.map(|(_, d)| dist < d).unwrap_or(true) {
            best = Some((*idx, dist));
        }
    }
    best.map(|(idx, _)| idx).filter(|idx| *idx != dragging_idx)
}

fn read_git_info(cwd: Option<&Path>) -> Option<crate::ui::status_bar::GitInfo> {
    let cwd = cwd?;

    let mut rev_cmd = Command::new("git");
    rev_cmd.arg("rev-parse").arg("--show-toplevel").current_dir(cwd);
    let rev_output = rev_cmd.output().ok()?;
    if !rev_output.status.success() {
        return None;
    }
    let toplevel = String::from_utf8_lossy(&rev_output.stdout).trim().to_string();
    if toplevel.is_empty() {
        return None;
    }
    let toplevel = Path::new(&toplevel);
    if !cwd.starts_with(toplevel) {
        return None;
    }

    let mut cmd = Command::new("git");
    cmd.arg("status").arg("-sb");
    cmd.current_dir(cwd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let first = lines.next()?.trim();
    if !first.starts_with("## ") {
        return None;
    }
    let mut branch = first.trim_start_matches("## ").to_string();
    let mut ahead = 0usize;
    let mut behind = 0usize;
    if let Some((name, rest)) = branch.clone().split_once("...") {
        branch = name.to_string();
        if let Some(start) = rest.find('[') {
            if let Some(end) = rest.find(']') {
                let stats = &rest[start + 1..end];
                for part in stats.split(',') {
                    let part = part.trim();
                    if let Some(value) = part.strip_prefix("ahead ") {
                        ahead = value.parse().unwrap_or(0);
                    } else if let Some(value) = part.strip_prefix("behind ") {
                        behind = value.parse().unwrap_or(0);
                    }
                }
            }
        }
    }
    let dirty = lines.next().is_some();
    Some(crate::ui::status_bar::GitInfo {
        branch,
        ahead,
        behind,
        dirty,
    })
}
