use eframe::egui::{self, Color32, FontId, Pos2, Rect, Rgba, Sense, Stroke, Vec2};

use crate::editor::{DiffHunk, DiffKind, Editor, Position, LINE_HEIGHT};
use crate::syntax::{StyledToken, SyntaxHighlighter};
use arboard::Clipboard;
use std::path::Path;
use std::process::Command;

const MINIMAP_SPACING: f32 = 8.0;
const MIN_EDITOR_WIDTH_FOR_MINIMAP: f32 = 600.0;

#[derive(Clone)]
pub struct EditorTheme {
    pub background: Color32,
    pub text: Color32,
    pub cursor: Color32,
    pub selection: Color32,
    pub search_match: Color32,
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
                search_match: Color32::from_rgba_premultiplied(255, 216, 102, 120),
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
                search_match: Color32::from_rgba_premultiplied(181, 137, 0, 120),
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
    search_query: Option<&str>,
    auto_focus: bool,
) -> bool {
    let mut changed = false;
    let metrics = EditorMetrics::compute(ui, editor.line_count(), theme);
    let available = ui.available_rect_before_wrap();
    let mut minimap_rect =
        if editor.minimap_enabled && available.width() > MIN_EDITOR_WIDTH_FOR_MINIMAP {
            let width = editor.minimap_width.clamp(80.0, 200.0);
            Some(Rect::from_min_size(
                Pos2::new(available.max.x - width, available.top()),
                Vec2::new(width, available.height()),
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
    let visible_lines_count = (editor_rect.height() / metrics.line_height).ceil() as usize;
    let visible_lines_count = visible_lines_count.max(1);

    let fold_map = compute_fold_ranges(editor);
    let collapsed_ranges = fold_map
        .iter()
        .filter_map(|(start, end)| {
            if editor.folded_lines.contains(start) {
                Some((*start, *end))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let (visible_lines, line_to_visible) =
        build_visible_lines(editor.line_count(), &collapsed_ranges);

    // Background layers
    ui.painter().rect_filled(editor_rect, 0.0, theme.background);
    if let Some(rect) = minimap_rect {
        let opacity = editor.minimap_opacity.clamp(0.2, 1.0);
        ui.painter()
            .rect_filled(rect, 6.0, apply_minimap_opacity(theme.minimap_bg, opacity));
        ui.painter().rect_stroke(
            rect,
            6.0,
            Stroke::new(1.0, apply_minimap_opacity(theme.minimap_border, opacity)),
        );
    }

    // Allocate interactive regions
    let response = ui.allocate_rect(editor_rect, Sense::click_and_drag());
    let minimap_response = minimap_rect.map(|rect| ui.allocate_rect(rect, Sense::click_and_drag()));

    // Request focus on click/drag, or automatically when no overlay is active
    if response.clicked() || response.dragged() || auto_focus {
        ui.memory_mut(|m| m.request_focus(response.id));
    }

    let has_focus = ui.memory(|m| m.has_focus(response.id));

    // Handle mouse click -> fold toggle or set cursor position
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let (line, col, vis_line) =
                screen_to_editor_pos(pos, &editor_rect, &metrics, editor, &visible_lines);
            if pos.x <= editor_rect.left() + metrics.gutter_width {
                if let Some(end_line) = fold_map.get(&line) {
                    let is_collapsed = editor.folded_lines.contains(&line);
                    if is_collapsed {
                        editor.folded_lines.remove(&line);
                    } else if *end_line > line + 1 {
                        editor.folded_lines.insert(line);
                    }
                    let clamped_vis = vis_line.min(visible_lines.len().saturating_sub(1));
                    let target_scroll = clamped_vis as f32 * metrics.line_height;
                    editor.scroll_y = target_scroll;
                    return changed;
                }
            }
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
            let (line, col, _) =
                screen_to_editor_pos(pos, &editor_rect, &metrics, editor, &visible_lines);
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
            let (line, col, _) =
                screen_to_editor_pos(pos, &editor_rect, &metrics, editor, &visible_lines);
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
            (visible_lines.len() as f32 * metrics.line_height - editor_rect.height()).max(0.0);
        editor.scroll_y = editor.scroll_y.min(max_scroll);
    }

    // Handle keyboard input
    if has_focus {
        changed = handle_keyboard(
            ui,
            editor,
            clipboard,
            visible_lines_count,
            metrics.line_height,
        );
    }

    let full_text = editor.rope.to_string();
    let now = ui.input(|i| i.time);
    if minimap_rect.is_some() && now - editor.diff_last_check > 1.0 {
        editor.diff_hunks = read_diff_hunks(editor.file_path.as_deref());
        editor.diff_last_check = now;
    }

    // Render visible lines
    render_lines(
        ui,
        &editor_rect,
        editor,
        &metrics,
        highlighter,
        theme,
        &full_text,
        search_query,
        &visible_lines,
        &fold_map,
    );

    if let (Some(rect), Some(resp)) = (minimap_rect, minimap_response) {
        if resp.hovered() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let line_count = editor.line_count().max(1);
                let rel = ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                let line_idx = (rel * line_count as f32).floor() as usize;
                let start = line_idx.saturating_sub(2);
                let end = (line_idx + 3).min(line_count);
                let mut preview = String::new();
                for line in start..end {
                    let line_text = editor.line_text(line);
                    preview.push_str(&format!("{:>4}  {}\n", line + 1, line_text));
                }
                let layer_id = egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("minimap_preview_layer"),
                );
                egui::show_tooltip_at_pointer(
                    ui.ctx(),
                    layer_id,
                    egui::Id::new("minimap_preview"),
                    |ui: &mut egui::Ui| {
                        ui.label(egui::RichText::new(preview).monospace().size(11.0));
                    },
                );
            }
        }
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
            visible_lines_count,
            metrics.line_height,
            &minimap_tokens,
            search_query,
            &editor.diff_hunks,
            &fold_map,
        );
        if (resp.clicked() || resp.dragged()) && resp.contains_pointer() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let line_count = editor.line_count();
                if is_minimap_fold_marker_hit(pos, rect, line_count, &fold_map) {
                    toggle_fold_from_minimap(
                        pos,
                        rect,
                        editor,
                        &fold_map,
                        &visible_lines,
                        &metrics,
                    );
                } else {
                    scroll_to_minimap_pos(
                        pos,
                        rect,
                        editor,
                        metrics.line_height,
                        editor_rect.height(),
                        visible_lines_count,
                    );
                }
            }
        }
    }

    // Ensure cursor is visible (auto-scroll)
    if !editor.cursors.is_empty() {
        let primary = &editor.cursors[0];
        let cursor_y = cursor_visual_y(primary.pos, &line_to_visible, metrics.line_height);

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
    visible_lines: &[usize],
) -> (usize, usize, usize) {
    if visible_lines.is_empty() || editor.line_count() == 0 {
        return (0, 0, 0);
    }

    let rel_y = screen_pos.y - rect.top() + editor.scroll_y;
    let rel_x = screen_pos.x - rect.left() - metrics.gutter_width - 4.0 + editor.scroll_x;

    let mut vis_line = (rel_y / metrics.line_height).floor().max(0.0) as usize;
    vis_line = vis_line.min(visible_lines.len().saturating_sub(1));
    let line = visible_lines[vis_line];

    let col = (rel_x / metrics.char_width).round().max(0.0) as usize;
    let line_text = editor.line_text(line);
    let col = col.min(line_text.chars().count());

    (line, col, vis_line)
}

fn handle_keyboard(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    clipboard: &mut Option<Clipboard>,
    visible_lines: usize,
    line_height: f32,
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
                let alt = modifiers.alt;

                match key {
                    egui::Key::ArrowUp if ctrl && alt => {
                        let delta = visible_lines as f32 * line_height;
                        editor.scroll_y = (editor.scroll_y - delta).max(0.0);
                    }
                    egui::Key::ArrowDown if ctrl && alt => {
                        let delta = visible_lines as f32 * line_height;
                        let max_scroll =
                            (editor.line_count() as f32 * line_height - delta).max(0.0);
                        editor.scroll_y = (editor.scroll_y + delta).min(max_scroll);
                    }
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

fn compute_fold_ranges(editor: &Editor) -> std::collections::HashMap<usize, usize> {
    let mut map = std::collections::HashMap::new();
    let mut stack: Vec<usize> = Vec::new();

    for line_idx in 0..editor.line_count() {
        let line = editor.line_text(line_idx);
        for ch in line.chars() {
            match ch {
                '{' => stack.push(line_idx),
                '}' => {
                    if let Some(start) = stack.pop() {
                        if line_idx > start {
                            let entry = map.entry(start).or_insert(line_idx);
                            if line_idx > *entry {
                                *entry = line_idx;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    map
}

fn build_visible_lines(
    line_count: usize,
    collapsed_ranges: &[(usize, usize)],
) -> (Vec<usize>, Vec<usize>) {
    if line_count == 0 {
        return (Vec::new(), Vec::new());
    }

    let mut ranges = collapsed_ranges.to_vec();
    ranges.sort_by_key(|(start, _)| *start);

    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let mut visible_lines = Vec::with_capacity(line_count);
    let mut line_to_visible = vec![0usize; line_count];
    let mut range_idx = 0usize;
    let mut fold_start_visible: Option<usize> = None;

    for line in 0..line_count {
        while range_idx < merged.len() && line > merged[range_idx].1 {
            range_idx += 1;
            fold_start_visible = None;
        }

        if let Some((start, end)) = merged.get(range_idx).copied() {
            if line >= start && line <= end {
                if line == start {
                    let idx = visible_lines.len();
                    visible_lines.push(line);
                    line_to_visible[line] = idx;
                    fold_start_visible = Some(idx);
                } else {
                    let idx =
                        fold_start_visible.unwrap_or_else(|| visible_lines.len().saturating_sub(1));
                    line_to_visible[line] = idx;
                }
                continue;
            }
        }

        let idx = visible_lines.len();
        visible_lines.push(line);
        line_to_visible[line] = idx;
    }

    (visible_lines, line_to_visible)
}

fn cursor_visual_y(pos: Position, line_to_visible: &[usize], line_height: f32) -> f32 {
    if line_to_visible.is_empty() {
        return 0.0;
    }
    let line = pos.line.min(line_to_visible.len().saturating_sub(1));
    line_to_visible[line] as f32 * line_height
}

fn read_diff_hunks(path: Option<&Path>) -> Vec<DiffHunk> {
    let path = match path {
        Some(path) => path,
        None => return Vec::new(),
    };
    let file_dir = path.parent().unwrap_or_else(|| Path::new("."));

    let output = Command::new("git")
        .arg("diff")
        .arg("-U0")
        .arg("--")
        .arg(path)
        .current_dir(file_dir)
        .output();

    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut hunks = Vec::new();

    for line in stdout.lines() {
        if !line.starts_with("@@") {
            continue;
        }
        let header = match line.split("@@").nth(1) {
            Some(header) => header.trim(),
            None => continue,
        };
        let mut parts = header.split_whitespace();
        let removed = match parts.next() {
            Some(part) => part,
            None => continue,
        };
        let added = match parts.next() {
            Some(part) => part,
            None => continue,
        };

        let (_removed_start, removed_count) = parse_diff_range(removed);
        let (added_start, added_count) = parse_diff_range(added);

        let kind = if removed_count > 0 && added_count > 0 {
            DiffKind::Modified
        } else if added_count > 0 {
            DiffKind::Added
        } else {
            DiffKind::Removed
        };

        let start_line = added_start.saturating_sub(1);
        let length = if added_count > 0 {
            added_count
        } else {
            removed_count.max(1)
        };
        let end_line = start_line.saturating_add(length.saturating_sub(1));

        hunks.push(DiffHunk {
            start: start_line,
            end: end_line,
            kind,
        });
    }

    hunks
}

fn parse_diff_range(range: &str) -> (usize, usize) {
    let trimmed = range.trim_start_matches(['-', '+'].as_ref());
    let mut parts = trimmed.split(',');
    let start = parts
        .next()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let count = parts
        .next()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);
    (start, count)
}

fn minimap_line_at(pointer: Pos2, rect: Rect, line_count: usize) -> Option<usize> {
    if line_count == 0 || rect.height() <= 0.0 {
        return None;
    }
    let minimap_line_height = rect.height() / line_count as f32;
    let clamped_y = pointer.y.clamp(rect.top(), rect.bottom());
    let line = ((clamped_y - rect.top()) / minimap_line_height)
        .floor()
        .min(line_count.saturating_sub(1) as f32) as usize;
    Some(line)
}

fn is_minimap_fold_marker_hit(
    pointer: Pos2,
    rect: Rect,
    line_count: usize,
    fold_map: &std::collections::HashMap<usize, usize>,
) -> bool {
    if fold_map.is_empty() {
        return false;
    }
    if pointer.x < rect.right() - 10.0 {
        return false;
    }
    let line = match minimap_line_at(pointer, rect, line_count) {
        Some(line) => line,
        None => return false,
    };
    fold_map.get(&line).map_or(false, |end| *end > line + 1)
}

fn toggle_fold_from_minimap(
    pointer: Pos2,
    rect: Rect,
    editor: &mut Editor,
    fold_map: &std::collections::HashMap<usize, usize>,
    visible_lines: &[usize],
    metrics: &EditorMetrics,
) {
    let line_count = editor.line_count();
    let line = match minimap_line_at(pointer, rect, line_count) {
        Some(line) => line,
        None => return,
    };
    if !fold_map
        .get(&line)
        .map_or(false, |end_line| *end_line > line + 1)
    {
        return;
    }

    if editor.folded_lines.contains(&line) {
        editor.folded_lines.remove(&line);
    } else {
        editor.folded_lines.insert(line);
    }

    let vis_idx = visible_lines
        .iter()
        .position(|&visible| visible == line)
        .unwrap_or(0);
    let target_scroll = vis_idx as f32 * metrics.line_height;
    editor.scroll_y = target_scroll;
}

fn render_lines(
    ui: &egui::Ui,
    rect: &Rect,
    editor: &Editor,
    metrics: &EditorMetrics,
    highlighter: &SyntaxHighlighter,
    theme: &EditorTheme,
    full_text: &str,
    search_query: Option<&str>,
    visible_lines: &[usize],
    fold_map: &std::collections::HashMap<usize, usize>,
) {
    if visible_lines.is_empty() {
        return;
    }

    let painter = ui.painter_at(*rect);
    let time = ui.input(|i| i.time);

    let since_edit = time - editor.last_edit_time;
    let cursor_visible = since_edit < 0.5 || ((since_edit * 2.0) as u64 % 2 == 0);

    let first_line = (editor.scroll_y / metrics.line_height).floor() as usize;
    let visible_count = (rect.height() / metrics.line_height).ceil() as usize + 1;
    let last_line = (first_line + visible_count).min(visible_lines.len());

    let highlighted = highlighter.highlight_lines(
        full_text,
        editor.file_path.as_deref(),
        0,
        editor.line_count(),
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

    for vis_idx in first_line..last_line {
        let line_idx = visible_lines[vis_idx];
        let y = rect.top() + (vis_idx as f32) * metrics.line_height - editor.scroll_y;

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

        if let Some(query) = search_query {
            if !query.is_empty() {
                let line_text = editor.line_text(line_idx);
                draw_search_matches(&painter, rect, &line_text, query, y, metrics, editor, theme);
            }
        }

        for cursor in &editor.cursors {
            if let Some((sel_start, sel_end)) = cursor.selection_ordered() {
                draw_selection(
                    &painter, rect, line_idx, y, &sel_start, &sel_end, metrics, editor, theme,
                );
            }
        }

        let hl_idx = line_idx;
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

        if let Some(end_line) = fold_map.get(&line_idx) {
            let marker_x = rect.left() + metrics.gutter_width - theme.gutter_padding * 0.9;
            let marker_y = y + metrics.line_height / 2.0;
            let collapsed = editor.folded_lines.contains(&line_idx);
            let points = if collapsed {
                vec![
                    Pos2::new(marker_x - 2.0, marker_y - 4.0),
                    Pos2::new(marker_x - 2.0, marker_y + 4.0),
                    Pos2::new(marker_x + 4.0, marker_y),
                ]
            } else {
                vec![
                    Pos2::new(marker_x - 4.0, marker_y - 2.0),
                    Pos2::new(marker_x + 4.0, marker_y - 2.0),
                    Pos2::new(marker_x, marker_y + 4.0),
                ]
            };
            let color = if collapsed {
                egui::Color32::from_rgb(140, 140, 140)
            } else {
                egui::Color32::from_rgb(170, 170, 170)
            };
            if *end_line > line_idx + 1 {
                painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
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
    search_query: Option<&str>,
    diff_hunks: &[DiffHunk],
    fold_map: &std::collections::HashMap<usize, usize>,
) {
    let line_count = editor.line_count();
    if line_count == 0 {
        return;
    }

    let painter = ui.painter_at(rect);
    let minimap_line_height = (rect.height() / line_count.max(1) as f32).clamp(1.0, 4.0);
    let opacity = editor.minimap_opacity.clamp(0.2, 1.0);

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
            let color = tint_for_minimap(token.color, theme, opacity);
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, minimap_line_height)),
                0.5,
                color,
            );
            x += width;
        }
    }

    if let Some(query) = search_query {
        if !query.is_empty() {
            for line_idx in 0..line_count {
                let line_text = editor.line_text(line_idx);
                if line_text.contains(query) {
                    let y = rect.top() + line_idx as f32 * minimap_line_height;
                    let marker = Rect::from_min_size(
                        Pos2::new(rect.left(), y),
                        Vec2::new(rect.width(), minimap_line_height.max(1.0)),
                    );
                    painter.rect_filled(
                        marker,
                        0.0,
                        apply_minimap_opacity(theme.search_match, opacity * 0.8),
                    );
                }
            }
        }
    }

    if let Some((sel_start, sel_end)) = editor.cursors[0].selection_ordered() {
        let start = sel_start.line.min(sel_end.line);
        let end = sel_end.line.max(sel_start.line);
        let y1 = rect.top() + start as f32 * minimap_line_height;
        let y2 = rect.top() + (end + 1) as f32 * minimap_line_height;
        let selection_rect = Rect::from_min_size(
            Pos2::new(rect.left(), y1),
            Vec2::new(rect.width(), (y2 - y1).max(minimap_line_height)),
        );
        painter.rect_filled(
            selection_rect,
            2.0,
            apply_minimap_opacity(theme.selection, opacity * 0.7),
        );
    }

    if !diff_hunks.is_empty() {
        for hunk in diff_hunks {
            if hunk.start >= line_count {
                continue;
            }
            let start = hunk.start.min(line_count.saturating_sub(1));
            let end = hunk.end.min(line_count.saturating_sub(1));
            let y1 = rect.top() + start as f32 * minimap_line_height;
            let y2 = rect.top() + (end + 1) as f32 * minimap_line_height;
            let color = match hunk.kind {
                DiffKind::Added => Color32::from_rgb(75, 185, 116),
                DiffKind::Removed => Color32::from_rgb(214, 90, 90),
                DiffKind::Modified => Color32::from_rgb(220, 170, 80),
            };
            let marker = Rect::from_min_size(
                Pos2::new(rect.right() - 4.0, y1),
                Vec2::new(3.0, (y2 - y1).max(minimap_line_height)),
            );
            painter.rect_filled(marker, 1.0, apply_minimap_opacity(color, opacity));
        }
    }

    if !fold_map.is_empty() {
        let marker_x = rect.right() - 9.0;
        for (start, end) in fold_map {
            if *end <= *start + 1 {
                continue;
            }
            let y = rect.top() + *start as f32 * minimap_line_height + minimap_line_height / 2.0;
            let collapsed = editor.folded_lines.contains(start);
            let points = if collapsed {
                vec![
                    Pos2::new(marker_x - 2.0, y - 3.0),
                    Pos2::new(marker_x - 2.0, y + 3.0),
                    Pos2::new(marker_x + 3.0, y),
                ]
            } else {
                vec![
                    Pos2::new(marker_x - 3.0, y - 2.0),
                    Pos2::new(marker_x + 3.0, y - 2.0),
                    Pos2::new(marker_x, y + 3.0),
                ]
            };
            let color = if collapsed {
                Color32::from_rgb(140, 140, 140)
            } else {
                Color32::from_rgb(175, 175, 175)
            };
            painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
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

    painter.rect_filled(
        viewport,
        3.0,
        apply_minimap_opacity(theme.minimap_viewport, opacity),
    );
    painter.rect_stroke(
        viewport,
        3.0,
        Stroke::new(1.0, apply_minimap_opacity(theme.minimap_border, opacity)),
    );
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
    y: f32,
    sel_start: &crate::editor::Position,
    sel_end: &crate::editor::Position,
    metrics: &EditorMetrics,
    editor: &Editor,
    theme: &EditorTheme,
) {
    if line_idx < sel_start.line || line_idx > sel_end.line {
        return;
    }

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

fn draw_search_matches(
    painter: &egui::Painter,
    rect: &Rect,
    line_text: &str,
    query: &str,
    y: f32,
    metrics: &EditorMetrics,
    editor: &Editor,
    theme: &EditorTheme,
) {
    if query.is_empty() || line_text.is_empty() {
        return;
    }
    let text_x = rect.left() + metrics.gutter_width + 4.0;
    let query_len = query.chars().count().max(1);

    for (byte_idx, _) in line_text.match_indices(query) {
        let start_col = line_text[..byte_idx].chars().count();
        let end_col = start_col + query_len;
        let x1 = text_x + start_col as f32 * metrics.char_width - editor.scroll_x;
        let x2 = text_x + end_col as f32 * metrics.char_width - editor.scroll_x;
        let rect = Rect::from_min_size(Pos2::new(x1, y), Vec2::new(x2 - x1, metrics.line_height));
        painter.rect_filled(rect, 0.0, theme.search_match);
    }
}

fn tint_for_minimap(color: Color32, theme: &EditorTheme, opacity: f32) -> Color32 {
    let fg = Rgba::from(color);
    let accent = Rgba::from(theme.minimap_fg);
    let bg = Rgba::from(theme.minimap_bg);
    let mixed = fg * 0.55 + accent * 0.25 + bg * 0.2;
    Color32::from_rgba_unmultiplied(
        (mixed.r() * 255.0) as u8,
        (mixed.g() * 255.0) as u8,
        (mixed.b() * 255.0) as u8,
        (opacity * 230.0).clamp(40.0, 230.0) as u8,
    )
}

fn apply_minimap_opacity(color: Color32, opacity: f32) -> Color32 {
    let mut rgba = Rgba::from(color);
    rgba = Rgba::from_rgba_premultiplied(
        rgba.r(),
        rgba.g(),
        rgba.b(),
        (rgba.a() * opacity).clamp(0.05, 1.0),
    );
    Color32::from_rgba_premultiplied(
        (rgba.r() * 255.0) as u8,
        (rgba.g() * 255.0) as u8,
        (rgba.b() * 255.0) as u8,
        (rgba.a() * 255.0) as u8,
    )
}
