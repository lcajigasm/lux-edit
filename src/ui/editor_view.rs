use eframe::egui::{self, Color32, FontId, Pos2, Rect, Rgba, Sense, Stroke, Vec2};

use crate::editor::{Editor, LINE_HEIGHT};
use crate::syntax::{StyledToken, SyntaxHighlighter};
use arboard::Clipboard;

const MINIMAP_WIDTH: f32 = 120.0;
const MINIMAP_SPACING: f32 = 8.0;
const MIN_EDITOR_WIDTH_FOR_MINIMAP: f32 = 600.0;

#[derive(Clone)]
pub struct EditorTheme {
    pub background: Color32,
    pub text: Color32,
    pub cursor: Color32,
    pub selection: Color32,
    pub line_num: Color32,
    pub line_num_active: Color32,
    pub gutter_bg: Color32,
    pub gutter_divider: Color32,
    pub active_line: Color32,
    pub minimap_bg: Color32,
    pub minimap_fg: Color32,
    pub minimap_viewport: Color32,
    pub minimap_border: Color32,
    pub font_size: f32,
    pub gutter_padding: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditorThemeKind {
    Monokai,
    SolarizedDark,
}

impl EditorThemeKind {
    pub fn name(&self) -> &'static str {
        match self {
            EditorThemeKind::Monokai => "Monokai",
            EditorThemeKind::SolarizedDark => "Solarized Dark",
        }
    }

    pub fn palette(&self) -> EditorTheme {
        match self {
            EditorThemeKind::Monokai => EditorTheme {
                background: Color32::from_rgb(39, 40, 34),
                text: Color32::from_rgb(248, 248, 242),
                cursor: Color32::from_rgb(255, 197, 109),
                selection: Color32::from_rgba_premultiplied(73, 72, 62, 180),
                line_num: Color32::from_rgb(143, 144, 138),
                line_num_active: Color32::from_rgb(230, 231, 224),
                gutter_bg: Color32::from_rgb(47, 48, 43),
                gutter_divider: Color32::from_rgb(35, 35, 30),
                active_line: Color32::from_rgb(62, 63, 56),
                minimap_bg: Color32::from_rgb(32, 33, 29),
                minimap_fg: Color32::from_rgba_premultiplied(120, 120, 120, 140),
                minimap_viewport: Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                minimap_border: Color32::from_rgba_unmultiplied(255, 255, 255, 25),
                font_size: 15.0,
                gutter_padding: 18.0,
            },
            EditorThemeKind::SolarizedDark => EditorTheme {
                background: Color32::from_rgb(0x00, 0x2b, 0x36),
                text: Color32::from_rgb(0xee, 0xe8, 0xd5),
                cursor: Color32::from_rgb(0xfd, 0xf6, 0xe3),
                selection: Color32::from_rgba_premultiplied(38, 139, 210, 120),
                line_num: Color32::from_rgb(88, 110, 117),
                line_num_active: Color32::from_rgb(203, 213, 214),
                gutter_bg: Color32::from_rgb(12, 44, 52),
                gutter_divider: Color32::from_rgb(8, 32, 39),
                active_line: Color32::from_rgb(7, 54, 66),
                minimap_bg: Color32::from_rgb(6, 35, 41),
                minimap_fg: Color32::from_rgba_premultiplied(69, 147, 168, 150),
                minimap_viewport: Color32::from_rgba_unmultiplied(238, 232, 213, 30),
                minimap_border: Color32::from_rgba_unmultiplied(147, 161, 161, 40),
                font_size: 15.0,
                gutter_padding: 18.0,
            },
        }
    }
}

pub struct EditorMetrics {
    pub char_width: f32,
    pub line_height: f32,
    pub gutter_width: f32,
    pub font_id: FontId,
}

impl EditorMetrics {
    pub fn compute(ui: &egui::Ui, line_count: usize, theme: &EditorTheme) -> Self {
        let font_id = FontId::monospace(theme.font_size);
        let text_color = theme.text;
        let char_width = ui.fonts(|f| {
            let galley = f.layout_no_wrap("M".to_string(), font_id.clone(), text_color);
            galley.size().x
        });
        let digits = format!("{}", line_count).len().max(3);
        let gutter_width = char_width * digits as f32 + theme.gutter_padding * 2.0;

        Self {
            char_width,
            line_height: LINE_HEIGHT,
            gutter_width,
            font_id,
        }
    }
}

/// Renders the editor area and handles input. Returns true if content changed.
pub fn show(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    clipboard: &mut Option<Clipboard>,
    highlighter: &SyntaxHighlighter,
    theme: &EditorTheme,
    auto_focus: bool,
) -> bool {
    let mut changed = false;
    let metrics = EditorMetrics::compute(ui, editor.line_count(), theme);
    let available = ui.available_rect_before_wrap();
    let mut minimap_rect = if available.width() > MIN_EDITOR_WIDTH_FOR_MINIMAP {
        Some(Rect::from_min_size(
            Pos2::new(available.max.x - MINIMAP_WIDTH, available.top()),
            Vec2::new(MINIMAP_WIDTH, available.height()),
        ))
    } else {
        None
    };
    let mut editor_rect = if let Some(mini) = minimap_rect {
        Rect::from_min_max(
            available.min,
            Pos2::new(mini.left() - MINIMAP_SPACING, available.max.y),
        )
    } else {
        available
    };
    if editor_rect.width() < 200.0 {
        minimap_rect = None;
        editor_rect = available;
    }

    // Background layers
    ui.painter().rect_filled(editor_rect, 0.0, theme.background);
    if let Some(rect) = minimap_rect {
        ui.painter().rect_filled(rect, 6.0, theme.minimap_bg);
        ui.painter()
            .rect_stroke(rect, 6.0, Stroke::new(1.0, theme.minimap_border));
    }

    // Allocate interactive regions
    let response = ui.allocate_rect(editor_rect, Sense::click_and_drag());
    let minimap_response = minimap_rect.map(|rect| ui.allocate_rect(rect, Sense::click_and_drag()));

    // Request focus on click/drag, or automatically when no overlay is active
    if response.clicked() || response.dragged() || auto_focus {
        ui.memory_mut(|m| m.request_focus(response.id));
    }

    let has_focus = ui.memory(|m| m.has_focus(response.id));

    // Handle mouse click -> set cursor position
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let (line, col) = screen_to_editor_pos(pos, &editor_rect, &metrics, editor);
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
            let (line, col) = screen_to_editor_pos(pos, &editor_rect, &metrics, editor);
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
            let (line, col) = screen_to_editor_pos(pos, &editor_rect, &metrics, editor);
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
        let max_scroll =
            (editor.line_count() as f32 * metrics.line_height - editor_rect.height()).max(0.0);
        editor.scroll_y = editor.scroll_y.min(max_scroll);
    }

    // Handle keyboard input
    if has_focus {
        changed = handle_keyboard(ui, editor, clipboard);
    }

    let full_text = editor.rope.to_string();

    // Render visible lines
    render_lines(
        ui,
        &editor_rect,
        editor,
        &metrics,
        highlighter,
        theme,
        &full_text,
    );

    let visible_lines = (editor_rect.height() / metrics.line_height).ceil() as usize;
    let visible_lines = visible_lines.max(1);
    if let (Some(rect), Some(resp)) = (minimap_rect, minimap_response) {
        let minimap_tokens = highlighter.highlight_lines(
            &full_text,
            editor.file_path.as_deref(),
            0,
            editor.line_count(),
        );
        render_minimap(
            ui,
            rect,
            editor,
            theme,
            visible_lines,
            metrics.line_height,
            &minimap_tokens,
        );
        if (resp.clicked() || resp.dragged()) && resp.contains_pointer() {
            if let Some(pos) = resp.interact_pointer_pos() {
                scroll_to_minimap_pos(
                    pos,
                    rect,
                    editor,
                    metrics.line_height,
                    editor_rect.height(),
                    visible_lines,
                );
            }
        }
    }

    // Ensure cursor is visible (auto-scroll)
    if !editor.cursors.is_empty() {
        let primary = &editor.cursors[0];
        let cursor_y = primary.pos.line as f32 * metrics.line_height;

        if cursor_y < editor.scroll_y {
            editor.scroll_y = cursor_y;
        } else if cursor_y + metrics.line_height > editor.scroll_y + editor_rect.height() {
            editor.scroll_y = cursor_y + metrics.line_height - editor_rect.height();
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

fn handle_keyboard(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    clipboard: &mut Option<Clipboard>,
) -> bool {
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
    theme: &EditorTheme,
    full_text: &str,
) {
    let painter = ui.painter_at(*rect);
    let time = ui.input(|i| i.time);

    let since_edit = time - editor.last_edit_time;
    let cursor_visible = since_edit < 0.5 || ((since_edit * 2.0) as u64 % 2 == 0);

    let first_line = (editor.scroll_y / metrics.line_height).floor() as usize;
    let visible_count = (rect.height() / metrics.line_height).ceil() as usize + 1;
    let last_line = (first_line + visible_count).min(editor.line_count());

    let highlighted = highlighter.highlight_lines(
        full_text,
        editor.file_path.as_deref(),
        first_line,
        last_line,
    );

    let active_lines: Vec<usize> = editor.cursors.iter().map(|c| c.pos.line).collect();

    let gutter_rect = Rect::from_min_size(
        rect.left_top(),
        Vec2::new(metrics.gutter_width, rect.height()),
    );
    painter.rect_filled(gutter_rect, 0.0, theme.gutter_bg);

    painter.line_segment(
        [
            Pos2::new(rect.left() + metrics.gutter_width, rect.top()),
            Pos2::new(rect.left() + metrics.gutter_width, rect.bottom()),
        ],
        Stroke::new(1.0, theme.gutter_divider),
    );

    for line_idx in first_line..last_line {
        let y = rect.top() + (line_idx as f32) * metrics.line_height - editor.scroll_y;

        if active_lines.contains(&line_idx) {
            let line_rect = Rect::from_min_size(
                Pos2::new(rect.left() + metrics.gutter_width, y),
                Vec2::new(rect.width() - metrics.gutter_width, metrics.line_height),
            );
            painter.rect_filled(line_rect, 0.0, theme.active_line);
        }

        let ln_color = if active_lines.contains(&line_idx) {
            theme.line_num_active
        } else {
            theme.line_num
        };
        let ln_text = format!("{}", line_idx + 1);
        painter.text(
            Pos2::new(
                rect.left() + metrics.gutter_width - theme.gutter_padding / 2.0,
                y + metrics.line_height / 2.0,
            ),
            egui::Align2::RIGHT_CENTER,
            &ln_text,
            metrics.font_id.clone(),
            ln_color,
        );

        for cursor in &editor.cursors {
            if let Some((sel_start, sel_end)) = cursor.selection_ordered() {
                draw_selection(
                    &painter, rect, line_idx, &sel_start, &sel_end, metrics, editor, theme,
                );
            }
        }

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
                    theme.text,
                );
            }
        }

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
                    painter.rect_filled(cursor_rect, 0.0, theme.cursor);
                }
            }
        }
    }
}

fn render_minimap(
    ui: &egui::Ui,
    rect: Rect,
    editor: &Editor,
    theme: &EditorTheme,
    visible_lines: usize,
    line_height: f32,
    tokens: &[Vec<StyledToken>],
) {
    let line_count = editor.line_count();
    if line_count == 0 {
        return;
    }

    let painter = ui.painter_at(rect);
    let minimap_line_height = (rect.height() / line_count.max(1) as f32).clamp(1.0, 4.0);

    let mut max_cols = 1usize;
    for line in 0..line_count {
        let len = editor.line_text(line).chars().count();
        if len > max_cols {
            max_cols = len;
        }
    }
    let max_cols = max_cols.max(1) as f32;

    for (line_idx, token_line) in tokens.iter().enumerate() {
        if line_idx >= line_count {
            break;
        }
        let y = rect.top() + line_idx as f32 * minimap_line_height;
        let mut x = rect.left();
        for token in token_line {
            let chars = token.text.chars().count() as f32;
            if chars <= 0.0 {
                continue;
            }
            let width = (chars / max_cols).max(0.002) * rect.width();
            let color = tint_for_minimap(token.color, theme);
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, minimap_line_height)),
                0.5,
                color,
            );
            x += width;
        }
    }

    let viewport_top = rect.top() + (editor.scroll_y / line_height) * minimap_line_height;
    let viewport_height = (visible_lines as f32 * minimap_line_height).min(rect.height());
    let viewport = Rect::from_min_size(
        Pos2::new(
            rect.left(),
            viewport_top.clamp(rect.top(), rect.bottom() - viewport_height),
        ),
        Vec2::new(rect.width(), viewport_height.max(minimap_line_height)),
    );

    painter.rect_filled(viewport, 3.0, theme.minimap_viewport);
    painter.rect_stroke(viewport, 3.0, Stroke::new(1.0, theme.minimap_border));
}

fn scroll_to_minimap_pos(
    pointer: Pos2,
    rect: Rect,
    editor: &mut Editor,
    line_height: f32,
    editor_height: f32,
    visible_lines: usize,
) {
    let line_count = editor.line_count();
    if line_count == 0 || rect.height() <= 0.0 {
        return;
    }
    let minimap_line_height = rect.height() / line_count as f32;
    let clamped_y = pointer.y.clamp(rect.top(), rect.bottom());
    let target_line = ((clamped_y - rect.top()) / minimap_line_height)
        .floor()
        .min(line_count.saturating_sub(1) as f32);
    let center_offset = (visible_lines as f32 / 2.0).max(1.0);
    let mut target_scroll = (target_line - center_offset).max(0.0) * line_height;
    let max_scroll = (line_count as f32 * line_height - editor_height).max(0.0);
    if target_scroll > max_scroll {
        target_scroll = max_scroll;
    }
    editor.scroll_y = target_scroll;
}

fn draw_selection(
    painter: &egui::Painter,
    rect: &Rect,
    line_idx: usize,
    sel_start: &crate::editor::Position,
    sel_end: &crate::editor::Position,
    metrics: &EditorMetrics,
    editor: &Editor,
    theme: &EditorTheme,
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

    let sel_rect = Rect::from_min_size(Pos2::new(x1, y), Vec2::new(x2 - x1, metrics.line_height));
    painter.rect_filled(sel_rect, 0.0, theme.selection);
}

fn tint_for_minimap(color: Color32, theme: &EditorTheme) -> Color32 {
    let fg = Rgba::from(color);
    let accent = Rgba::from(theme.minimap_fg);
    let bg = Rgba::from(theme.minimap_bg);
    let mixed = fg * 0.55 + accent * 0.25 + bg * 0.2;
    Color32::from_rgba_unmultiplied(
        (mixed.r() * 255.0) as u8,
        (mixed.g() * 255.0) as u8,
        (mixed.b() * 255.0) as u8,
        230,
    )
}
