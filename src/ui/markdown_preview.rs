use eframe::egui;

#[derive(Debug)]
enum MdBlock {
    Heading { level: usize, text: String },
    Paragraph(String),
    ListItem(String),
    Quote(String),
    Code(String),
    Hr,
}

pub fn show(ui: &mut egui::Ui, markdown: &str) {
    let blocks = parse_markdown(markdown);
    let bg = egui::Color32::from_rgb(18, 18, 27);
    let border = egui::Color32::from_rgb(40, 40, 58);

    egui::Frame::none()
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, border))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for block in blocks {
                    render_block(ui, block);
                }
            });
        });
}

fn parse_markdown(input: &str) -> Vec<MdBlock> {
    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    let mut in_code = false;
    let mut code_buf = String::new();

    let flush_paragraph = |blocks: &mut Vec<MdBlock>, paragraph: &mut String| {
        if !paragraph.trim().is_empty() {
            blocks.push(MdBlock::Paragraph(paragraph.trim().to_string()));
        }
        paragraph.clear();
    };

    for line in input.lines() {
        let trimmed = line.trim_end();
        if in_code {
            if trimmed.starts_with("```") {
                blocks.push(MdBlock::Code(code_buf.trim_end().to_string()));
                code_buf.clear();
                in_code = false;
            } else {
                code_buf.push_str(trimmed);
                code_buf.push('\n');
            }
            continue;
        }

        if trimmed.starts_with("```") {
            flush_paragraph(&mut blocks, &mut paragraph);
            in_code = true;
            continue;
        }

        if trimmed.is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph);
            continue;
        }

        if is_horizontal_rule(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            blocks.push(MdBlock::Hr);
            continue;
        }

        if let Some((level, text)) = parse_heading(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            blocks.push(MdBlock::Heading { level, text });
            continue;
        }

        if let Some(item) = parse_list_item(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            blocks.push(MdBlock::ListItem(item));
            continue;
        }

        if let Some(quote) = trimmed.strip_prefix("> ") {
            flush_paragraph(&mut blocks, &mut paragraph);
            blocks.push(MdBlock::Quote(quote.trim().to_string()));
            continue;
        }

        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }

    if in_code && !code_buf.is_empty() {
        blocks.push(MdBlock::Code(code_buf.trim_end().to_string()));
    }
    if !paragraph.is_empty() {
        blocks.push(MdBlock::Paragraph(paragraph.trim().to_string()));
    }

    blocks
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let text = trimmed[hashes..].trim();
    if text.is_empty() {
        return None;
    }
    Some((hashes, text.to_string()))
}

fn parse_list_item(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some(rest.trim().to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return Some(rest.trim().to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("+ ") {
        return Some(rest.trim().to_string());
    }
    None
}

fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let all_dashes = trimmed.chars().all(|c| c == '-');
    let all_stars = trimmed.chars().all(|c| c == '*');
    all_dashes || all_stars
}

fn render_block(ui: &mut egui::Ui, block: MdBlock) {
    match block {
        MdBlock::Heading { level, text } => {
            let size = match level {
                1 => 24.0,
                2 => 20.0,
                3 => 18.0,
                4 => 16.0,
                5 => 15.0,
                _ => 14.0,
            };
            ui.label(
                egui::RichText::new(text)
                    .size(size)
                    .color(egui::Color32::from_rgb(235, 235, 235))
                    .strong(),
            );
            ui.add_space(6.0);
        }
        MdBlock::Paragraph(text) => {
            ui.label(
                egui::RichText::new(text)
                    .size(14.0)
                    .color(egui::Color32::from_rgb(210, 210, 210)),
            );
            ui.add_space(6.0);
        }
        MdBlock::ListItem(text) => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("-")
                        .size(14.0)
                        .color(egui::Color32::from_rgb(210, 210, 210)),
                );
                ui.label(
                    egui::RichText::new(text)
                        .size(14.0)
                        .color(egui::Color32::from_rgb(210, 210, 210)),
                );
            });
            ui.add_space(4.0);
        }
        MdBlock::Quote(text) => {
            let bg = egui::Color32::from_rgb(38, 38, 42);
            let stroke = egui::Color32::from_rgb(75, 75, 80);
            egui::Frame::none()
                .fill(bg)
                .stroke(egui::Stroke::new(1.0, stroke))
                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(text)
                            .size(14.0)
                            .color(egui::Color32::from_rgb(215, 215, 215)),
                    );
                });
            ui.add_space(6.0);
        }
        MdBlock::Code(code) => {
            let bg = egui::Color32::from_rgb(25, 25, 28);
            let stroke = egui::Color32::from_rgb(60, 60, 66);
            egui::Frame::none()
                .fill(bg)
                .stroke(egui::Stroke::new(1.0, stroke))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(code)
                            .monospace()
                            .size(13.0)
                            .color(egui::Color32::from_rgb(200, 200, 200)),
                    );
                });
            ui.add_space(6.0);
        }
        MdBlock::Hr => {
            ui.add(egui::Separator::default());
            ui.add_space(6.0);
        }
    }
}
