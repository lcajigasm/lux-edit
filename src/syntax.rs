use eframe::egui::Color32;
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

#[derive(Clone)]
pub struct StyledToken {
    pub text: String,
    pub color: Color32,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    fn find_syntax(
        &self,
        file_path: Option<&Path>,
        first_line: Option<&str>,
        override_name: Option<&str>,
    ) -> &SyntaxReference {
        if let Some(name) = override_name {
            if name.eq_ignore_ascii_case("plain text") {
                return self.syntax_set.find_syntax_plain_text();
            }
            if let Some(syn) = self.syntax_set.find_syntax_by_name(name) {
                return syn;
            }
        }
        if let Some(path) = file_path {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_ascii_lowercase();
                if let Some(syn) = self.syntax_set.find_syntax_by_extension(&ext) {
                    return syn;
                }
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(syn) = self.syntax_set.find_syntax_by_name(name) {
                    return syn;
                }
            }
        }
        if let Some(line) = first_line {
            if let Some(syn) = self.syntax_set.find_syntax_by_first_line(line) {
                return syn;
            }
        }
        self.syntax_set.find_syntax_plain_text()
    }

    pub fn available_syntaxes(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .syntax_set
            .syntaxes()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    pub fn syntax_name_for(
        &self,
        file_path: Option<&Path>,
        first_line: Option<&str>,
        override_name: Option<&str>,
    ) -> String {
        let syn = self.find_syntax(file_path, first_line, override_name);
        syn.name.clone()
    }

    /// Highlight a range of lines. Returns a Vec of line token lists.
    pub fn highlight_lines(
        &self,
        full_text: &str,
        file_path: Option<&Path>,
        override_name: Option<&str>,
        first_line: usize,
        last_line: usize,
    ) -> Vec<Vec<StyledToken>> {
        let syntax = self.find_syntax(file_path, full_text.lines().next(), override_name);
        let theme = &self.theme_set.themes["base16-eighties.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut result = Vec::new();
        for (i, line) in LinesWithEndings::from(full_text).enumerate() {
            let regions = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            if i >= first_line && i < last_line {
                let tokens: Vec<StyledToken> = regions
                    .iter()
                    .map(|(style, text)| StyledToken {
                        text: text
                            .trim_end_matches('\n')
                            .trim_end_matches('\r')
                            .to_string(),
                        color: syntect_to_egui(*style),
                    })
                    .filter(|t| !t.text.is_empty())
                    .collect();
                result.push(tokens);
            }
            if i >= last_line {
                break;
            }
        }

        result
    }
}

fn syntect_to_egui(style: Style) -> Color32 {
    Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b)
}
