use eframe::egui::{self, Sense};

#[derive(Clone, Debug)]
pub struct Command {
    pub name: String,
    pub shortcut: String,
    pub id: CommandId,
}

#[derive(Clone, Debug, PartialEq)]
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
}

pub struct CommandPalette {
    pub visible: bool,
    pub input: String,
    pub selected: usize,
    commands: Vec<Command>,
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
                },
                Command {
                    name: "Open File".into(),
                    shortcut: "Ctrl+O".into(),
                    id: CommandId::OpenFile,
                },
                Command {
                    name: "Save File".into(),
                    shortcut: "Ctrl+S".into(),
                    id: CommandId::SaveFile,
                },
                Command {
                    name: "Save File As...".into(),
                    shortcut: "Ctrl+Shift+S".into(),
                    id: CommandId::SaveFileAs,
                },
                Command {
                    name: "Close Tab".into(),
                    shortcut: "Ctrl+W".into(),
                    id: CommandId::CloseTab,
                },
                Command {
                    name: "Find".into(),
                    shortcut: "Ctrl+F".into(),
                    id: CommandId::Find,
                },
                Command {
                    name: "Go to Line".into(),
                    shortcut: "Ctrl+G".into(),
                    id: CommandId::GoToLine,
                },
                Command {
                    name: "Select All".into(),
                    shortcut: "Ctrl+A".into(),
                    id: CommandId::SelectAll,
                },
            ],
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
                ui.painter().rect_filled(
                    screen,
                    0.0,
                    egui::Color32::from_black_alpha(100),
                );
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
                                .hint_text("Type a command..."),
                        );
                        input_response.request_focus();

                        ui.add_space(4.0);

                        // Collect filtered commands as owned data to avoid borrow conflicts
                        let query = self.input.to_lowercase();
                        let filtered: Vec<Command> = self
                            .commands
                            .iter()
                            .filter(|c| query.is_empty() || c.name.to_lowercase().contains(&query))
                            .cloned()
                            .collect();
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
                            if let Some(cmd) = filtered.get(self.selected) {
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
                                for (i, cmd) in filtered.iter().enumerate() {
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
                                                    egui::RichText::new(&cmd.name)
                                                        .color(egui::Color32::WHITE)
                                                        .size(13.0),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        ui.label(
                                                            egui::RichText::new(&cmd.shortcut)
                                                                .color(egui::Color32::from_rgb(120, 120, 120))
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
