use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::editor::{Editor, LINE_HEIGHT};
use crate::syntax::SyntaxHighlighter;
use arboard::Clipboard;

const BG_COLOR: Color32 = Color32::from_rgb(30, 30, 30);
const TEXT_COLOR: Color32 = Color32::from_rgb(212, 212, 212);
const CURSOR_COLOR: Color32 = Color32::from_rgb(248, 248, 240);
const SELECTION_BG: Color32 = Color32::from_rgba_premultiplied(60, 100, 150, 120);
const LINE_NUM_COLOR: Color32 = Color32::from_rgb(90, 90, 90);
const LINE_NUM_ACTIVE_COLOR: Color32 = Color32::from_rgb(180, 180, 180);
const GUTTER_BG: Color32 = Color32::from_rgb(37, 37, 37);
const ACTIVE_LINE_BG: Color32 = Color32::from_rgb(40, 40, 40);
const FONT_SIZE: f32 = 14.0;
const GUTTER_PADDING: f32 = 16.0;

pub struct EditorMetrics {
    pub char_width: f32,
    pub line_height: f32,
    pub gutter_width: f32,
    pub font_id: FontId,
}

impl EditorMetrics {
    pub fn compute(ui: &egui::Ui, line_count: usize) -> Self {
        let font_id = FontId::monospace(FONT_SIZE);
        let char_width = ui.fonts(|f| {
            let galley = f.layout_no_wrap("M".to_string(), font_id.clone(), TEXT_COLOR);
            galley.size().x
        });
        let digits = format!("{}", line_count).len().max(3);
        let gutter_width = char_width * digits as f32 + GUTTER_PADDING * 2.0;

        Self {
            char_width,
            line_height: LINE_HEIGHT,
            gutter_width,
            font_id,
        }
    }
}

/// Renders the editor area and handles input. Returns true if content changed.
pub fn show(ui: &mut egui::Ui, editor: &mut Editor, clipboard: &mut Option<Clipboard>, highlighter: &SyntaxHighlighter, auto_focus: bool) -> bool {
    let mut changed = false;
    let metrics = EditorMetrics::compute(ui, editor.line_count());
    let available = ui.available_rect_before_wrap();

    // Background
    ui.painter()
        .rect_filled(available, 0.0, BG_COLOR);

    // Allocate the full area as an interactive region
    let response = ui.allocate_rect(available, Sense::click_and_drag());

    // Request focus on click/drag, or automatically when no overlay is active
    if response.clicked() || response.dragged() || auto_focus {
        ui.memory_mut(|m| m.request_focus(response.id));
    }

    let has_focus = ui.memory(|m| m.has_focus(response.id));

    // Handle mouse click -> set cursor position
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let (line, col) = screen_to_editor_pos(pos, &available, &metrics, editor);
            let ctrl = ui.input(|i| i.modifiers.command);
            if ctrl {
                editor.add_cursor_at(line, col);
            } else {
                editor.cursors.truncate(1);
                editor.cursors[0].pos = crate::editor::Position::new(line, col);
                editor.cursors[0].anchor = None;
                editor.cursors[0].desired_col = col;
            }
        }
    }

    // Handle double-click -> select word
    if response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let (line, col) = screen_to_editor_pos(pos, &available, &metrics, editor);
            editor.cursors.truncate(1);
            editor.cursors[0].pos = crate::editor::Position::new(line, col);
            editor.cursors[0].anchor = None;
            // select_next_occurrence on first call selects the word under cursor
            editor.select_next_occurrence();
        }
    }

    // Handle drag -> extend selection
    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let (line, col) = screen_to_editor_pos(pos, &available, &metrics, editor);
            let cursor = &mut editor.cursors[0];
            if cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos);
            }
            cursor.pos = crate::editor::Position::new(line, col);
            cursor.desired_col = col;
        }
    }

    // Handle scroll
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta != 0.0 {
        editor.scroll_y = (editor.scroll_y - scroll_delta).max(0.0);
        let max_scroll = (editor.line_count() as f32 * metrics.line_height - available.height())
            .max(0.0);
        editor.scroll_y = editor.scroll_y.min(max_scroll);
    }

    // Handle keyboard input
    if has_focus {
        changed = handle_keyboard(ui, editor, clipboard);
    }

    // Render visible lines
    render_lines(ui, &available, editor, &metrics, highlighter);

    // Ensure cursor is visible (auto-scroll)
    if !editor.cursors.is_empty() {
        let primary = &editor.cursors[0];
        let cursor_y = primary.pos.line as f32 * metrics.line_height;

        if cursor_y < editor.scroll_y {
            editor.scroll_y = cursor_y;
        } else if cursor_y + metrics.line_height > editor.scroll_y + available.height() {
            editor.scroll_y = cursor_y + metrics.line_height - available.height();
        }
    }

    changed
}

fn screen_to_editor_pos(
    screen_pos: Pos2,
    rect: &Rect,
    metrics: &EditorMetrics,
    editor: &Editor,
) -> (usize, usize) {
    let rel_y = screen_pos.y - rect.top() + editor.scroll_y;
    let rel_x = screen_pos.x - rect.left() - metrics.gutter_width - 4.0 + editor.scroll_x;

    let line = (rel_y / metrics.line_height).floor().max(0.0) as usize;
    let line = line.min(editor.line_count().saturating_sub(1));

    let col = (rel_x / metrics.char_width).round().max(0.0) as usize;
    let line_text = editor.line_text(line);
    let col = col.min(line_text.chars().count());

    (line, col)
}

fn handle_keyboard(ui: &mut egui::Ui, editor: &mut Editor, clipboard: &mut Option<Clipboard>) -> bool {
    let mut changed = false;
    let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
    let time = ui.input(|i| i.time);

    for event in &events {
        match event {
            egui::Event::Text(text) => {
                let ctrl = ui.input(|i| i.modifiers.command);
                if !ctrl {
                    editor.insert_text(text);
                    changed = true;
                }
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let shift = modifiers.shift;
                let ctrl = modifiers.command;

                match key {
                    egui::Key::Backspace if ctrl => {
                        editor.delete_word_backward();
                        changed = true;
                    }
                    egui::Key::Backspace => {
                        editor.backspace();
                        changed = true;
                    }
                    egui::Key::Delete if ctrl => {
                        editor.delete_word_forward();
                        changed = true;
                    }
                    egui::Key::Delete => {
                        editor.delete_forward();
                        changed = true;
                    }
                    egui::Key::Enter => {
                        editor.insert_newline();
                        changed = true;
                    }
                    egui::Key::Tab => {
                        editor.insert_tab();
                        changed = true;
                    }
                    egui::Key::ArrowLeft if ctrl => editor.move_word_left(shift),
                    egui::Key::ArrowRight if ctrl => editor.move_word_right(shift),
                    egui::Key::ArrowLeft => editor.move_left(shift),
                    egui::Key::ArrowRight => editor.move_right(shift),
                    egui::Key::ArrowUp => editor.move_up(shift),
                    egui::Key::ArrowDown => editor.move_down(shift),
                    egui::Key::Home if ctrl => editor.move_to_start(shift),
                    egui::Key::End if ctrl => editor.move_to_end(shift),
                    egui::Key::Home => editor.move_home(shift),
                    egui::Key::End => editor.move_end(shift),
                    egui::Key::PageUp => {
                        let visible = (ui.available_height() / LINE_HEIGHT) as usize;
                        editor.move_page_up(shift, visible.max(1));
                    }
                    egui::Key::PageDown => {
                        let visible = (ui.available_height() / LINE_HEIGHT) as usize;
                        editor.move_page_down(shift, visible.max(1));
                    }
                    egui::Key::A if ctrl => editor.select_all(),
                    egui::Key::D if ctrl => editor.select_next_occurrence(),
                    egui::Key::C if ctrl => {
                        if let Some(cb) = clipboard.as_mut() {
                            let text = editor.copy_text();
                            let _ = cb.set_text(&text);
                        }
                    }
                    egui::Key::X if ctrl => {
                        if let Some(cb) = clipboard.as_mut() {
                            let text = editor.cut_text();
                            let _ = cb.set_text(&text);
                            changed = true;
                        }
                    }
                    egui::Key::V if ctrl => {
                        if let Some(cb) = clipboard.as_mut() {
                            if let Ok(text) = cb.get_text() {
                                editor.insert_text(&text);
                                changed = true;
                            }
                        }
                    }
                    egui::Key::Z if ctrl && shift => {
                        editor.redo();
                        changed = true;
                    }
                    egui::Key::Z if ctrl => {
                        editor.undo();
                        changed = true;
                    }
                    egui::Key::Y if ctrl => {
                        editor.redo();
                        changed = true;
                    }
                    egui::Key::Escape => editor.clear_extra_cursors(),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    if changed {
        editor.last_edit_time = time;
    }

    changed
}

fn render_lines(
    ui: &egui::Ui,
    rect: &Rect,
    editor: &Editor,
    metrics: &EditorMetrics,
    highlighter: &SyntaxHighlighter,
) {
    let painter = ui.painter_at(*rect);
    let time = ui.input(|i| i.time);

    let since_edit = time - editor.last_edit_time;
    let cursor_visible = since_edit < 0.5 || ((since_edit * 2.0) as u64 % 2 == 0);

    let first_line = (editor.scroll_y / metrics.line_height).floor() as usize;
    let visible_count = (rect.height() / metrics.line_height).ceil() as usize + 1;
    let last_line = (first_line + visible_count).min(editor.line_count());

    // Syntax highlighting for visible lines
    let full_text = editor.rope.to_string();
    let highlighted = highlighter.highlight_lines(
        &full_text,
        editor.file_path.as_deref(),
        first_line,
        last_line,
    );

    // Collect active cursor lines
    let active_lines: Vec<usize> = editor.cursors.iter().map(|c| c.pos.line).collect();

    // Draw gutter background
    let gutter_rect = Rect::from_min_size(
        rect.left_top(),
        Vec2::new(metrics.gutter_width, rect.height()),
    );
    painter.rect_filled(gutter_rect, 0.0, GUTTER_BG);

    // Draw separator line
    painter.line_segment(
        [
            Pos2::new(rect.left() + metrics.gutter_width, rect.top()),
            Pos2::new(rect.left() + metrics.gutter_width, rect.bottom()),
        ],
        Stroke::new(1.0, Color32::from_rgb(50, 50, 50)),
    );

    for line_idx in first_line..last_line {
        let y = rect.top() + (line_idx as f32) * metrics.line_height - editor.scroll_y;

        // Active line highlight
        if active_lines.contains(&line_idx) {
            let line_rect = Rect::from_min_size(
                Pos2::new(rect.left() + metrics.gutter_width, y),
                Vec2::new(rect.width() - metrics.gutter_width, metrics.line_height),
            );
            painter.rect_filled(line_rect, 0.0, ACTIVE_LINE_BG);
        }

        // Line number
        let ln_color = if active_lines.contains(&line_idx) {
            LINE_NUM_ACTIVE_COLOR
        } else {
            LINE_NUM_COLOR
        };
        let ln_text = format!("{}", line_idx + 1);
        painter.text(
            Pos2::new(rect.left() + metrics.gutter_width - GUTTER_PADDING / 2.0, y + metrics.line_height / 2.0),
            egui::Align2::RIGHT_CENTER,
            &ln_text,
            metrics.font_id.clone(),
            ln_color,
        );

        // Selection highlighting
        for cursor in &editor.cursors {
            if let Some((sel_start, sel_end)) = cursor.selection_ordered() {
                draw_selection(
                    &painter,
                    rect,
                    line_idx,
                    &sel_start,
                    &sel_end,
                    metrics,
                    editor,
                );
            }
        }

        // Line text (syntax highlighted)
        let hl_idx = line_idx - first_line;
        let text_x_base = rect.left() + metrics.gutter_width + 4.0 - editor.scroll_x;
        if let Some(tokens) = highlighted.get(hl_idx) {
            let mut offset_x = text_x_base;
            for token in tokens {
                if !token.text.is_empty() {
                    painter.text(
                        Pos2::new(offset_x, y + metrics.line_height / 2.0),
                        egui::Align2::LEFT_CENTER,
                        &token.text,
                        metrics.font_id.clone(),
                        token.color,
                    );
                    offset_x += token.text.chars().count() as f32 * metrics.char_width;
                }
            }
        } else {
            let text = editor.line_text(line_idx);
            if !text.is_empty() {
                painter.text(
                    Pos2::new(text_x_base, y + metrics.line_height / 2.0),
                    egui::Align2::LEFT_CENTER,
                    &text,
                    metrics.font_id.clone(),
                    TEXT_COLOR,
                );
            }
        }

        // Cursors on this line
        if cursor_visible {
            for cursor in &editor.cursors {
                if cursor.pos.line == line_idx {
                    let cx = rect.left()
                        + metrics.gutter_width
                        + 4.0
                        + cursor.pos.col as f32 * metrics.char_width
                        - editor.scroll_x;
                    let cursor_rect = Rect::from_min_size(
                        Pos2::new(cx, y + 1.0),
                        Vec2::new(2.0, metrics.line_height - 2.0),
                    );
                    painter.rect_filled(cursor_rect, 0.0, CURSOR_COLOR);
                }
            }
        }
    }
}

fn draw_selection(
    painter: &egui::Painter,
    rect: &Rect,
    line_idx: usize,
    sel_start: &crate::editor::Position,
    sel_end: &crate::editor::Position,
    metrics: &EditorMetrics,
    editor: &Editor,
) {
    if line_idx < sel_start.line || line_idx > sel_end.line {
        return;
    }

    let y = rect.top() + line_idx as f32 * metrics.line_height - editor.scroll_y;
    let text_x = rect.left() + metrics.gutter_width + 4.0;

    let start_col = if line_idx == sel_start.line {
        sel_start.col
    } else {
        0
    };
    let end_col = if line_idx == sel_end.line {
        sel_end.col
    } else {
        editor.line_text(line_idx).chars().count()
    };

    if start_col >= end_col && line_idx == sel_start.line && line_idx == sel_end.line {
        return;
    }

    let x1 = text_x + start_col as f32 * metrics.char_width - editor.scroll_x;
    let x2 = text_x + end_col as f32 * metrics.char_width - editor.scroll_x;

    let sel_rect = Rect::from_min_size(
        Pos2::new(x1, y),
        Vec2::new(x2 - x1, metrics.line_height),
    );
    painter.rect_filled(sel_rect, 0.0, SELECTION_BG);
}
