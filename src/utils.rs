use std::path::{Path, PathBuf};

use eframe::egui;

use crate::editor::{CodeLensMetric, Editor};

pub fn file_icon(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "rs" => "[RS]",
        "py" => "[PY]",
        "js" | "ts" | "tsx" | "jsx" => "[JS]",
        "md" => "[MD]",
        "json" | "toml" | "yaml" | "yml" => "[CFG]",
        "sh" | "bash" | "zsh" => "[SH]",
        "html" | "css" => "[WEB]",
        _ => "[FILE]",
    }
}

pub fn symbol_score(candidate: &str, query: &str) -> Option<i32> {
    if candidate == query {
        return Some(100);
    }
    if candidate.starts_with(query) {
        return Some(80);
    }
    if let Some(idx) = candidate.find(query) {
        return Some(60 - idx as i32);
    }
    let mut score = 0i32;
    let mut cursor = 0usize;
    for ch in query.chars() {
        if let Some(found) = candidate[cursor..].find(ch) {
            score += 2;
            cursor += found + 1;
        } else {
            return None;
        }
    }
    Some(score)
}

pub fn build_simple_diff_preview(before: &str, after: &str) -> String {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let max_len = before_lines.len().max(after_lines.len());
    let mut out = String::new();
    for i in 0..max_len {
        let b = before_lines.get(i).copied();
        let a = after_lines.get(i).copied();
        match (b, a) {
            (Some(b), Some(a)) if b == a => {}
            (Some(b), Some(a)) => {
                out.push_str(&format!("- {:>4} {}\n", i + 1, b));
                out.push_str(&format!("+ {:>4} {}\n", i + 1, a));
            }
            (Some(b), None) => out.push_str(&format!("- {:>4} {}\n", i + 1, b)),
            (None, Some(a)) => out.push_str(&format!("+ {:>4} {}\n", i + 1, a)),
            (None, None) => {}
        }
    }
    if out.is_empty() {
        "No textual differences".to_string()
    } else {
        out
    }
}

pub fn parse_stacktrace_location(line: &str) -> Option<(PathBuf, usize)> {
    let trimmed = line.trim();
    let (head, tail) = trimmed.rsplit_once(':')?;
    let last_num = tail.trim().parse::<usize>().ok()?;

    if let Some((path_part, line_part)) = head.rsplit_once(':') {
        if let Ok(line_no) = line_part.trim().parse::<usize>() {
            let candidate = PathBuf::from(path_part.trim());
            if candidate.exists() {
                return Some((candidate, line_no));
            }
        }
    }

    let candidate = PathBuf::from(head.trim());
    if candidate.exists() {
        return Some((candidate, last_num));
    }
    None
}

pub fn parse_search_hit_location(hit: &str) -> Option<(String, usize)> {
    let trimmed = hit.trim();
    let mut start = 0usize;
    while start < trimmed.len() {
        let first = start + trimmed[start..].find(':')?;
        let after_first = first + 1;
        let Some(next_rel) = trimmed[after_first..].find(':') else {
            break;
        };
        let second = after_first + next_rel;
        if let Ok(line) = trimmed[after_first..second].trim().parse::<usize>() {
            let path = trimmed[..first].trim();
            if !path.is_empty() {
                return Some((path.to_string(), line));
            }
        }
        start = after_first;
    }
    None
}

pub fn parse_symbol_location(symbol: &str) -> Option<(PathBuf, usize)> {
    let trimmed = symbol.trim();
    let mut start = 0usize;
    while start < trimmed.len() {
        let first = start + trimmed[start..].find(':')?;
        let after_first = first + 1;
        let Some(next_rel) = trimmed[after_first..].find(':') else {
            break;
        };
        let second = after_first + next_rel;
        if let Ok(line) = trimmed[after_first..second].trim().parse::<usize>() {
            let path = trimmed[..first].trim();
            if !path.is_empty() {
                return Some((PathBuf::from(path), line));
            }
        }
        start = after_first;
    }
    None
}

pub fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();
    for key in ["TOKEN", "SECRET", "PASSWORD", "API_KEY"] {
        if let Some(idx) = out.to_uppercase().find(key) {
            let end = (idx + key.len() + 24).min(out.len());
            out.replace_range(idx..end, &format!("{}=[REDACTED]", key));
        }
    }
    out
}

pub fn color_to_hex(color: egui::Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

pub fn parse_hex_color(hex: &str) -> Option<egui::Color32> {
    let stripped = hex.trim().trim_start_matches('#');
    if stripped.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&stripped[0..2], 16).ok()?;
    let g = u8::from_str_radix(&stripped[2..4], 16).ok()?;
    let b = u8::from_str_radix(&stripped[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b))
}

pub fn build_code_lens_metrics(editor: &Editor) -> Vec<CodeLensMetric> {
    let line_count = editor.line_count();
    if line_count == 0 {
        return Vec::new();
    }

    let mut symbol_lines = Vec::new();
    for line_idx in 0..line_count {
        let trimmed = editor.line_text(line_idx).trim().to_string();
        if looks_like_symbol_header(&trimmed) {
            symbol_lines.push(line_idx);
        }
    }

    if symbol_lines.is_empty() {
        return Vec::new();
    }

    let mut metrics = Vec::with_capacity(symbol_lines.len());
    for (idx, start_line) in symbol_lines.iter().enumerate() {
        let end_line = symbol_lines
            .get(idx + 1)
            .copied()
            .unwrap_or(line_count)
            .max(*start_line + 1);
        let span = end_line - *start_line;
        let mut non_empty = 0usize;
        let mut todo_count = 0usize;

        for line in *start_line..end_line {
            let text = editor.line_text(line);
            if !text.trim().is_empty() {
                non_empty += 1;
            }
            if text.contains("TODO") || text.contains("FIXME") {
                todo_count += 1;
            }
        }

        let mut label = format!("{span} lines • {non_empty} non-empty");
        if todo_count > 0 {
            label.push_str(&format!(" • {todo_count} todo"));
        }
        metrics.push(CodeLensMetric {
            line: *start_line,
            label,
        });
    }

    metrics
}

pub(crate) fn looks_like_symbol_header(line: &str) -> bool {
    if line.is_empty() {
        return false;
    }

    let prefixes = [
        "fn ",
        "pub fn ",
        "async fn ",
        "pub async fn ",
        "def ",
        "class ",
        "function ",
        "struct ",
        "enum ",
        "impl ",
    ];
    prefixes.iter().any(|prefix| line.starts_with(prefix))
}
