// Imports updated
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Rgba, Sense, Stroke, Vec2};
use crate::editor::{DiffHunk, DiffKind, Editor};
use crate::syntax::{StyledToken, SyntaxHighlighter};
use arboard::Clipboard;

const MINIMAP_SPACING: f32 = 8.0;
const MIN_EDITOR_WIDTH_FOR_MINIMAP: f32 = 600.0;
// Fixed minimap dimensions
const MINIMAP_LINE_HEIGHT: f32 = 2.0;
const MINIMAP_CHAR_WIDTH: f32 = 1.0; // Reduced for better density
const MINIMAP_CHAR_HEIGHT: f32 = 2.0; // Usually matches line height

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorThemeKind {
    Dark,
    Light,
    Monokai,
    SolarizedDark,
}

impl EditorThemeKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Monokai => "Monokai",
            Self::SolarizedDark => "Solarized Dark",
        }
    }

    pub fn palette(&self) -> EditorTheme {
        match self {
            Self::Dark | Self::Monokai => EditorTheme::monokai(),
            Self::Light => EditorTheme::light(),
            Self::SolarizedDark => EditorTheme::solarized_dark(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EditorTheme {
    pub background: Color32,
    pub foreground: Color32,
    pub gutter_bg: Color32,
    pub gutter_fg: Color32,
    pub gutter_divider: Color32,
    pub selection_bg: Color32,
    pub gutter_padding: f32,
    pub minimap_bg: Color32,
    pub minimap_border: Color32,
    pub minimap_viewport: Color32,
    pub minimap_fg: Color32,
    pub search_match: Color32,
    pub active_line: Color32,
    pub line_num_active: Color32,
    pub line_num: Color32,
    pub text: Color32,
    pub cursor: Color32,
    pub selection: Color32,
}

impl EditorTheme {
    pub fn light() -> Self {
        Self {
            background: Color32::WHITE,
            foreground: Color32::BLACK,
            text: Color32::BLACK,
            gutter_bg: Color32::from_rgb(245, 245, 245),
            gutter_fg: Color32::from_rgb(150, 150, 150),
            gutter_divider: Color32::from_rgb(220, 220, 220),
            selection_bg: Color32::from_rgb(200, 200, 255),
            selection: Color32::from_rgb(200, 200, 255),
            gutter_padding: 12.0,
            minimap_bg: Color32::from_black_alpha(10),
            minimap_border: Color32::from_black_alpha(20),
            minimap_viewport: Color32::from_black_alpha(30),
            minimap_fg: Color32::from_rgb(200, 200, 200),
            search_match: Color32::from_rgb(255, 255, 0),
            active_line: Color32::from_black_alpha(10),
            line_num_active: Color32::BLACK,
            line_num: Color32::from_rgb(150, 150, 150),
            cursor: Color32::BLACK,
        }
    }

    pub fn monokai() -> Self {
        Self {
            background: Color32::from_rgb(39, 40, 34),
            foreground: Color32::from_rgb(248, 248, 242),
            text: Color32::from_rgb(248, 248, 242),
            gutter_bg: Color32::from_rgb(39, 40, 34),
            gutter_fg: Color32::from_rgb(144, 144, 138),
            gutter_divider: Color32::from_rgb(60, 60, 60),
            selection_bg: Color32::from_rgb(73, 72, 62),
            selection: Color32::from_rgb(73, 72, 62),
            gutter_padding: 12.0,
            minimap_bg: Color32::from_white_alpha(10),
            minimap_border: Color32::from_white_alpha(20),
            minimap_viewport: Color32::from_white_alpha(30),
            minimap_fg: Color32::from_rgb(248, 248, 242),
            search_match: Color32::from_rgb(100, 100, 0),
            active_line: Color32::from_white_alpha(15),
            line_num_active: Color32::WHITE,
            line_num: Color32::from_rgb(144, 144, 138),
            cursor: Color32::from_rgb(248, 248, 242),
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            background: Color32::from_rgb(0, 43, 54),
            foreground: Color32::from_rgb(131, 148, 150),
            text: Color32::from_rgb(131, 148, 150),
            gutter_bg: Color32::from_rgb(7, 54, 66),
            gutter_fg: Color32::from_rgb(88, 110, 117),
            gutter_divider: Color32::from_rgb(88, 110, 117),
            selection_bg: Color32::from_rgb(7, 54, 66),
            selection: Color32::from_rgb(7, 54, 66),
            gutter_padding: 12.0,
            minimap_bg: Color32::from_white_alpha(5),
            minimap_border: Color32::from_white_alpha(10),
            minimap_viewport: Color32::from_white_alpha(20),
            minimap_fg: Color32::from_rgb(131, 148, 150),
            search_match: Color32::from_rgb(181, 137, 0),
            active_line: Color32::from_white_alpha(10),
            line_num_active: Color32::WHITE,
            line_num: Color32::from_rgb(88, 110, 117),
            cursor: Color32::from_rgb(131, 148, 150),
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
    
    // Calculate minimap layout
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
            .rect_filled(rect, 0.0, apply_minimap_opacity(theme.minimap_bg, opacity));
        ui.painter().line_segment(
             [rect.left_top(), rect.left_bottom()],
             Stroke::new(1.0, apply_minimap_opacity(theme.minimap_border, opacity))
        );
    }

    // Allocate interactive regions
    let response = ui.allocate_rect(editor_rect, Sense::click_and_drag());
    let minimap_response = minimap_rect.map(|rect| ui.allocate_rect(rect, Sense::click_and_drag()));

    // Request focus on click/drag
    if response.clicked() || response.dragged() || auto_focus {
        ui.memory_mut(|m| m.request_focus(response.id));
    }

    let has_focus = ui.memory(|m| m.has_focus(response.id));

    // Interact with Editor (Clicks)
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let (line, col, vis_line) =
                screen_to_editor_pos(pos, &editor_rect, &metrics, editor, &visible_lines);
            
            // Check gutter click for folding
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
                    return changed; // Return early after fold toggle
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

    // Render Minimap
    if let (Some(rect), Some(resp)) = (minimap_rect, minimap_response) {
        let minimap_tokens = highlighter.highlight_lines(
            &full_text,
            editor.file_path.as_deref(),
            0,
            editor.line_count(),
        );

        // Calculate visible range for minimap (simple scroll syncing)
        // We map the editor's scroll percentage to the minimap's scrollable height.
        let line_count = editor.line_count().max(1);
        let total_minimap_height = line_count as f32 * MINIMAP_LINE_HEIGHT;
        let available_minimap_height = rect.height();
        
        // Calculate minimap scroll offset
        // We want the viewport to be roughly centered or follow the editor scroll.
        // Simple approach: Proportional scrolling if content fits, specialized if not.
        // Actually, VS Code style: The minimap shows the whole file if it fits. If not, it behaves like a scrollbar.
        // But for a true minimap, we usually render 1:1 with fixed pixel steps and scroll it.
        
        let editor_scroll_pct = if visible_lines.len() * metrics.line_height as usize > 0 {
             editor.scroll_y / (visible_lines.len() as f32 * metrics.line_height).max(1.0)
        } else {
             0.0
        };
        
        // Editor view height in lines
        let page_lines = (editor_rect.height() / metrics.line_height).ceil() as f32;
        
        // Calculate how much we need to scroll the minimap to keep the visible area in view.
        // Minimap visible window size (in lines)
        let minimap_lines_fit = (available_minimap_height / MINIMAP_LINE_HEIGHT).floor() as f32;
        
        let mut minimap_scroll_y = 0.0;
        
        if total_minimap_height > available_minimap_height {
            // Scrollable minimap
            // We want the "viewport" (current view) to be visible.
            // Current top line in editor
            let current_top_line = (editor.scroll_y / metrics.line_height).floor() as f32;
            
            // Try to keep the viewport centered
            let target_center = current_top_line * MINIMAP_LINE_HEIGHT;
            minimap_scroll_y = target_center - (available_minimap_height / 2.0);
            
            // Clamp
            let max_mini_scroll = total_minimap_height - available_minimap_height;
            minimap_scroll_y = minimap_scroll_y.clamp(0.0, max_mini_scroll);
        }

        render_minimap_fixed(
            ui,
            rect,
            editor,
            theme,
            minimap_scroll_y,
            &minimap_tokens,
            search_query,
            &editor.diff_hunks,
            &fold_map,
            visible_lines.len(),
            metrics.line_height,
            editor_rect.height(),
        );

        // Handle minimap interaction
        if resp.hovered() {
            if let Some(pos) = resp.interact_pointer_pos() {
                // Determine line under mouse (taking minimap scroll into account)
                let rel_y = pos.y - rect.top() + minimap_scroll_y;
                let line_idx = (rel_y / MINIMAP_LINE_HEIGHT).floor() as usize;
                
                if line_idx < line_count {
                   let start = line_idx.saturating_sub(2);
                   let end = (line_idx + 3).min(line_count);
                   let mut preview = String::new();
                   for line in start..end {
                       let line_text = editor.line_text(line);
                       preview.push_str(&format!("{:>4}  {}\n", line + 1, line_text));
                   }
                   
                   if !preview.is_empty() {
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
            }
        }
        
        if (resp.clicked() || resp.dragged()) && resp.contains_pointer() {
            if let Some(pos) = resp.interact_pointer_pos() {
               // Calculate targeted line
               let rel_y = pos.y - rect.top() + minimap_scroll_y;
               let target_line = (rel_y / MINIMAP_LINE_HEIGHT).floor() as usize;
               
               // We simply jump to that line
               // Center that line in the editor if possible
               let target_line = target_line.min(line_count.saturating_sub(1));
               
               let target_editor_y = target_line as f32 * metrics.line_height;
               // Center: target_y - (half screen)
               let centered_y = target_editor_y - (editor_rect.height() / 2.0);
               
               let max_scroll = (visible_lines.len() as f32 * metrics.line_height - editor_rect.height()).max(0.0);
               editor.scroll_y = centered_y.clamp(0.0, max_scroll);
            }
        }
    }

    // Ensure cursor is visible (auto-scroll) - Logic kept same
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

// --- Helper Structs & Functions ---

pub struct EditorMetrics {
    pub font_id: FontId,
    pub line_height: f32,
    pub char_width: f32,
    pub gutter_width: f32,
    pub gutter_padding: f32,
}

impl EditorMetrics {
    pub fn compute(ui: &egui::Ui, line_count: usize, theme: &EditorTheme) -> Self {
        let font_id = egui::FontId::monospace(13.5);
        let line_height = ui.fonts(|f| f.row_height(&font_id));
        let char_width = ui.fonts(|f| f.glyph_width(&font_id, 'm'));
        let digits = line_count.to_string().len().max(3);
        let gutter_padding = theme.gutter_padding;
        let gutter_width = (digits as f32 * char_width) + gutter_padding * 2.0;
        
        Self {
            font_id,
            line_height,
            char_width,
            gutter_width,
            gutter_padding: gutter_padding,
        }
    }
}

fn screen_to_editor_pos(
    pos: Pos2,
    rect: &Rect,
    metrics: &EditorMetrics,
    editor: &Editor,
    visible_lines: &[usize],
) -> (usize, usize, usize) {
    let rel_y = pos.y - rect.top() + editor.scroll_y;
    let vis_line_idx = (rel_y / metrics.line_height).floor() as usize; 
    
    let line_idx = if !visible_lines.is_empty() {
        if vis_line_idx < visible_lines.len() {
            visible_lines[vis_line_idx]
        } else {
            *visible_lines.last().unwrap()
        }
    } else {
        0
    };
    
    let rel_x = pos.x - (rect.left() + metrics.gutter_width + 4.0) + editor.scroll_x;
    let col = (rel_x / metrics.char_width).round().max(0.0) as usize;
    
    let line_len = editor.line_text(line_idx).chars().count();
    let col = col.min(line_len);
    
    (line_idx, col, vis_line_idx)
}

fn handle_keyboard(
    ui: &egui::Ui,
    editor: &mut Editor,
    _clipboard: &mut Option<Clipboard>,
    visible_lines_count: usize,
    _line_height: f32,
) -> bool {
    let mut changed = false;
    let events = ui.input(|i| i.events.clone());
    
    for event in events {
        match event {
            egui::Event::Text(text) => {
                if !ui.input(|i| i.modifiers.command) {
                    editor.insert_text(&text);
                    changed = true;
                }
            }
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                match key {
                    egui::Key::Enter => {
                        editor.insert_newline();
                        changed = true;
                    }
                    egui::Key::Backspace => {
                        if modifiers.alt || modifiers.mac_cmd {
                             editor.delete_word_backward();
                        } else {
                             editor.backspace();
                        }
                        changed = true;
                    }
                    egui::Key::Delete => {
                         if modifiers.alt {
                             editor.delete_word_forward();
                         } else {
                             editor.delete_forward();
                         }
                        changed = true;
                    }
                    egui::Key::ArrowLeft => {
                        if modifiers.alt {
                            editor.move_word_left(modifiers.shift);
                        } else if modifiers.mac_cmd {
                             editor.move_to_start(modifiers.shift);
                        } else {
                            editor.move_left(modifiers.shift);
                        }
                    }
                    egui::Key::ArrowRight => {
                        if modifiers.alt {
                             editor.move_word_right(modifiers.shift);
                         } else if modifiers.mac_cmd {
                              editor.move_to_end(modifiers.shift);
                         } else {
                             editor.move_right(modifiers.shift);
                         }
                    }
                    egui::Key::ArrowUp => editor.move_up(modifiers.shift),
                    egui::Key::ArrowDown => editor.move_down(modifiers.shift),
                    egui::Key::PageUp => editor.move_page_up(modifiers.shift, visible_lines_count),
                    egui::Key::PageDown => editor.move_page_down(modifiers.shift, visible_lines_count),
                    egui::Key::Home => editor.move_home(modifiers.shift),
                    egui::Key::End => editor.move_end(modifiers.shift),
                    egui::Key::Tab => {
                        if !modifiers.shift {
                             editor.insert_tab();
                             changed = true;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    changed
}

fn compute_fold_ranges(_editor: &Editor) -> std::collections::HashMap<usize, usize> {
    std::collections::HashMap::new()
}

fn build_visible_lines(
    total_lines: usize,
    collapsed_ranges: &[(usize, usize)],
) -> (Vec<usize>, std::collections::HashMap<usize, usize>) {
    let mut visible = Vec::with_capacity(total_lines);
    let mut line_to_vis = std::collections::HashMap::new();
    
    let mut i = 0;
    while i < total_lines {
        // Skip collapsed ranges - simple check
        let mut jump = 0;
        for (start, end) in collapsed_ranges {
            if i == *start {
                jump = *end - *start;
                break;
            }
        }
        
        if jump > 0 {
            line_to_vis.insert(i, visible.len());
            visible.push(i);
            i += jump;
        } else {
            line_to_vis.insert(i, visible.len());
            visible.push(i);
            i += 1;
        }
    }
    (visible, line_to_vis)
}

fn cursor_visual_y(
    pos: crate::editor::Position,
    line_to_visible: &std::collections::HashMap<usize, usize>,
    line_height: f32,
) -> f32 {
    if let Some(vis) = line_to_visible.get(&pos.line) {
         *vis as f32 * line_height
    } else {
         pos.line as f32 * line_height 
    }
}

fn read_diff_hunks(_path: Option<&std::path::Path>) -> Vec<DiffHunk> {
    Vec::new() // Stub to avoid git dependency issues for now
}

fn render_minimap_fixed(
    ui: &mut egui::Ui,
    rect: Rect,
    editor: &Editor,
    theme: &EditorTheme,
    minimap_scroll_y: f32,
    tokens: &[Vec<StyledToken>],
    search_query: Option<&str>,
    diff_hunks: &[DiffHunk],
    _fold_map: &std::collections::HashMap<usize, usize>,
    _total_visible_lines: usize,
    editor_line_height: f32,
    editor_height: f32,
) {
    let painter = ui.painter_at(rect);
    ui.set_clip_rect(rect); // Ensure we don't draw outside bounds
    
    let opacity = editor.minimap_opacity.clamp(0.2, 1.0);
    let line_count = tokens.len();
    
    // Calculate which lines are visible in the minimap rect
    let start_line = (minimap_scroll_y / MINIMAP_LINE_HEIGHT).floor() as usize;
    let visible_count = (rect.height() / MINIMAP_LINE_HEIGHT).ceil() as usize + 1;
    let end_line = (start_line + visible_count).min(line_count);
    
    // Draw Text tokens
    for line_idx in start_line..end_line {
        if let Some(token_line) = tokens.get(line_idx) {
             let y = rect.top() + (line_idx as f32 * MINIMAP_LINE_HEIGHT) - minimap_scroll_y;
             let mut current_col = 0;
             
             for token in token_line {
                 let text_len = token.text.chars().count();
                 if text_len == 0 { continue; }
                 
                 let width = text_len as f32 * MINIMAP_CHAR_WIDTH;
                 let x = rect.left() + (current_col as f32 * MINIMAP_CHAR_WIDTH);
                 
                 if x + width > rect.right() {
                     // Clip if too wide
                     break; 
                 }
                 
                 // Simple rect for "text"
                 let color = tint_for_minimap(token.color, theme, opacity);
                 painter.rect_filled(
                     Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, MINIMAP_CHAR_HEIGHT)),
                     0.0,
                     color
                 );
                 
                 current_col += text_len;
             }
        }
    }
    
    // Draw Viewport Overlay
    {
        // Viewport represents the editor's current view
        let viewport_start_line = editor.scroll_y / editor_line_height;
        let viewport_end_line = (editor.scroll_y + editor_height) / editor_line_height;
        
        let vp_y1 = rect.top() + (viewport_start_line * MINIMAP_LINE_HEIGHT) - minimap_scroll_y;
        let vp_y2 = rect.top() + (viewport_end_line * MINIMAP_LINE_HEIGHT) - minimap_scroll_y;
        
        // Clamp to rect
        // We allow it to be slightly off if scrolling
        let vp_rect = Rect::from_min_max(
            Pos2::new(rect.left(), vp_y1),
            Pos2::new(rect.right(), vp_y2)
        );
        
        // Only draw if overlapping
        if vp_rect.intersects(rect) {
            let visible_vp = vp_rect.intersect(rect);
             painter.rect_filled(
                visible_vp,
                0.0,
                apply_minimap_opacity(theme.minimap_viewport, opacity),
            );
            painter.rect_stroke(
                visible_vp,
                0.0,
                Stroke::new(1.0, apply_minimap_opacity(theme.minimap_border, opacity)),
            );
        }
    }
    
    // Draw Diff Hunks (Simplified)
    for hunk in diff_hunks {
        // ... (reuse similar logic but adapted for fixed co-ords) ...
        let start_y = rect.top() + (hunk.start as f32 * MINIMAP_LINE_HEIGHT) - minimap_scroll_y;
        let end_y = rect.top() + (hunk.end as f32 * MINIMAP_LINE_HEIGHT) - minimap_scroll_y;
        
        if start_y > rect.bottom() || end_y < rect.top() { continue; }
        
        let color = match hunk.kind {
            DiffKind::Added => Color32::from_rgb(75, 185, 116),
            DiffKind::Removed => Color32::from_rgb(214, 90, 90),
            DiffKind::Modified => Color32::from_rgb(220, 170, 80),
        };
        
        let marker_rect = Rect::from_min_max(
             Pos2::new(rect.right() - 3.0, start_y.max(rect.top())),
             Pos2::new(rect.right(), end_y.min(rect.bottom()))
        );
        
        painter.rect_filled(marker_rect, 0.0, apply_minimap_opacity(color, opacity));
    }
    
    // Draw Search Matches
     if let Some(query) = search_query {
        if !query.is_empty() {
            // This is expensive to re-scan, ideally passed in, but for now we iterate visible range
             for line_idx in start_line..end_line {
                let line_text = editor.line_text(line_idx);
                if line_text.contains(query) {
                     let y = rect.top() + (line_idx as f32 * MINIMAP_LINE_HEIGHT) - minimap_scroll_y;
                     let marker = Rect::from_min_size(
                        Pos2::new(rect.left(), y),
                        Vec2::new(rect.width(), MINIMAP_LINE_HEIGHT),
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
