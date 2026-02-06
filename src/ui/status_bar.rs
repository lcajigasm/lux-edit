use eframe::egui;

use crate::editor::{Editor, IndentStyle, LineEnding, TextEncoding};
use crate::syntax::SyntaxHighlighter;

const BAR_HEIGHT: f32 = 22.0;
const BAR_ITEM_HOVER_ALPHA: u8 = 30;

const BAR_BG: egui::Color32 = egui::Color32::from_rgb(0, 122, 204); // VS Code Blue for status bar
const BAR_TEXT: egui::Color32 = egui::Color32::WHITE;

#[derive(Clone, Debug, Default)]
pub struct GitInfo {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: bool,
}

pub fn show(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    git_info: Option<&GitInfo>,
    highlighter: &SyntaxHighlighter,
) {
    let rect = ui.available_rect_before_wrap();
    let bar_rect = egui::Rect::from_min_size(
        egui::Pos2::new(rect.left(), rect.bottom() - BAR_HEIGHT),
        egui::Vec2::new(rect.width(), BAR_HEIGHT),
    );

    ui.painter().rect_filled(bar_rect, 0.0, BAR_BG);

    // Allocate the area
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    ui.spacing_mut().item_spacing = egui::Vec2::new(0.0, 0.0);

    // Left Side (Git)
    if let Some(info) = git_info {
        let mut status = info.branch.clone();
        if info.dirty {
            status.push('*');
        }
        if info.ahead > 0 {
            status.push_str(&format!(" ↑{}", info.ahead));
        }
        if info.behind > 0 {
            status.push_str(&format!(" ↓{}", info.behind));
        }

        status_item(&mut ui, &format!("\u{E0A0} {}", status)); //  branch symbol (requires nerd font, fallback text)
                                                               // Note: \u{E0A0} is Nerd Font git branch.
                                                               // If not available, we can use "git: "
    }
    let mut added = 0usize;
    let mut modified = 0usize;
    let mut removed = 0usize;
    for h in &editor.diff_hunks {
        let span = h.end.saturating_sub(h.start).max(1);
        match h.kind {
            crate::editor::DiffKind::Added => added += span,
            crate::editor::DiffKind::Modified => modified += span,
            crate::editor::DiffKind::Removed => removed += span,
        }
    }
    status_item(
        &mut ui,
        &format!("SCM +{} ~{} -{}", added, modified, removed),
    );
    let diag_errors = editor
        .diagnostics
        .iter()
        .filter(|d| d.severity <= 2)
        .count();
    let diag_warnings = editor
        .diagnostics
        .iter()
        .filter(|d| d.severity == 3)
        .count();
    let diag_text = format!("Diag E:{} W:{}", diag_errors, diag_warnings);
    status_item(&mut ui, &diag_text);
    status_item(&mut ui, &format!("Tasks {}", editor.background_tasks));
    status_item(&mut ui, &format!("Badge {}", editor.notification_badges));
    status_item(&mut ui, &editor.lsp_status);

    // Spacer
    ui.label("");

    // Right Side
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // Cursor Position
        let primary = &editor.cursors[0];
        let cursor_text = if editor.cursors.len() > 1 {
            format!(
                "Ln {}, Col {} ({} cursors)",
                primary.pos.line + 1,
                primary.pos.col + 1,
                editor.cursors.len()
            )
        } else {
            format!("Ln {}, Col {}", primary.pos.line + 1, primary.pos.col + 1)
        };
        let _ = status_item(ui, &cursor_text);

        // Encoding
        let encoding_label = match editor.encoding {
            TextEncoding::Utf8 => "UTF-8",
            TextEncoding::Utf8Bom => "UTF-8 BOM",
        };
        let encoding_resp = status_item(ui, encoding_label);
        if encoding_resp.clicked() {
            editor.encoding = match editor.encoding {
                TextEncoding::Utf8 => TextEncoding::Utf8Bom,
                TextEncoding::Utf8Bom => TextEncoding::Utf8,
            };
        }
        encoding_resp.context_menu(|ui| {
            let mut set = None;
            if ui
                .selectable_label(editor.encoding == TextEncoding::Utf8, "UTF-8")
                .clicked()
            {
                set = Some(TextEncoding::Utf8);
            }
            if ui
                .selectable_label(editor.encoding == TextEncoding::Utf8Bom, "UTF-8 BOM")
                .clicked()
            {
                set = Some(TextEncoding::Utf8Bom);
            }
            if let Some(val) = set {
                editor.encoding = val;
                ui.close_menu();
            }
        });

        // Line endings
        let eol_label = match editor.line_ending {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
        };
        let eol_resp = status_item(ui, eol_label);
        if eol_resp.clicked() {
            editor.line_ending = match editor.line_ending {
                LineEnding::Lf => LineEnding::CrLf,
                LineEnding::CrLf => LineEnding::Lf,
            };
        }
        eol_resp.context_menu(|ui| {
            let mut set = None;
            if ui
                .selectable_label(editor.line_ending == LineEnding::Lf, "LF")
                .clicked()
            {
                set = Some(LineEnding::Lf);
            }
            if ui
                .selectable_label(editor.line_ending == LineEnding::CrLf, "CRLF")
                .clicked()
            {
                set = Some(LineEnding::CrLf);
            }
            if let Some(val) = set {
                editor.line_ending = val;
                ui.close_menu();
            }
        });

        // Indentation
        let indent_label = match editor.indent_style {
            IndentStyle::Spaces => format!("Spaces: {}", editor.indent_width.max(1)),
            IndentStyle::Tabs => format!("Tabs: {}", editor.indent_width.max(1)),
        };
        let indent_resp = status_item(ui, &indent_label);
        if indent_resp.clicked() {
            editor.indent_style = match editor.indent_style {
                IndentStyle::Spaces => IndentStyle::Tabs,
                IndentStyle::Tabs => IndentStyle::Spaces,
            };
        }
        indent_resp.context_menu(|ui| {
            let mut new_style = None;
            if ui
                .selectable_label(editor.indent_style == IndentStyle::Spaces, "Spaces")
                .clicked()
            {
                new_style = Some(IndentStyle::Spaces);
            }
            if ui
                .selectable_label(editor.indent_style == IndentStyle::Tabs, "Tabs")
                .clicked()
            {
                new_style = Some(IndentStyle::Tabs);
            }
            if let Some(style) = new_style {
                editor.indent_style = style;
                if editor.indent_width == 0 {
                    editor.indent_width = 4;
                }
                ui.close_menu();
            }
            ui.separator();
            ui.label("Indent width");
            let mut set_width = None;
            for width in [2usize, 4, 8] {
                let label = format!("{}", width);
                if ui
                    .selectable_label(editor.indent_width == width, label)
                    .clicked()
                {
                    set_width = Some(width);
                }
            }
            if let Some(width) = set_width {
                editor.indent_width = width;
                ui.close_menu();
            }
        });

        // Markdown preview toggle
        if editor.is_markdown() {
            let label = if editor.markdown_preview {
                "Preview: On"
            } else {
                "Preview: Off"
            };
            let preview_resp = status_item(ui, label);
            if preview_resp.clicked() {
                editor.markdown_preview = !editor.markdown_preview;
            }
        }

        // Language
        let first_line = editor.first_line_text();
        let syntax_label = highlighter.syntax_name_for(
            editor.file_path.as_deref(),
            first_line.as_deref(),
            editor.syntax_override.as_deref(),
        );
        let lang_resp = status_item(ui, &syntax_label);
        let popup_id = ui.make_persistent_id("syntax_picker_popup");
        if lang_resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
        }
        egui::popup::popup_below_widget(
            ui,
            popup_id,
            &lang_resp,
            egui::popup::PopupCloseBehavior::CloseOnClickOutside,
            |ui: &mut egui::Ui| {
                ui.set_min_width(220.0);
                if ui
                    .selectable_label(editor.syntax_override.is_none(), "Auto (Detect)")
                    .clicked()
                {
                    editor.syntax_override = None;
                    ui.close_menu();
                }
                ui.separator();
                let mut syntaxes = highlighter.available_syntaxes();
                if !syntaxes
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case("Plain Text"))
                {
                    syntaxes.insert(0, "Plain Text".into());
                }
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for name in syntaxes {
                            let selected = editor
                                .syntax_override
                                .as_deref()
                                .map(|v| v == name.as_str())
                                .unwrap_or(false);
                            if ui.selectable_label(selected, &name).clicked() {
                                editor.syntax_override = Some(name.clone());
                                ui.close_menu();
                            }
                        }
                    });
            },
        );
    });
}

fn status_item(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let font = egui::FontId::proportional(12.0);
    let text_color = BAR_TEXT;

    // Calculate size
    let padding = egui::vec2(10.0, 0.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), text_color);
    let (rect, resp) = ui.allocate_exact_size(
        egui::Vec2::new(galley.rect.width() + padding.x * 2.0, BAR_HEIGHT),
        egui::Sense::click(),
    );

    if resp.hovered() {
        ui.painter().rect_filled(
            rect,
            0.0,
            egui::Color32::from_white_alpha(BAR_ITEM_HOVER_ALPHA),
        );
    }

    ui.painter().galley(
        egui::Pos2::new(
            rect.left() + padding.x,
            rect.center().y - galley.rect.height() / 2.0,
        ),
        galley,
        text_color,
    );

    resp
}
