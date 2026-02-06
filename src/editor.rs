use ropey::Rope;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

pub const LINE_HEIGHT: f32 = 20.0;

// --- Position & Cursor ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl Position {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.line, self.col).cmp(&(other.line, other.col))
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Added,
    Removed,
    Modified,
}

#[derive(Clone, Copy, Debug)]
pub struct DiffHunk {
    pub start: usize,
    pub end: usize,
    pub kind: DiffKind,
}

#[derive(Clone, Debug, Default)]
pub struct InlineBlameEntry {
    pub commit_short: String,
    pub author: String,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct CodeLensMetric {
    pub line: usize,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub label: String,
    pub insert_text: String,
    pub detail: String,
    pub is_snippet: bool,
}

#[derive(Clone, Debug)]
pub struct DiagnosticItem {
    pub line: usize,
    pub severity: u8,
    pub message: String,
}

#[derive(Clone, Debug)]
enum MacroAction {
    InsertText(String),
    Backspace,
    DeleteForward,
}

#[derive(Clone, Debug)]
pub struct Cursor {
    pub pos: Position,
    pub anchor: Option<Position>,
    pub desired_col: usize,
}

impl Cursor {
    pub fn new(line: usize, col: usize) -> Self {
        Self {
            pos: Position::new(line, col),
            anchor: None,
            desired_col: col,
        }
    }

    pub fn selection_ordered(&self) -> Option<(Position, Position)> {
        self.anchor.as_ref().map(|anchor| {
            if self.pos <= *anchor {
                (self.pos.clone(), anchor.clone())
            } else {
                (anchor.clone(), self.pos.clone())
            }
        })
    }
}

// --- Helper ---

fn line_len_chars(rope: &Rope, line: usize) -> usize {
    if line >= rope.len_lines() {
        return 0;
    }
    let slice = rope.line(line);
    let len = slice.len_chars();
    // Don't count trailing newline
    if len > 0 && line < rope.len_lines() - 1 {
        len - 1
    } else {
        len
    }
}

fn pos_to_char_idx(rope: &Rope, pos: &Position) -> usize {
    let line_start = rope.line_to_char(pos.line);
    let max_col = line_len_chars(rope, pos.line);
    line_start + pos.col.min(max_col)
}

// --- Undo snapshot ---

#[derive(Clone)]
struct Snapshot {
    rope: Rope,
    cursors: Vec<Cursor>,
}

// --- Editor ---

pub struct Editor {
    pub rope: Rope,
    pub cursors: Vec<Cursor>,
    pub file_path: Option<PathBuf>,
    pub modified: bool,
    pub scroll_y: f32,
    pub scroll_x: f32,
    pub title: String,
    pub pinned: bool,
    pub folded_lines: BTreeSet<usize>,
    pub minimap_enabled: bool,
    pub minimap_width: f32,
    pub minimap_opacity: f32,
    pub markdown_preview: bool,
    pub syntax_override: Option<String>,
    pub diff_hunks: Vec<DiffHunk>,
    pub diff_last_check: f64,
    pub indent_style: IndentStyle,
    pub indent_width: usize,
    pub line_ending: LineEnding,
    pub encoding: TextEncoding,
    pub inline_blame: Vec<InlineBlameEntry>,
    pub inline_blame_last_check: f64,
    pub code_lens_metrics: Vec<CodeLensMetric>,
    pub completion_items: Vec<CompletionItem>,
    pub completion_visible: bool,
    pub diagnostics: Vec<DiagnosticItem>,
    pub lsp_status: String,
    pub lsp_last_check: f64,
    pub request_completion: bool,
    pub request_formatting: bool,
    pub request_definition: bool,
    pub request_references: bool,
    pub request_implementations: bool,
    pub lsp_nav_results: Vec<String>,
    pub code_actions: Vec<String>,
    pub background_tasks: usize,
    pub notification_badges: usize,
    pub macro_recording: bool,
    pub has_macro: bool,
    macro_playing: bool,
    macro_actions: Vec<MacroAction>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    /// Timestamp of last edit/keystroke (seconds since epoch via std::time)
    pub last_edit_time: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces,
    Tabs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    Utf8Bom,
}

fn detect_line_ending(content: &str) -> LineEnding {
    if content.contains("\r\n") {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n")
}

fn detect_indent_style(content: &str) -> (IndentStyle, usize) {
    let mut tab_hits = 0usize;
    let mut space_indents: Vec<usize> = Vec::new();
    for line in content.lines().take(200) {
        if line.is_empty() {
            continue;
        }
        let mut spaces = 0usize;
        for ch in line.chars() {
            match ch {
                '\t' => {
                    tab_hits += 1;
                    break;
                }
                ' ' => spaces += 1,
                _ => {
                    if spaces > 0 {
                        space_indents.push(spaces);
                    }
                    break;
                }
            }
        }
    }

    if tab_hits > 0 {
        return (IndentStyle::Tabs, 4);
    }

    let mut best = 4usize;
    for candidate in [2usize, 4, 8] {
        if space_indents.iter().any(|v| *v == candidate) {
            best = candidate;
            break;
        }
    }
    (IndentStyle::Spaces, best)
}

impl Editor {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            cursors: vec![Cursor::new(0, 0)],
            file_path: None,
            modified: false,
            scroll_y: 0.0,
            scroll_x: 0.0,
            title: "Untitled".into(),
            pinned: false,
            folded_lines: BTreeSet::new(),
            minimap_enabled: true,
            minimap_width: 120.0,
            minimap_opacity: 0.9,
            markdown_preview: false,
            syntax_override: None,
            diff_hunks: Vec::new(),
            diff_last_check: 0.0,
            indent_style: IndentStyle::Spaces,
            indent_width: 4,
            line_ending: LineEnding::Lf,
            encoding: TextEncoding::Utf8,
            inline_blame: Vec::new(),
            inline_blame_last_check: 0.0,
            code_lens_metrics: Vec::new(),
            completion_items: Vec::new(),
            completion_visible: false,
            diagnostics: Vec::new(),
            lsp_status: "LSP: idle".to_string(),
            lsp_last_check: 0.0,
            request_completion: false,
            request_formatting: false,
            request_definition: false,
            request_references: false,
            request_implementations: false,
            lsp_nav_results: Vec::new(),
            code_actions: Vec::new(),
            background_tasks: 0,
            notification_badges: 0,
            macro_recording: false,
            has_macro: false,
            macro_playing: false,
            macro_actions: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit_time: 0.0,
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let mut content = fs::read_to_string(&path)?;
        let mut encoding = TextEncoding::Utf8;
        if content.starts_with('\u{FEFF}') {
            encoding = TextEncoding::Utf8Bom;
            content = content.trim_start_matches('\u{FEFF}').to_string();
        }
        let line_ending = detect_line_ending(&content);
        let content = normalize_line_endings(&content);
        let (indent_style, indent_width) = detect_indent_style(&content);
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        let markdown_preview = is_markdown_path(Some(path.as_path()));
        Ok(Self {
            rope: Rope::from_str(&content),
            cursors: vec![Cursor::new(0, 0)],
            file_path: Some(path),
            modified: false,
            scroll_y: 0.0,
            scroll_x: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit_time: 0.0,
            title,
            pinned: false,
            folded_lines: BTreeSet::new(),
            minimap_enabled: true,
            minimap_width: 120.0,
            minimap_opacity: 0.9,
            markdown_preview,
            syntax_override: None,
            diff_hunks: Vec::new(),
            diff_last_check: 0.0,
            indent_style,
            indent_width,
            line_ending,
            encoding,
            inline_blame: Vec::new(),
            inline_blame_last_check: 0.0,
            code_lens_metrics: Vec::new(),
            completion_items: Vec::new(),
            completion_visible: false,
            diagnostics: Vec::new(),
            lsp_status: "LSP: idle".to_string(),
            lsp_last_check: 0.0,
            request_completion: false,
            request_formatting: false,
            request_definition: false,
            request_references: false,
            request_implementations: false,
            lsp_nav_results: Vec::new(),
            code_actions: Vec::new(),
            background_tasks: 0,
            notification_badges: 0,
            macro_recording: false,
            has_macro: false,
            macro_playing: false,
            macro_actions: Vec::new(),
        })
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if let Some(path) = &self.file_path {
            fs::write(path, self.serialized_contents())?;
            self.modified = false;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No file path set",
            ))
        }
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        fs::write(&path, self.serialized_contents())?;
        self.title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        self.file_path = Some(path);
        self.markdown_preview = is_markdown_path(self.file_path.as_ref().map(|p| p.as_path()));
        self.modified = false;
        Ok(())
    }

    pub fn is_markdown(&self) -> bool {
        is_markdown_path(self.file_path.as_ref().map(|p| p.as_path()))
    }

    pub fn first_line_text(&self) -> Option<String> {
        if self.rope.len_lines() == 0 {
            return None;
        }
        let line = self.rope.line(0).to_string();
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    fn serialized_contents(&self) -> String {
        let mut content = self.rope.to_string();
        if self.line_ending == LineEnding::CrLf {
            content = content.replace('\n', "\r\n");
        }
        if self.encoding == TextEncoding::Utf8Bom {
            format!("\u{FEFF}{}", content)
        } else {
            content
        }
    }

    // --- Undo/Redo ---

    fn save_undo(&mut self) {
        self.undo_stack.push(Snapshot {
            rope: self.rope.clone(),
            cursors: self.cursors.clone(),
        });
        // Cap at 500 entries
        if self.undo_stack.len() > 500 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(Snapshot {
                rope: self.rope.clone(),
                cursors: self.cursors.clone(),
            });
            self.rope = snap.rope;
            self.cursors = snap.cursors;
            self.modified = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(Snapshot {
                rope: self.rope.clone(),
                cursors: self.cursors.clone(),
            });
            self.rope = snap.rope;
            self.cursors = snap.cursors;
            self.modified = true;
        }
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line_text(&self, line: usize) -> String {
        if line >= self.rope.len_lines() {
            return String::new();
        }
        let mut s = self.rope.line(line).to_string();
        if s.ends_with('\n') {
            s.pop();
        }
        if s.ends_with('\r') {
            s.pop();
        }
        s
    }

    // --- Editing operations ---

    /// Indices sorted in reverse document order for safe multi-cursor edits.
    fn sorted_cursor_indices_rev(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by(|&a, &b| {
            let pa = &self.cursors[a].pos;
            let pb = &self.cursors[b].pos;
            pb.cmp(pa)
        });
        indices
    }

    fn delete_selection_at(&mut self, idx: usize) -> bool {
        let sel = self.cursors[idx].selection_ordered();
        if let Some((start, end)) = sel {
            let start_ci = pos_to_char_idx(&self.rope, &start);
            let end_ci = pos_to_char_idx(&self.rope, &end);
            if start_ci < end_ci {
                self.rope.remove(start_ci..end_ci);
            }
            self.cursors[idx].pos = start;
            self.cursors[idx].anchor = None;
            self.cursors[idx].desired_col = start.col;
            true
        } else {
            false
        }
    }

    fn record_macro_action(&mut self, action: MacroAction) {
        if self.macro_recording && !self.macro_playing {
            self.macro_actions.push(action);
            self.has_macro = !self.macro_actions.is_empty();
        }
    }

    pub fn toggle_macro_recording(&mut self) {
        if self.macro_recording {
            self.macro_recording = false;
            self.has_macro = !self.macro_actions.is_empty();
            return;
        }
        self.macro_actions.clear();
        self.has_macro = false;
        self.macro_recording = true;
    }

    pub fn play_last_macro(&mut self) {
        if self.macro_actions.is_empty() || self.macro_playing {
            return;
        }
        self.macro_playing = true;
        let actions = self.macro_actions.clone();
        for action in actions {
            match action {
                MacroAction::InsertText(text) => self.insert_text(&text),
                MacroAction::Backspace => self.backspace(),
                MacroAction::DeleteForward => self.delete_forward(),
            }
        }
        self.macro_playing = false;
    }

    pub fn add_column_cursors_from_selection(&mut self) {
        if self.cursors.is_empty() {
            return;
        }
        let Some((start, end)) = self.cursors[0].selection_ordered() else {
            return;
        };
        if start.line == end.line {
            return;
        }

        let target_col = end.col;
        let mut cursors = Vec::new();
        for line in start.line..=end.line {
            let max_col = line_len_chars(&self.rope, line);
            let col = target_col.min(max_col);
            cursors.push(Cursor::new(line, col));
        }
        if !cursors.is_empty() {
            self.cursors = cursors;
        }
    }

    pub fn apply_completion(&mut self, idx: usize) {
        let Some(item) = self.completion_items.get(idx).cloned() else {
            return;
        };
        self.insert_text(&strip_snippet_placeholders(&item.insert_text));
        self.completion_visible = false;
    }

    pub fn set_document_text(&mut self, text: &str) {
        self.save_undo();
        self.rope = Rope::from_str(text);
        self.modified = true;
        self.completion_visible = false;
        self.mark_edit();
        let max_line = self.rope.len_lines().saturating_sub(1);
        for cursor in &mut self.cursors {
            cursor.pos.line = cursor.pos.line.min(max_line);
            let ll = line_len_chars(&self.rope, cursor.pos.line);
            cursor.pos.col = cursor.pos.col.min(ll);
            cursor.desired_col = cursor.pos.col;
            cursor.anchor = None;
        }
    }

    pub fn insert_text(&mut self, text: &str) {
        self.record_macro_action(MacroAction::InsertText(text.to_string()));
        self.save_undo();
        let order = self.sorted_cursor_indices_rev();
        for &idx in &order {
            self.delete_selection_at(idx);
            let ci = pos_to_char_idx(&self.rope, &self.cursors[idx].pos);
            self.rope.insert(ci, text);

            let newlines: usize = text.chars().filter(|&c| c == '\n').count();
            if newlines > 0 {
                self.cursors[idx].pos.line += newlines;
                let last_segment = text.rsplit('\n').next().unwrap_or("");
                self.cursors[idx].pos.col = last_segment.chars().count();
            } else {
                self.cursors[idx].pos.col += text.chars().count();
            }
            self.cursors[idx].desired_col = self.cursors[idx].pos.col;
        }
        self.modified = true;
        self.mark_edit();
    }

    pub fn backspace(&mut self) {
        self.record_macro_action(MacroAction::Backspace);
        self.save_undo();
        let order = self.sorted_cursor_indices_rev();
        for &idx in &order {
            if self.delete_selection_at(idx) {
                continue;
            }
            let pos = &self.cursors[idx].pos;
            if pos.line == 0 && pos.col == 0 {
                continue;
            }
            let ci = pos_to_char_idx(&self.rope, pos);
            if ci == 0 {
                continue;
            }
            self.rope.remove(ci - 1..ci);

            if self.cursors[idx].pos.col == 0 {
                self.cursors[idx].pos.line -= 1;
                self.cursors[idx].pos.col = line_len_chars(&self.rope, self.cursors[idx].pos.line);
            } else {
                self.cursors[idx].pos.col -= 1;
            }
            self.cursors[idx].desired_col = self.cursors[idx].pos.col;
        }
        self.modified = true;
        self.mark_edit();
    }

    pub fn delete_forward(&mut self) {
        self.record_macro_action(MacroAction::DeleteForward);
        self.save_undo();
        let order = self.sorted_cursor_indices_rev();
        for &idx in &order {
            if self.delete_selection_at(idx) {
                continue;
            }
            let ci = pos_to_char_idx(&self.rope, &self.cursors[idx].pos);
            if ci >= self.rope.len_chars() {
                continue;
            }
            self.rope.remove(ci..ci + 1);
        }
        self.modified = true;
        self.mark_edit();
    }

    pub fn insert_newline(&mut self) {
        // Auto-indent: match previous line indentation and add extra for openers
        let line = self.cursors[0].pos.line;
        let line_text = self.line_text(line);
        let indent: String = line_text
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();

        let trimmed = line_text.trim_end();
        let extra_indent = if trimmed.ends_with('{')
            || trimmed.ends_with('(')
            || trimmed.ends_with('[')
            || trimmed.ends_with(':')
        {
            "    "
        } else {
            ""
        };

        let mut newline = String::from("\n");
        newline.push_str(&indent);
        newline.push_str(extra_indent);
        self.insert_text(&newline);
    }

    pub fn insert_tab(&mut self) {
        match self.indent_style {
            IndentStyle::Tabs => self.insert_text("\t"),
            IndentStyle::Spaces => {
                let spaces = " ".repeat(self.indent_width.max(1));
                self.insert_text(&spaces);
            }
        }
    }

    // --- Cursor movement ---

    pub fn move_left(&mut self, select: bool) {
        let rope = &self.rope;
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos.clone());
            } else if !select {
                // If there's a selection and not extending, collapse to start
                if let Some(anchor) = cursor.anchor.take() {
                    cursor.pos = cursor.pos.clone().min(anchor);
                    cursor.desired_col = cursor.pos.col;
                    continue;
                }
            }

            if cursor.pos.col > 0 {
                cursor.pos.col -= 1;
            } else if cursor.pos.line > 0 {
                cursor.pos.line -= 1;
                cursor.pos.col = line_len_chars(rope, cursor.pos.line);
            }
            cursor.desired_col = cursor.pos.col;
        }
    }

    pub fn move_right(&mut self, select: bool) {
        let rope = &self.rope;
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos.clone());
            } else if !select {
                if let Some(anchor) = cursor.anchor.take() {
                    cursor.pos = cursor.pos.clone().max(anchor);
                    cursor.desired_col = cursor.pos.col;
                    continue;
                }
            }

            let ll = line_len_chars(rope, cursor.pos.line);
            if cursor.pos.col < ll {
                cursor.pos.col += 1;
            } else if cursor.pos.line < rope.len_lines().saturating_sub(1) {
                cursor.pos.line += 1;
                cursor.pos.col = 0;
            }
            cursor.desired_col = cursor.pos.col;
        }
    }

    pub fn move_up(&mut self, select: bool) {
        let rope = &self.rope;
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos.clone());
            } else if !select {
                cursor.anchor = None;
            }

            if cursor.pos.line > 0 {
                cursor.pos.line -= 1;
                let ll = line_len_chars(rope, cursor.pos.line);
                cursor.pos.col = cursor.desired_col.min(ll);
            }
        }
    }

    pub fn move_down(&mut self, select: bool) {
        let rope = &self.rope;
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos.clone());
            } else if !select {
                cursor.anchor = None;
            }

            if cursor.pos.line < rope.len_lines().saturating_sub(1) {
                cursor.pos.line += 1;
                let ll = line_len_chars(rope, cursor.pos.line);
                cursor.pos.col = cursor.desired_col.min(ll);
            }
        }
    }

    pub fn move_home(&mut self, select: bool) {
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos.clone());
            } else if !select {
                cursor.anchor = None;
            }
            cursor.pos.col = 0;
            cursor.desired_col = 0;
        }
    }

    pub fn move_end(&mut self, select: bool) {
        let rope = &self.rope;
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos.clone());
            } else if !select {
                cursor.anchor = None;
            }
            cursor.pos.col = line_len_chars(rope, cursor.pos.line);
            cursor.desired_col = cursor.pos.col;
        }
    }

    pub fn move_page_up(&mut self, select: bool, visible_lines: usize) {
        let rope = &self.rope;
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos);
            } else if !select {
                cursor.anchor = None;
            }
            cursor.pos.line = cursor.pos.line.saturating_sub(visible_lines);
            let ll = line_len_chars(rope, cursor.pos.line);
            cursor.pos.col = cursor.desired_col.min(ll);
        }
    }

    pub fn move_page_down(&mut self, select: bool, visible_lines: usize) {
        let rope = &self.rope;
        let max_line = rope.len_lines().saturating_sub(1);
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos);
            } else if !select {
                cursor.anchor = None;
            }
            cursor.pos.line = (cursor.pos.line + visible_lines).min(max_line);
            let ll = line_len_chars(rope, cursor.pos.line);
            cursor.pos.col = cursor.desired_col.min(ll);
        }
    }

    pub fn move_to_start(&mut self, select: bool) {
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos);
            } else if !select {
                cursor.anchor = None;
            }
            cursor.pos = Position::new(0, 0);
            cursor.desired_col = 0;
        }
    }

    pub fn move_to_end(&mut self, select: bool) {
        let rope = &self.rope;
        let last_line = rope.len_lines().saturating_sub(1);
        let last_col = line_len_chars(rope, last_line);
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos);
            } else if !select {
                cursor.anchor = None;
            }
            cursor.pos = Position::new(last_line, last_col);
            cursor.desired_col = last_col;
        }
    }

    // --- Word movement ---

    pub fn move_word_left(&mut self, select: bool) {
        let rope = &self.rope;
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos);
            } else if !select {
                cursor.anchor = None;
            }
            let line_text = rope.line(cursor.pos.line).to_string();
            let chars: Vec<char> = line_text.chars().collect();
            let mut col = cursor.pos.col;

            if col == 0 {
                if cursor.pos.line > 0 {
                    cursor.pos.line -= 1;
                    cursor.pos.col = line_len_chars(rope, cursor.pos.line);
                }
            } else {
                // Skip whitespace backwards
                while col > 0
                    && chars
                        .get(col - 1)
                        .map_or(false, |c| !c.is_alphanumeric() && *c != '_')
                {
                    col -= 1;
                }
                // Skip word chars backwards
                while col > 0
                    && chars
                        .get(col - 1)
                        .map_or(false, |c| c.is_alphanumeric() || *c == '_')
                {
                    col -= 1;
                }
                cursor.pos.col = col;
            }
            cursor.desired_col = cursor.pos.col;
        }
    }

    pub fn move_word_right(&mut self, select: bool) {
        let rope = &self.rope;
        for cursor in &mut self.cursors {
            if select && cursor.anchor.is_none() {
                cursor.anchor = Some(cursor.pos);
            } else if !select {
                cursor.anchor = None;
            }
            let ll = line_len_chars(rope, cursor.pos.line);
            let line_text = rope.line(cursor.pos.line).to_string();
            let chars: Vec<char> = line_text.chars().collect();
            let mut col = cursor.pos.col;

            if col >= ll {
                if cursor.pos.line < rope.len_lines().saturating_sub(1) {
                    cursor.pos.line += 1;
                    cursor.pos.col = 0;
                }
            } else {
                // Skip word chars forward
                while col < chars.len() && (chars[col].is_alphanumeric() || chars[col] == '_') {
                    col += 1;
                }
                // Skip non-word chars forward
                while col < chars.len() && !chars[col].is_alphanumeric() && chars[col] != '_' {
                    col += 1;
                }
                cursor.pos.col = col.min(ll);
            }
            cursor.desired_col = cursor.pos.col;
        }
    }

    pub fn delete_word_backward(&mut self) {
        self.save_undo();
        let order = self.sorted_cursor_indices_rev();
        for &idx in &order {
            if self.delete_selection_at(idx) {
                continue;
            }
            let pos = self.cursors[idx].pos;
            if pos.line == 0 && pos.col == 0 {
                continue;
            }
            // Find word start
            let line_text = self.line_text(pos.line);
            let chars: Vec<char> = line_text.chars().collect();
            let mut col = pos.col;
            if col == 0 {
                // Merge with previous line
                let ci = pos_to_char_idx(&self.rope, &pos);
                if ci > 0 {
                    self.rope.remove(ci - 1..ci);
                    self.cursors[idx].pos.line -= 1;
                    self.cursors[idx].pos.col =
                        line_len_chars(&self.rope, self.cursors[idx].pos.line);
                }
            } else {
                let start_col = col;
                while col > 0
                    && chars
                        .get(col - 1)
                        .map_or(false, |c| !c.is_alphanumeric() && *c != '_')
                {
                    col -= 1;
                }
                while col > 0
                    && chars
                        .get(col - 1)
                        .map_or(false, |c| c.is_alphanumeric() || *c == '_')
                {
                    col -= 1;
                }
                let start_ci = self.rope.line_to_char(pos.line) + col;
                let end_ci = self.rope.line_to_char(pos.line) + start_col;
                self.rope.remove(start_ci..end_ci);
                self.cursors[idx].pos.col = col;
            }
            self.cursors[idx].desired_col = self.cursors[idx].pos.col;
        }
        self.modified = true;
        self.mark_edit();
    }

    pub fn delete_word_forward(&mut self) {
        self.save_undo();
        let order = self.sorted_cursor_indices_rev();
        for &idx in &order {
            if self.delete_selection_at(idx) {
                continue;
            }
            let pos = self.cursors[idx].pos;
            let ll = line_len_chars(&self.rope, pos.line);
            if pos.col >= ll {
                // Merge with next line
                let ci = pos_to_char_idx(&self.rope, &pos);
                if ci < self.rope.len_chars() {
                    self.rope.remove(ci..ci + 1);
                }
            } else {
                let line_text = self.line_text(pos.line);
                let chars: Vec<char> = line_text.chars().collect();
                let mut col = pos.col;
                while col < chars.len() && (chars[col].is_alphanumeric() || chars[col] == '_') {
                    col += 1;
                }
                while col < chars.len() && !chars[col].is_alphanumeric() && chars[col] != '_' {
                    col += 1;
                }
                let start_ci = self.rope.line_to_char(pos.line) + pos.col;
                let end_ci = self.rope.line_to_char(pos.line) + col;
                self.rope.remove(start_ci..end_ci);
            }
        }
        self.modified = true;
        self.mark_edit();
    }

    // --- Multi-cursor ---

    pub fn add_cursor_at(&mut self, line: usize, col: usize) {
        let line = line.min(self.rope.len_lines().saturating_sub(1));
        let col = col.min(line_len_chars(&self.rope, line));
        // Don't add duplicate
        if !self
            .cursors
            .iter()
            .any(|c| c.pos.line == line && c.pos.col == col)
        {
            self.cursors.push(Cursor::new(line, col));
        }
    }

    /// Select next occurrence of current word/selection (Ctrl+D behavior)
    pub fn select_next_occurrence(&mut self) {
        let primary = &self.cursors[0];

        // Get the selected text, or the word under cursor
        let search_text = if let Some((start, end)) = primary.selection_ordered() {
            let start_ci = pos_to_char_idx(&self.rope, &start);
            let end_ci = pos_to_char_idx(&self.rope, &end);
            self.rope.slice(start_ci..end_ci).to_string()
        } else {
            self.word_at_cursor(primary)
        };

        if search_text.is_empty() {
            return;
        }

        // If no selection on primary, select the current word first
        if self.cursors[0].anchor.is_none() {
            let (ws, we) = self.word_bounds_at_cursor(&self.cursors[0]);
            self.cursors[0].anchor = Some(ws);
            self.cursors[0].pos = we;
            self.cursors[0].desired_col = self.cursors[0].pos.col;
            return;
        }

        // Find the next occurrence after the last cursor
        let last_cursor = self
            .cursors
            .iter()
            .max_by_key(|c| (c.pos.line, c.pos.col))
            .unwrap();
        let start_ci = pos_to_char_idx(&self.rope, &last_cursor.pos);
        let full_text = self.rope.to_string();

        if let Some(offset) = full_text[start_ci..].find(&search_text) {
            let match_start_ci = start_ci + offset;
            let match_end_ci = match_start_ci + search_text.len();

            let start_line = self.rope.char_to_line(match_start_ci);
            let start_col = match_start_ci - self.rope.line_to_char(start_line);
            let end_line = self.rope.char_to_line(match_end_ci);
            let end_col = match_end_ci - self.rope.line_to_char(end_line);

            let mut new_cursor = Cursor::new(end_line, end_col);
            new_cursor.anchor = Some(Position::new(start_line, start_col));
            self.cursors.push(new_cursor);
        }
    }

    /// Select all occurrences of current word/selection (Ctrl+Shift+L behavior)
    pub fn select_all_occurrences(&mut self) {
        let primary = &self.cursors[0];
        let search_text = if let Some((start, end)) = primary.selection_ordered() {
            let start_ci = pos_to_char_idx(&self.rope, &start);
            let end_ci = pos_to_char_idx(&self.rope, &end);
            self.rope.slice(start_ci..end_ci).to_string()
        } else {
            self.word_at_cursor(primary)
        };

        if search_text.is_empty() {
            return;
        }

        let full_text = self.rope.to_string();
        let mut cursors = Vec::new();
        let mut start = 0usize;
        while let Some(offset) = full_text[start..].find(&search_text) {
            let match_start_ci = start + offset;
            let match_end_ci = match_start_ci + search_text.len();
            let start_line = self.rope.char_to_line(match_start_ci);
            let start_col = match_start_ci - self.rope.line_to_char(start_line);
            let end_line = self.rope.char_to_line(match_end_ci);
            let end_col = match_end_ci - self.rope.line_to_char(end_line);

            let mut cursor = Cursor::new(end_line, end_col);
            cursor.anchor = Some(Position::new(start_line, start_col));
            cursors.push(cursor);
            start = match_end_ci;
        }

        if !cursors.is_empty() {
            self.cursors = cursors;
        }
    }

    pub fn add_cursors_vertical(&mut self, delta: isize) {
        if delta == 0 {
            return;
        }
        let max_line = self.rope.len_lines().saturating_sub(1);
        let current = self.cursors.clone();
        for cursor in current {
            let line = if delta.is_negative() {
                cursor.pos.line.saturating_sub(delta.wrapping_abs() as usize)
            } else {
                (cursor.pos.line + delta as usize).min(max_line)
            };
            self.add_cursor_at(line, cursor.pos.col);
        }
    }

    fn word_at_cursor(&self, cursor: &Cursor) -> String {
        let (start, end) = self.word_bounds_at_cursor(cursor);
        let start_ci = pos_to_char_idx(&self.rope, &start);
        let end_ci = pos_to_char_idx(&self.rope, &end);
        if start_ci < end_ci {
            self.rope.slice(start_ci..end_ci).to_string()
        } else {
            String::new()
        }
    }

    fn word_bounds_at_cursor(&self, cursor: &Cursor) -> (Position, Position) {
        let line_text = self.line_text(cursor.pos.line);
        let chars: Vec<char> = line_text.chars().collect();
        let col = cursor.pos.col.min(chars.len());

        if chars.is_empty() || col >= chars.len() {
            return (cursor.pos.clone(), cursor.pos.clone());
        }

        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

        if !is_word_char(chars[col]) {
            return (cursor.pos.clone(), Position::new(cursor.pos.line, col + 1));
        }

        let mut start = col;
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }
        let mut end = col;
        while end < chars.len() && is_word_char(chars[end]) {
            end += 1;
        }

        (
            Position::new(cursor.pos.line, start),
            Position::new(cursor.pos.line, end),
        )
    }

    // --- Selection helpers ---

    pub fn select_all(&mut self) {
        let last_line = self.rope.len_lines().saturating_sub(1);
        let last_col = line_len_chars(&self.rope, last_line);
        self.cursors.truncate(1);
        self.cursors[0].anchor = Some(Position::new(0, 0));
        self.cursors[0].pos = Position::new(last_line, last_col);
        self.cursors[0].desired_col = last_col;
    }

    pub fn selected_text(&self) -> String {
        if let Some((start, end)) = self.cursors[0].selection_ordered() {
            let start_ci = pos_to_char_idx(&self.rope, &start);
            let end_ci = pos_to_char_idx(&self.rope, &end);
            self.rope.slice(start_ci..end_ci).to_string()
        } else {
            String::new()
        }
    }

    pub fn symbol_at_primary_cursor(&self) -> String {
        if self.cursors.is_empty() {
            return String::new();
        }
        self.word_at_cursor(&self.cursors[0])
    }

    pub fn rename_symbol_in_document(&mut self, from: &str, to: &str) -> bool {
        if from.is_empty() || to.is_empty() || from == to {
            return false;
        }
        let original = self.rope.to_string();
        let replaced = replace_whole_word(&original, from, to);
        if replaced == original {
            return false;
        }
        self.set_document_text(&replaced);
        true
    }

    pub fn extract_variable(&mut self, name: &str) -> bool {
        let var_name = name.trim();
        if var_name.is_empty() || self.cursors.is_empty() {
            return false;
        }
        let Some((start, end)) = self.cursors[0].selection_ordered() else {
            return false;
        };
        let selected = self.selected_text();
        if selected.trim().is_empty() {
            return false;
        }

        let mut content = self.rope.to_string();
        let start_ci = pos_to_char_idx(&self.rope, &start);
        let end_ci = pos_to_char_idx(&self.rope, &end);
        if start_ci >= end_ci || end_ci > content.len() {
            return false;
        }

        let line_start_ci = self.rope.line_to_char(start.line);
        let line_prefix = content[line_start_ci..start_ci].to_string();
        let indent: String = line_prefix
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        let declaration = format!("{indent}let {var_name} = {};\n", selected.trim());
        content.insert_str(line_start_ci, &declaration);

        let shift = declaration.chars().count();
        let new_start = start_ci + shift;
        let new_end = end_ci + shift;
        content.replace_range(new_start..new_end, var_name);
        self.set_document_text(&content);
        true
    }

    pub fn extract_method(&mut self, name: &str) -> bool {
        let method_name = name.trim();
        if method_name.is_empty() || self.cursors.is_empty() {
            return false;
        }
        let Some((start, end)) = self.cursors[0].selection_ordered() else {
            return false;
        };
        let selected = self.selected_text();
        if selected.trim().is_empty() {
            return false;
        }
        let mut content = self.rope.to_string();
        let start_ci = pos_to_char_idx(&self.rope, &start);
        let end_ci = pos_to_char_idx(&self.rope, &end);
        if start_ci >= end_ci || end_ci > content.len() {
            return false;
        }

        content.replace_range(start_ci..end_ci, &format!("{method_name}();"));
        let extracted_body = selected
            .lines()
            .map(|line| format!("    {}", line))
            .collect::<Vec<_>>()
            .join("\n");
        content.push_str(&format!(
            "\n\nfn {method_name}() {{\n{extracted_body}\n}}\n"
        ));
        self.set_document_text(&content);
        true
    }

    pub fn inline_variable_at_cursor(&mut self) -> bool {
        if self.cursors.is_empty() {
            return false;
        }
        let line_idx = self.cursors[0].pos.line;
        let line = self.line_text(line_idx);
        let trimmed = line.trim();
        if !trimmed.starts_with("let ") || !trimmed.ends_with(';') || !trimmed.contains('=') {
            return false;
        }
        let after_let = trimmed.trim_start_matches("let ").trim();
        let Some((name_part, expr_part)) = after_let.split_once('=') else {
            return false;
        };
        let name = name_part.trim();
        let expr = expr_part.trim().trim_end_matches(';').trim();
        if name.is_empty() || expr.is_empty() {
            return false;
        }

        let mut content = self.rope.to_string();
        let line_start = self.rope.line_to_char(line_idx);
        let line_end = if line_idx + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line_idx + 1)
        } else {
            self.rope.len_chars()
        };
        if line_end > line_start && line_end <= content.len() {
            content.replace_range(line_start..line_end, "");
        }
        let replaced = replace_whole_word(&content, name, expr);
        self.set_document_text(&replaced);
        true
    }

    pub fn organize_imports(&mut self) -> bool {
        let path = self.file_path.as_deref();
        let mut lines: Vec<String> = self.rope.to_string().lines().map(|s| s.to_string()).collect();
        if lines.is_empty() {
            return false;
        }

        let is_import = |line: &str| {
            let t = line.trim();
            if let Some(ext) = path.and_then(|p| p.extension()).and_then(|e| e.to_str()) {
                match ext {
                    "rs" => t.starts_with("use "),
                    "py" => t.starts_with("import ") || t.starts_with("from "),
                    "js" | "jsx" | "ts" | "tsx" => t.starts_with("import "),
                    _ => t.starts_with("use ") || t.starts_with("import ") || t.starts_with("from "),
                }
            } else {
                t.starts_with("use ") || t.starts_with("import ") || t.starts_with("from ")
            }
        };

        let mut import_indices = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            if is_import(line) {
                import_indices.push(idx);
            } else if !line.trim().is_empty() && !line.trim_start().starts_with("//") {
                break;
            }
        }
        if import_indices.is_empty() {
            return false;
        }

        let mut imports: Vec<String> = import_indices
            .iter()
            .map(|i| lines[*i].trim().to_string())
            .collect();
        imports.sort();
        imports.dedup();

        let start = *import_indices.first().unwrap_or(&0);
        let end = *import_indices.last().unwrap_or(&0);
        lines.splice(start..=end, imports);
        let new_content = lines.join("\n") + "\n";
        if new_content == self.rope.to_string() {
            return false;
        }
        self.set_document_text(&new_content);
        true
    }

    pub fn refresh_code_actions(&mut self) {
        let mut actions = Vec::new();
        if !self.diagnostics.is_empty() {
            actions.push("Quick Fix: Organize imports".to_string());
        }
        for diag in &self.diagnostics {
            let msg = diag.message.to_lowercase();
            if msg.contains("cannot find") || msg.contains("undeclared") || msg.contains("not found")
            {
                if let Some(symbol) = extract_backticked_symbol(&diag.message) {
                    actions.push(format!("Auto Import: {}", symbol));
                }
            }
            if msg.contains("unused") {
                actions.push("Quick Fix: Remove current line".to_string());
            }
        }
        actions.sort();
        actions.dedup();
        self.code_actions = actions;
    }

    pub fn apply_code_action(&mut self, action: &str) -> bool {
        if action == "Quick Fix: Organize imports" {
            return self.organize_imports();
        }
        if action == "Quick Fix: Remove current line" {
            return self.remove_current_line();
        }
        if let Some(symbol) = action.strip_prefix("Auto Import: ") {
            return self.add_import_suggestion(symbol.trim());
        }
        false
    }

    fn remove_current_line(&mut self) -> bool {
        if self.cursors.is_empty() {
            return false;
        }
        let line_idx = self.cursors[0].pos.line;
        let mut content = self.rope.to_string();
        let start = self.rope.line_to_char(line_idx);
        let end = if line_idx + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line_idx + 1)
        } else {
            self.rope.len_chars()
        };
        if start >= end || end > content.len() {
            return false;
        }
        content.replace_range(start..end, "");
        self.set_document_text(&content);
        true
    }

    fn add_import_suggestion(&mut self, symbol: &str) -> bool {
        if symbol.is_empty() {
            return false;
        }
        let mut content = self.rope.to_string();
        let import_line = format!("use crate::{};\n", symbol.to_lowercase());
        if content.contains(&import_line) {
            return false;
        }
        let insert_at = content
            .lines()
            .take_while(|line| line.trim().starts_with("use ") || line.trim().is_empty())
            .map(|line| line.len() + 1)
            .sum::<usize>();
        let insert_at = insert_at.min(content.len());
        content.insert_str(insert_at, &import_line);
        self.set_document_text(&content);
        true
    }

    /// Copy: returns selected text (or current line if no selection).
    pub fn copy_text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for cursor in &self.cursors {
            if let Some((start, end)) = cursor.selection_ordered() {
                let s = pos_to_char_idx(&self.rope, &start);
                let e = pos_to_char_idx(&self.rope, &end);
                parts.push(self.rope.slice(s..e).to_string());
            } else {
                // No selection: copy entire line
                let mut line = self.line_text(cursor.pos.line);
                line.push('\n');
                parts.push(line);
            }
        }
        parts.join("")
    }

    /// Cut: returns selected text and deletes it (or cuts current line).
    pub fn cut_text(&mut self) -> String {
        self.save_undo();
        let text = self.copy_text();
        let has_selection = self.cursors.iter().any(|c| c.anchor.is_some());
        if has_selection {
            // Delete all selections
            let order = self.sorted_cursor_indices_rev();
            for &idx in &order {
                self.delete_selection_at(idx);
            }
            self.modified = true;
            self.mark_edit();
        } else {
            // Delete entire current line
            let line = self.cursors[0].pos.line;
            let line_start = self.rope.line_to_char(line);
            let line_end = if line + 1 < self.rope.len_lines() {
                self.rope.line_to_char(line + 1)
            } else {
                self.rope.len_chars()
            };
            if line_start < line_end {
                self.rope.remove(line_start..line_end);
            }
            let new_line = line.min(self.rope.len_lines().saturating_sub(1));
            self.cursors.truncate(1);
            self.cursors[0].pos = Position::new(new_line, 0);
            self.cursors[0].anchor = None;
            self.cursors[0].desired_col = 0;
            self.modified = true;
            self.mark_edit();
        }
        text
    }

    // --- Search ---

    pub fn find_and_select(&mut self, query: &str) {
        self.find_next(query);
    }

    pub fn find_next(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let primary = &self.cursors[0];
        let start_pos = primary
            .selection_ordered()
            .map(|(_, end)| end)
            .unwrap_or(primary.pos);
        let start_ci = pos_to_char_idx(&self.rope, &start_pos);
        let full = self.rope.to_string();

        let found = full[start_ci..]
            .find(query)
            .map(|o| start_ci + o)
            .or_else(|| full[..start_ci].find(query)); // Wrap around

        if let Some(match_start) = found {
            let match_end = match_start + query.len();
            let start_line = self.rope.char_to_line(match_start);
            let start_col = match_start - self.rope.line_to_char(start_line);
            let end_line = self.rope.char_to_line(match_end);
            let end_col = match_end - self.rope.line_to_char(end_line);

            self.cursors.truncate(1);
            self.cursors[0].anchor = Some(Position::new(start_line, start_col));
            self.cursors[0].pos = Position::new(end_line, end_col);
            self.cursors[0].desired_col = end_col;

            // Scroll to match
            self.scroll_y = (start_line as f32 * LINE_HEIGHT).max(0.0);
        }
    }

    pub fn find_prev(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let primary = &self.cursors[0];
        let start_pos = primary
            .selection_ordered()
            .map(|(start, _)| start)
            .unwrap_or(primary.pos);
        let start_ci = pos_to_char_idx(&self.rope, &start_pos);
        let full = self.rope.to_string();

        let found = full[..start_ci]
            .rfind(query)
            .or_else(|| full.rfind(query)); // Wrap around

        if let Some(match_start) = found {
            let match_end = match_start + query.len();
            let start_line = self.rope.char_to_line(match_start);
            let start_col = match_start - self.rope.line_to_char(start_line);
            let end_line = self.rope.char_to_line(match_end);
            let end_col = match_end - self.rope.line_to_char(end_line);

            self.cursors.truncate(1);
            self.cursors[0].anchor = Some(Position::new(start_line, start_col));
            self.cursors[0].pos = Position::new(end_line, end_col);
            self.cursors[0].desired_col = end_col;

            // Scroll to match
            self.scroll_y = (start_line as f32 * LINE_HEIGHT).max(0.0);
        }
    }

    /// Replace the current selection (if it matches query) and find the next match.
    pub fn replace_next(&mut self, find: &str, replace: &str) {
        if find.is_empty() {
            return;
        }
        // If current selection matches find, replace it
        let selected = self.selected_text();
        if selected == find {
            self.save_undo();
            // Delete selection and insert replacement
            self.delete_selection_at(0);
            let ci = pos_to_char_idx(&self.rope, &self.cursors[0].pos);
            self.rope.insert(ci, replace);
            self.cursors[0].pos.col += replace.chars().count();
            self.cursors[0].desired_col = self.cursors[0].pos.col;
            self.modified = true;
            self.mark_edit();
        }
        // Find next occurrence
        self.find_and_select(find);
    }

    /// Replace all occurrences in the document.
    pub fn replace_all(&mut self, find: &str, replace: &str) {
        if find.is_empty() {
            return;
        }
        self.save_undo();
        let content = self.rope.to_string().replace(find, replace);
        self.rope = Rope::from_str(&content);
        // Reset cursors to safe position
        let max_line = self.rope.len_lines().saturating_sub(1);
        for cursor in &mut self.cursors {
            cursor.pos.line = cursor.pos.line.min(max_line);
            let ll = line_len_chars(&self.rope, cursor.pos.line);
            cursor.pos.col = cursor.pos.col.min(ll);
            cursor.desired_col = cursor.pos.col;
            cursor.anchor = None;
        }
        self.modified = true;
        self.mark_edit();
    }

    // --- Go to line ---

    pub fn goto_line(&mut self, line_number: usize) {
        let line = line_number
            .saturating_sub(1)
            .min(self.rope.len_lines().saturating_sub(1));
        self.cursors.truncate(1);
        self.cursors[0].pos = Position::new(line, 0);
        self.cursors[0].anchor = None;
        self.cursors[0].desired_col = 0;
        self.scroll_y = (line as f32 * LINE_HEIGHT).max(0.0);
    }
}

impl Editor {
    fn mark_edit(&mut self) {
        self.last_edit_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
    }
}

fn replace_whole_word(input: &str, from: &str, to: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let needle: Vec<char> = from.chars().collect();
    if needle.is_empty() {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    while i < chars.len() {
        let can_match = i + needle.len() <= chars.len()
            && chars[i..i + needle.len()] == needle[..];
        if can_match {
            let prev_ok = i == 0 || !is_word_char(chars[i - 1]);
            let next_ok = i + needle.len() >= chars.len() || !is_word_char(chars[i + needle.len()]);
            if prev_ok && next_ok {
                out.push_str(to);
                i += needle.len();
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn extract_backticked_symbol(input: &str) -> Option<String> {
    let start = input.find('`')?;
    let rest = &input[start + 1..];
    let end = rest.find('`')?;
    let symbol = rest[..end].trim();
    if symbol.is_empty() {
        None
    } else {
        Some(symbol.to_string())
    }
}

fn is_markdown_path(path: Option<&std::path::Path>) -> bool {
    let Some(path) = path else {
        return false;
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "md" | "markdown" | "mdown" | "mkd" | "mdx"
    )
}

fn strip_snippet_placeholders(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            if chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    chars.next();
                }
                continue;
            }
            if chars.peek() == Some(&'{') {
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '}' {
                        break;
                    }
                    if c == ':' {
                        while let Some(fallback) = chars.next() {
                            if fallback == '}' {
                                break;
                            }
                            out.push(fallback);
                        }
                        break;
                    }
                }
                continue;
            }
        }
        out.push(ch);
    }
    out
}
