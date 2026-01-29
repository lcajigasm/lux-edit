use ropey::Rope;
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
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    /// Timestamp of last edit/keystroke (seconds since epoch via std::time)
    pub last_edit_time: f64,
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
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit_time: 0.0,
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let content = fs::read_to_string(&path)?;
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
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
        })
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if let Some(path) = &self.file_path {
            fs::write(path, self.rope.to_string())?;
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
        fs::write(&path, self.rope.to_string())?;
        self.title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        self.file_path = Some(path);
        self.modified = false;
        Ok(())
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

    pub fn insert_text(&mut self, text: &str) {
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
    }

    pub fn backspace(&mut self) {
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
    }

    pub fn delete_forward(&mut self) {
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
    }

    pub fn insert_newline(&mut self) {
        // Auto-indent: match previous line indentation and add extra for openers
        let line = self.cursors[0].pos.line;
        let line_text = self.line_text(line);
        let indent: String = line_text.chars().take_while(|c| c.is_whitespace()).collect();

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
        self.insert_text("    ");
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
                while col > 0 && chars.get(col - 1).map_or(false, |c| !c.is_alphanumeric() && *c != '_') {
                    col -= 1;
                }
                // Skip word chars backwards
                while col > 0 && chars.get(col - 1).map_or(false, |c| c.is_alphanumeric() || *c == '_') {
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
                    self.cursors[idx].pos.col = line_len_chars(&self.rope, self.cursors[idx].pos.line);
                }
            } else {
                let start_col = col;
                while col > 0 && chars.get(col - 1).map_or(false, |c| !c.is_alphanumeric() && *c != '_') {
                    col -= 1;
                }
                while col > 0 && chars.get(col - 1).map_or(false, |c| c.is_alphanumeric() || *c == '_') {
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
    }

    // --- Multi-cursor ---

    pub fn add_cursor_at(&mut self, line: usize, col: usize) {
        let line = line.min(self.rope.len_lines().saturating_sub(1));
        let col = col.min(line_len_chars(&self.rope, line));
        // Don't add duplicate
        if !self.cursors.iter().any(|c| c.pos.line == line && c.pos.col == col) {
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

    pub fn clear_extra_cursors(&mut self) {
        self.cursors.truncate(1);
        self.cursors[0].anchor = None;
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
        }
        text
    }

    // --- Search ---

    pub fn find_and_select(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let full = self.rope.to_string();
        let primary_ci = pos_to_char_idx(&self.rope, &self.cursors[0].pos);

        // Search forward from cursor
        let found = full[primary_ci..]
            .find(query)
            .map(|o| primary_ci + o)
            .or_else(|| full[..primary_ci].find(query)); // Wrap around

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
    }

    // --- Go to line ---

    pub fn goto_line(&mut self, line_number: usize) {
        let line = line_number.saturating_sub(1).min(self.rope.len_lines().saturating_sub(1));
        self.cursors.truncate(1);
        self.cursors[0].pos = Position::new(line, 0);
        self.cursors[0].anchor = None;
        self.cursors[0].desired_col = 0;
        self.scroll_y = (line as f32 * LINE_HEIGHT).max(0.0);
    }
}
