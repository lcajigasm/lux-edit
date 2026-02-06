use eframe::egui::{self, Sense};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Command {
    pub name: String,
    pub shortcut: String,
    pub id: CommandId,
    pub category: CommandCategory,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    NewTab,
    OpenFile,
    SaveFile,
    SaveFileAs,
    CloseTab,
    Find,
    GoToLine,
    SelectAll,
    Undo,
    Redo,
    FormatDocument,
    ToggleGitPanel,
    RefreshGitPanel,
    StartDebugSession,
    RunTask,
    RunCustomScript,
    Extension(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCategory {
    File,
    Edit,
    Selection,
    Navigation,
    Find,
    Extensions,
}

impl CommandCategory {
    fn label(self) -> &'static str {
        match self {
            CommandCategory::File => "File",
            CommandCategory::Edit => "Edit",
            CommandCategory::Selection => "Selection",
            CommandCategory::Navigation => "Navigation",
            CommandCategory::Find => "Find",
            CommandCategory::Extensions => "Extensions",
        }
    }

    fn priority(self) -> i32 {
        match self {
            CommandCategory::File => 40,
            CommandCategory::Edit => 30,
            CommandCategory::Selection => 22,
            CommandCategory::Navigation => 24,
            CommandCategory::Find => 26,
            CommandCategory::Extensions => 18,
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        match self {
            CommandCategory::File => &["file", "fs", "tab"],
            CommandCategory::Edit => &["edit", "change"],
            CommandCategory::Selection => &["select", "selection"],
            CommandCategory::Navigation => &["nav", "goto", "jump"],
            CommandCategory::Find => &["find", "search", "replace"],
            CommandCategory::Extensions => &["extension", "plugin", "custom"],
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct UsageStat {
    count: u32,
    last_used: u64,
}

pub struct CommandPalette {
    pub visible: bool,
    pub input: String,
    pub selected: usize,
    commands: Vec<Command>,
    usage: HashMap<CommandId, UsageStat>,
    use_counter: u64,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            visible: false,
            input: String::new(),
            selected: 0,
            commands: vec![
                Command {
                    name: "New Tab".into(),
                    shortcut: "Ctrl+N".into(),
                    id: CommandId::NewTab,
                    category: CommandCategory::File,
                },
                Command {
                    name: "Open File".into(),
                    shortcut: "Ctrl+O".into(),
                    id: CommandId::OpenFile,
                    category: CommandCategory::File,
                },
                Command {
                    name: "Save File".into(),
                    shortcut: "Ctrl+S".into(),
                    id: CommandId::SaveFile,
                    category: CommandCategory::File,
                },
                Command {
                    name: "Save File As...".into(),
                    shortcut: "Ctrl+Shift+S".into(),
                    id: CommandId::SaveFileAs,
                    category: CommandCategory::File,
                },
                Command {
                    name: "Close Tab".into(),
                    shortcut: "Ctrl+W".into(),
                    id: CommandId::CloseTab,
                    category: CommandCategory::File,
                },
                Command {
                    name: "Find".into(),
                    shortcut: "Ctrl+F".into(),
                    id: CommandId::Find,
                    category: CommandCategory::Find,
                },
                Command {
                    name: "Go to Line".into(),
                    shortcut: "Ctrl+G".into(),
                    id: CommandId::GoToLine,
                    category: CommandCategory::Navigation,
                },
                Command {
                    name: "Select All".into(),
                    shortcut: "Ctrl+A".into(),
                    id: CommandId::SelectAll,
                    category: CommandCategory::Selection,
                },
                Command {
                    name: "Undo".into(),
                    shortcut: "Ctrl+Z".into(),
                    id: CommandId::Undo,
                    category: CommandCategory::Edit,
                },
                Command {
                    name: "Redo".into(),
                    shortcut: "Ctrl+Shift+Z".into(),
                    id: CommandId::Redo,
                    category: CommandCategory::Edit,
                },
                Command {
                    name: "Format Document".into(),
                    shortcut: "Ctrl+Shift+F".into(),
                    id: CommandId::FormatDocument,
                    category: CommandCategory::Edit,
                },
                Command {
                    name: "Git: Toggle Panel".into(),
                    shortcut: "".into(),
                    id: CommandId::ToggleGitPanel,
                    category: CommandCategory::Extensions,
                },
                Command {
                    name: "Git: Refresh Panel".into(),
                    shortcut: "".into(),
                    id: CommandId::RefreshGitPanel,
                    category: CommandCategory::Extensions,
                },
                Command {
                    name: "Debug: Start Session".into(),
                    shortcut: "".into(),
                    id: CommandId::StartDebugSession,
                    category: CommandCategory::Extensions,
                },
                Command {
                    name: "Task: Run Workspace Task".into(),
                    shortcut: "".into(),
                    id: CommandId::RunTask,
                    category: CommandCategory::Extensions,
                },
                Command {
                    name: "Script: Run Custom Script".into(),
                    shortcut: "".into(),
                    id: CommandId::RunCustomScript,
                    category: CommandCategory::Extensions,
                },
            ],
            usage: HashMap::new(),
            use_counter: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.input.clear();
            self.selected = 0;
        }
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.input.clear();
    }

    pub fn register_extension_commands<'a>(
        &mut self,
        provider: &str,
        commands: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
    ) {
        let provider_prefix = format!("{provider}:");
        self.commands.retain(|cmd| {
            !matches!(
                cmd.id,
                CommandId::Extension(ref id) if id.starts_with(&provider_prefix)
            )
        });
        for (name, shortcut, command_id) in commands {
            self.commands.push(Command {
                name: format!("{provider}: {name}"),
                shortcut: shortcut.to_string(),
                id: CommandId::Extension(format!("{provider}:{command_id}")),
                category: CommandCategory::Extensions,
            });
        }
    }

    pub fn register_use(&mut self, id: CommandId) {
        self.use_counter = self.use_counter.saturating_add(1);
        let entry = self.usage.entry(id).or_default();
        entry.count = entry.count.saturating_add(1);
        entry.last_used = self.use_counter;
    }

    /// Show the command palette overlay. Returns the selected CommandId if one was chosen.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<CommandId> {
        if !self.visible {
            return None;
        }

        let mut result = None;
        let mut should_close = false;

        egui::Area::new(egui::Id::new("command_palette_bg"))
            .fixed_pos(egui::Pos2::ZERO)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(100));
            });

        let screen = ctx.screen_rect();
        let palette_width = 500.0_f32.min(screen.width() - 40.0);
        let x = (screen.width() - palette_width) / 2.0;

        egui::Area::new(egui::Id::new("command_palette"))
            .fixed_pos(egui::Pos2::new(x, 80.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(40, 40, 40))
                    .rounding(egui::Rounding::same(8.0))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 70, 70)))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.set_width(palette_width);

                        let input_response = ui.add(
                            egui::TextEdit::singleline(&mut self.input)
                                .desired_width(palette_width - 16.0)
                                .font(egui::FontId::monospace(14.0))
                                .text_color(egui::Color32::WHITE)
                                .hint_text("Type a command... (e.g. file: open)"),
                        );
                        input_response.request_focus();

                        ui.add_space(4.0);

                        // Collect filtered commands as owned data to avoid borrow conflicts
                        let raw_query = self.input.trim();
                        let (category_filter, query) = split_query(raw_query);
                        let query = query.to_lowercase();
                        let mut filtered: Vec<(Command, i32)> = self
                            .commands
                            .iter()
                            .filter(|c| {
                                category_filter
                                    .as_deref()
                                    .map(|filter| category_matches(filter, c.category))
                                    .unwrap_or(true)
                            })
                            .filter_map(|c| {
                                let score = command_score(
                                    c,
                                    &query,
                                    self.use_counter,
                                    self.usage.get(&c.id),
                                )?;
                                Some((c.clone(), score))
                            })
                            .collect();
                        filtered
                            .sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
                        let count = filtered.len();

                        // Keyboard navigation
                        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                            should_close = true;
                            return;
                        }
                        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) && count > 0 {
                            self.selected = (self.selected + 1) % count;
                        }
                        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) && count > 0 {
                            self.selected = self.selected.checked_sub(1).unwrap_or(count - 1);
                        }
                        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Some((cmd, _)) = filtered.get(self.selected) {
                                result = Some(cmd.id.clone());
                                should_close = true;
                                return;
                            }
                        }

                        if self.selected >= count && count > 0 {
                            self.selected = count - 1;
                        }

                        // Command list
                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .show(ui, |ui| {
                                if filtered.is_empty() {
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new("No matches")
                                            .color(egui::Color32::from_rgb(140, 140, 140))
                                            .size(12.0),
                                    );
                                }

                                for (i, (cmd, _score)) in filtered.iter().enumerate() {
                                    let is_selected = i == self.selected;
                                    let bg = if is_selected {
                                        egui::Color32::from_rgb(55, 55, 75)
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };

                                    let resp = egui::Frame::none()
                                        .fill(bg)
                                        .rounding(egui::Rounding::same(4.0))
                                        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(cmd.category.label())
                                                        .color(egui::Color32::from_rgb(
                                                            120, 120, 120,
                                                        ))
                                                        .size(10.0),
                                                );
                                                ui.add_space(6.0);
                                                ui.label(
                                                    egui::RichText::new(&cmd.name)
                                                        .color(egui::Color32::WHITE)
                                                        .size(13.0),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            egui::RichText::new(&cmd.shortcut)
                                                                .color(egui::Color32::from_rgb(
                                                                    120, 120, 120,
                                                                ))
                                                                .size(11.0),
                                                        );
                                                    },
                                                );
                                            });
                                        })
                                        .response;

                                    if resp.interact(Sense::click()).clicked() {
                                        result = Some(cmd.id.clone());
                                        should_close = true;
                                    }
                                }
                            });
                    });
            });

        if should_close {
            self.close();
        }

        result
    }
}

fn split_query(raw: &str) -> (Option<String>, String) {
    if let Some((prefix, rest)) = raw.split_once(':') {
        let prefix = prefix.trim();
        let rest = rest.trim();
        if !prefix.is_empty() {
            return (Some(prefix.to_lowercase()), rest.to_string());
        }
    }
    (None, raw.to_string())
}

fn category_matches(filter: &str, category: CommandCategory) -> bool {
    let filter = filter.trim().to_lowercase();
    if filter.is_empty() {
        return true;
    }
    if category.label().to_lowercase().starts_with(&filter) {
        return true;
    }
    category
        .aliases()
        .iter()
        .any(|alias| alias.starts_with(&filter))
}

fn command_score(
    cmd: &Command,
    query: &str,
    current_use: u64,
    usage: Option<&UsageStat>,
) -> Option<i32> {
    let mut score = 0;
    let query_empty = query.trim().is_empty();

    if query_empty {
        score += 10;
    } else {
        let mut best = fuzzy_score(&cmd.name, query);
        if best.is_none() {
            best = fuzzy_score(cmd.category.label(), query);
        }
        if best.is_none() && !cmd.shortcut.is_empty() {
            best = fuzzy_score(&cmd.shortcut, query).map(|s| s.saturating_sub(10));
        }
        score += best?;
    }

    score += cmd.category.priority();
    score += history_bonus(current_use, usage);

    Some(score)
}

fn history_bonus(current_use: u64, usage: Option<&UsageStat>) -> i32 {
    let Some(usage) = usage else {
        return 0;
    };
    let recency = current_use.saturating_sub(usage.last_used) as i32;
    let recency_bonus = (12 - recency).clamp(0, 12);
    (usage.count as i32 * 4) + recency_bonus
}

fn fuzzy_score(text: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let text_chars: Vec<char> = text.chars().collect();
    let text_lower: Vec<char> = text.to_lowercase().chars().collect();
    let query_chars: Vec<char> = query.to_lowercase().chars().collect();

    let mut score = 0i32;
    let mut qi = 0usize;
    let mut last_match: Option<usize> = None;

    for (i, ch) in text_lower.iter().enumerate() {
        if qi >= query_chars.len() {
            break;
        }
        if *ch == query_chars[qi] {
            score += 12;
            if let Some(prev) = last_match {
                if i == prev + 1 {
                    score += 6;
                }
            }
            if is_word_start(&text_chars, i) {
                score += 8;
            }
            last_match = Some(i);
            qi += 1;
        }
    }

    if qi == query_chars.len() {
        score += (query_chars.len() as i32) * 2;
        Some(score)
    } else {
        None
    }
}

fn is_word_start(text: &[char], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    let prev = text[idx - 1];
    let current = text[idx];
    if !prev.is_alphanumeric() {
        return true;
    }
    prev.is_lowercase() && current.is_uppercase()
}
