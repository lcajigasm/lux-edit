use eframe::egui;
use super::*;

impl LuxApp {
    pub(super) fn show_activity_bar(&mut self, ui: &mut egui::Ui) {
        egui::SidePanel::left("activity_bar")
            .resizable(false)
            .exact_width(48.0)
            .frame(
                egui::Frame::none()
                    .fill(ACTIVITY_BAR_BG)
                    .inner_margin(egui::Margin::symmetric(4.0, 8.0)),
            )
            .show_inside(ui, |ui| {
                let tabs = [
                    (SidebarTab::Explorer, "E"),
                    (SidebarTab::Search, "S"),
                    (SidebarTab::Git, "G"),
                    (SidebarTab::Debug, "D"),
                    (SidebarTab::Collab, "C"),
                ];
                for (tab, icon) in tabs {
                    let selected = self.show_sidebar && self.sidebar_tab == tab;
                    let label = egui::RichText::new(icon).monospace().size(14.0).strong();
                    let button = egui::SelectableLabel::new(selected, label);
                    let response = ui.add_sized([36.0, 34.0], button).on_hover_text(format!(
                        "{} ({})",
                        Self::sidebar_tab_title(tab),
                        Self::sidebar_tab_hint(tab)
                    ));
                    if response.clicked() {
                        if self.show_sidebar && self.sidebar_tab == tab {
                            self.show_sidebar = false;
                        } else {
                            self.show_sidebar = true;
                            self.sidebar_tab = tab;
                        }
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if ui
                        .add_sized([36.0, 30.0], egui::Button::new("..."))
                        .on_hover_text("Manage")
                        .clicked()
                    {
                        self.show_help_window = true;
                    }
                });
            });
    }

    pub(super) fn show_left_sidebar(&mut self, ui: &mut egui::Ui) {
        if !self.show_sidebar {
            return;
        }
        egui::SidePanel::left("left_sidebar")
            .resizable(true)
            .default_width(290.0)
            .frame(
                egui::Frame::none()
                    .fill(SIDEBAR_BG)
                    .inner_margin(egui::Margin::symmetric(8.0, 8.0)),
            )
            .show_inside(ui, |ui| {
                ui.label(
                    egui::RichText::new(Self::sidebar_tab_title(self.sidebar_tab))
                        .strong()
                        .size(13.0)
                        .color(egui::Color32::from_rgb(220, 220, 220)),
                );
                ui.separator();

                match self.sidebar_tab {
                    SidebarTab::Explorer => {
                        ui.label("Workspace Roots");
                        for root in self.workspace_roots.clone() {
                            ui.horizontal(|ui| {
                                let selected = root == self.workspace_root;
                                if ui
                                    .selectable_label(selected, root.to_string_lossy().to_string())
                                    .clicked()
                                {
                                    self.workspace_root = root.clone();
                                }
                                let trusted = self.trusted_workspaces.contains(&root);
                                if ui
                                    .small_button(if trusted { "Trusted" } else { "Untrusted" })
                                    .clicked()
                                {
                                    if trusted {
                                        self.trusted_workspaces.remove(&root);
                                    } else {
                                        self.trusted_workspaces.insert(root.clone());
                                    }
                                }
                            });
                        }
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_workspace_root_input)
                                    .hint_text("add workspace root path"),
                            );
                            if ui.button("Add Root").clicked() {
                                let path =
                                    PathBuf::from(self.new_workspace_root_input.trim().to_string());
                                if !path.as_os_str().is_empty()
                                    && !self.workspace_roots.contains(&path)
                                {
                                    self.workspace_roots.push(path);
                                    self.new_workspace_root_input.clear();
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Search ignore pattern");
                            if ui.button("+ target/**").clicked()
                                && !self
                                    .project_gitignore_patterns
                                    .contains(&"target/**".to_string())
                            {
                                self.project_gitignore_patterns
                                    .push("target/**".to_string());
                            }
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut self.quick_open_query)
                                .hint_text("Quick open (filter files)"),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.file_ops_target)
                                .hint_text("Target path (relative to workspace)"),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("New File").clicked() {
                                self.create_file_or_folder(false);
                            }
                            if ui.button("New Folder").clicked() {
                                self.create_file_or_folder(true);
                            }
                            if ui.button("Duplicate").clicked() {
                                self.duplicate_active_file();
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Rename/Move").clicked() {
                                self.rename_or_move_active_file();
                            }
                            if ui.button("Delete Active").clicked() {
                                self.request_delete_active_file();
                            }
                            if self.deleted_file_backup.is_some()
                                && ui.button("Undo Delete").clicked()
                            {
                                self.undo_last_deleted_file();
                            }
                        });
                        if !self.file_ops_message.is_empty() {
                            ui.label(self.file_ops_message.clone());
                        }
                        let filter = self.quick_open_query.trim().to_lowercase();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for file in self.sidebar_files.clone() {
                                let label = file.to_string_lossy().to_string();
                                if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                                    continue;
                                }
                                let icon = file_icon(file.as_path());
                                if ui
                                    .selectable_label(false, format!("{} {}", icon, label))
                                    .clicked()
                                {
                                    self.open_path_in_tab(file.as_path());
                                }
                            }
                        });
                    }
                    SidebarTab::Search => {
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.sidebar_search_query)
                                .hint_text("Search in workspace"),
                        );
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.sidebar_search_regex, "Regex");
                            ui.checkbox(&mut self.sidebar_search_case_sensitive, "Case sensitive");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Include");
                            ui.text_edit_singleline(&mut self.sidebar_search_include_glob);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Exclude");
                            ui.text_edit_singleline(&mut self.sidebar_search_exclude_glob);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Replace");
                            ui.text_edit_singleline(&mut self.sidebar_replace_input);
                            if ui.button("Replace All").clicked() {
                                self.replace_all_search_results();
                            }
                        });
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.run_sidebar_search();
                        }
                        if ui.button("Run Search").clicked() {
                            self.run_sidebar_search();
                        }
                        if !self.sidebar_search_message.is_empty() {
                            ui.label(self.sidebar_search_message.clone());
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for hit in self.sidebar_search_results.clone() {
                                let selected = self
                                    .sidebar_search_selected
                                    .as_deref()
                                    .map(|s| s == hit.as_str())
                                    .unwrap_or(false);
                                if ui.selectable_label(selected, &hit).clicked() {
                                    self.sidebar_search_selected = Some(hit.clone());
                                    self.update_search_preview(&hit);
                                    if let Some((path, _line)) = parse_search_hit_location(&hit) {
                                        let full_path = self.workspace_join(&path);
                                        self.open_path_in_tab(full_path.as_path());
                                    }
                                }
                            }
                        });
                        if !self.sidebar_search_preview.is_empty() {
                            ui.separator();
                            ui.label("Preview");
                            ui.label(
                                egui::RichText::new(self.sidebar_search_preview.clone())
                                    .monospace()
                                    .size(11.0),
                            );
                        }
                        ui.separator();
                        ui.label("Symbol Search");
                        let symbol_resp = ui.add(
                            egui::TextEdit::singleline(&mut self.sidebar_symbol_query)
                                .hint_text("Search symbols (document/workspace)"),
                        );
                        if symbol_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            self.run_symbol_search();
                        }
                        if ui.button("Run Symbol Search").clicked() {
                            self.run_symbol_search();
                        }
                        egui::ScrollArea::vertical()
                            .max_height(120.0)
                            .show(ui, |ui| {
                                for symbol in self.sidebar_symbol_results.clone() {
                                    if ui.selectable_label(false, &symbol).clicked() {
                                        if let Some((path, line)) = parse_symbol_location(&symbol) {
                                            self.open_path_in_tab(path.as_path());
                                            self.active_editor().goto_line(line);
                                        }
                                    }
                                }
                            });
                        ui.separator();
                        ui.label("LSP Navigation Results");
                        egui::ScrollArea::vertical()
                            .max_height(110.0)
                            .show(ui, |ui| {
                                for nav in self.editors[self.active_tab].lsp_nav_results.clone() {
                                    if ui.selectable_label(false, &nav).clicked() {
                                        let cleaned = nav.trim_start_matches("file://");
                                        let parts: Vec<&str> = cleaned.split(':').collect();
                                        if parts.len() >= 3 {
                                            let line = parts[parts.len() - 2]
                                                .parse::<usize>()
                                                .unwrap_or(1);
                                            let path = parts[..parts.len() - 2].join(":");
                                            self.open_path_in_tab(Path::new(&path));
                                            self.active_editor().goto_line(line);
                                        }
                                    }
                                }
                            });
                        ui.separator();
                        ui.label("Code Actions");
                        let actions = self.editors[self.active_tab].code_actions.clone();
                        for action in actions {
                            if ui.button(&action).clicked() {
                                let ok = self.active_editor().apply_code_action(&action);
                                self.refactor_status = if ok {
                                    format!("Applied: {}", action)
                                } else {
                                    format!("No changes: {}", action)
                                };
                            }
                        }
                    }
                    SidebarTab::Git => {
                        ui.label("Use the right Git panel for staging, history and diff.");
                        if ui.button("Toggle right Git panel").clicked() {
                            self.show_git_panel = !self.show_git_panel;
                        }
                    }
                    SidebarTab::Debug => {
                        ui.heading("Debugger");
                        if ui.button("Toggle Breakpoint @ Cursor").clicked() {
                            self.add_breakpoint_at_cursor();
                        }
                        ui.label("Breakpoints");
                        for (path, lines) in self.debug_breakpoints.clone() {
                            for line in lines {
                                ui.label(format!("{}:{}", path.to_string_lossy(), line));
                            }
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.debug_watch_input)
                                    .hint_text("Watch expression"),
                            );
                            if ui.button("Add Watch").clicked() {
                                let watch = self.debug_watch_input.trim().to_string();
                                if !watch.is_empty() {
                                    self.debug_watches.push(watch);
                                    self.debug_watch_input.clear();
                                }
                            }
                        });
                        for watch in self.debug_watches.clone() {
                            ui.label(format!("watch: {}", watch));
                        }
                        ui.separator();
                        ui.label("Call Stack");
                        for frame in self.debug_call_stack.clone() {
                            ui.label(frame);
                        }
                        if ui.button("Start Debug Session").clicked() {
                            self.active_editor().lsp_status =
                                "Debug: session requested".to_string();
                            self.debug_call_stack = vec![
                                "main()".to_string(),
                                "app::update()".to_string(),
                                "editor_view::show()".to_string(),
                            ];
                        }
                        ui.separator();
                        ui.heading("Task Runner");
                        for task in self.tasks.clone() {
                            ui.horizontal(|ui| {
                                ui.label(task.name.clone());
                                if ui.button("Run").clicked() {
                                    self.run_workspace_task(&task.command);
                                }
                            });
                        }
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_task_name)
                                    .hint_text("task name"),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_task_command)
                                    .hint_text("task command"),
                            );
                        });
                        if ui.button("Add Task").clicked() {
                            let name = self.new_task_name.trim().to_string();
                            let cmd = self.new_task_command.trim().to_string();
                            if !name.is_empty() && !cmd.is_empty() {
                                self.tasks.push(WorkspaceTask { name, command: cmd });
                                self.new_task_name.clear();
                                self.new_task_command.clear();
                            }
                        }
                        ui.separator();
                        ui.heading("Run Configurations");
                        for cfg in self.run_configs.clone() {
                            ui.horizontal(|ui| {
                                ui.label(cfg.name.clone());
                                if ui.button("Run").clicked() {
                                    self.run_configuration(&cfg);
                                }
                            });
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_run_config_name)
                                .hint_text("config name"),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_run_config_command)
                                .hint_text("command"),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_run_config_env)
                                .hint_text("env overrides: A=1;B=2"),
                        );
                        if ui.button("Add Run Configuration").clicked() {
                            let name = self.new_run_config_name.trim().to_string();
                            let command = self.new_run_config_command.trim().to_string();
                            if !name.is_empty() && !command.is_empty() {
                                self.run_configs.push(RunConfiguration {
                                    name,
                                    command,
                                    env_overrides: self.new_run_config_env.trim().to_string(),
                                });
                                self.new_run_config_name.clear();
                                self.new_run_config_command.clear();
                                self.new_run_config_env.clear();
                            }
                        }
                    }
                    SidebarTab::Collab => {
                        ui.heading("Live Share");
                        ui.checkbox(&mut self.collab_enabled, "Enable session");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.collab_session_id)
                                .hint_text("session id"),
                        );
                        if ui.button("Start/Join Session").clicked() {
                            if self.collab_session_id.trim().is_empty() {
                                self.collab_session_id = format!("session-{}", (now_secs() as u64));
                            }
                            self.collab_enabled = true;
                        }
                        ui.separator();
                        ui.label("Peer cursors");
                        for peer in self.collab_peer_cursors.clone() {
                            ui.label(peer);
                        }
                        ui.separator();
                        ui.checkbox(&mut self.collab_review_mode, "Review mode");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.collab_note_input)
                                .hint_text("Inline note for current line"),
                        );
                        if ui.button("Add Note").clicked() {
                            if let Some(path) = self.editors[self.active_tab].file_path.clone() {
                                let line = self.editors[self.active_tab].cursors[0].pos.line + 1;
                                let note = self.collab_note_input.trim().to_string();
                                if !note.is_empty() {
                                    self.collab_notes
                                        .entry(path)
                                        .or_default()
                                        .push((line, note));
                                    self.collab_note_input.clear();
                                }
                            }
                        }
                        for (path, notes) in self.collab_notes.clone() {
                            for (line, note) in notes {
                                ui.label(format!("{}:{} {}", path.to_string_lossy(), line, note));
                            }
                        }
                        if ui.button("Export Handoff Snapshot").clicked() {
                            self.export_handoff_snapshot();
                        }
                    }
                }
            });
    }

    pub(super) fn ensure_split_secondary(&mut self) {
        if self.split_mode == SplitMode::None || self.editors.len() < 2 {
            self.split_secondary_tab = None;
            return;
        }
        if self
            .split_secondary_tab
            .map(|idx| idx < self.editors.len() && idx != self.active_tab)
            .unwrap_or(false)
        {
            return;
        }
        self.split_secondary_tab = (0..self.editors.len()).find(|idx| *idx != self.active_tab);
    }

    pub(super) fn run_terminal_command(&mut self, secondary: bool) {
        let command = if secondary {
            self.terminal_input_secondary.trim().to_string()
        } else {
            self.terminal_input.trim().to_string()
        };
        if command.is_empty() {
            return;
        }
        let profile = self
            .terminal_profiles
            .get(self.terminal_profile_idx)
            .cloned()
            .unwrap_or(TerminalProfile {
                name: "Default".to_string(),
                shell: "sh".to_string(),
                theme_hint: "Dark".to_string(),
            });
        if secondary {
            self.terminal_log_secondary
                .push(format!("[{}] $ {}", profile.name, command));
        } else {
            self.terminal_log
                .push(format!("[{}] $ {}", profile.name, command));
        }
        let target = if secondary {
            AsyncCommandTarget::TerminalSecondary
        } else {
            AsyncCommandTarget::TerminalPrimary
        };
        self.spawn_shell_command(
            target,
            format!("terminal {}", profile.name),
            profile.shell,
            command,
            self.workspace_root.clone(),
            Vec::new(),
        );
        if secondary {
            self.terminal_input_secondary.clear();
        } else {
            self.terminal_input.clear();
        }
    }

    pub(super) fn add_breakpoint_at_cursor(&mut self) {
        let Some(path) = self.editors[self.active_tab].file_path.clone() else {
            self.refactor_status = "No file for breakpoint".to_string();
            return;
        };
        let line = self.editors[self.active_tab].cursors[0].pos.line + 1;
        let set = self.debug_breakpoints.entry(path).or_default();
        if !set.insert(line) {
            set.remove(&line);
        }
    }

    pub(super) fn run_workspace_task(&mut self, command: &str) {
        if !self.trusted_workspaces.contains(&self.workspace_root) {
            self.push_output_log("Task blocked: workspace not trusted".to_string());
            return;
        }
        self.push_output_log(format!("Task started: {}", command));
        self.spawn_shell_command(
            AsyncCommandTarget::Task,
            command.to_string(),
            "sh".to_string(),
            command.to_string(),
            self.workspace_root.clone(),
            Vec::new(),
        );
    }

    pub(super) fn run_configuration(&mut self, cfg: &RunConfiguration) {
        if !self.trusted_workspaces.contains(&self.workspace_root) {
            self.push_output_log("Run config blocked: workspace not trusted".to_string());
            return;
        }
        let mut env_overrides = Vec::new();
        for pair in cfg.env_overrides.split(';') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            if let Some((k, v)) = pair.split_once('=') {
                env_overrides.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        self.push_output_log(format!("Run config started: {}", cfg.name));
        self.spawn_shell_command(
            AsyncCommandTarget::RunConfig,
            cfg.name.clone(),
            "sh".to_string(),
            cfg.command.clone(),
            self.workspace_root.clone(),
            env_overrides,
        );
    }

    pub(super) fn spawn_shell_command(
        &self,
        target: AsyncCommandTarget,
        label: String,
        shell: String,
        command: String,
        cwd: PathBuf,
        env_overrides: Vec<(String, String)>,
    ) {
        let tx = self.async_cmd_tx.clone();
        thread::spawn(move || {
            let mut cmd = Command::new(shell);
            cmd.arg("-lc").arg(&command).current_dir(cwd);
            for (k, v) in env_overrides {
                cmd.env(k, v);
            }
            let result = match cmd.output() {
                Ok(output) => AsyncCommandResult {
                    target,
                    label,
                    stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
                    success: output.status.success(),
                    error: None,
                },
                Err(err) => AsyncCommandResult {
                    target,
                    label,
                    stdout: String::new(),
                    stderr: String::new(),
                    success: false,
                    error: Some(err.to_string()),
                },
            };
            let _ = tx.send(result);
        });
    }

    pub(super) fn poll_async_command_results(&mut self) {
        while let Ok(result) = self.async_cmd_rx.try_recv() {
            let AsyncCommandResult {
                target,
                label,
                stdout,
                stderr,
                success,
                error,
            } = result;
            match target {
                AsyncCommandTarget::TerminalPrimary => {
                    if !stdout.is_empty() {
                        self.terminal_log.push(stdout.clone());
                        self.push_output_log(stdout);
                    }
                    if !stderr.is_empty() {
                        self.terminal_log.push(stderr.clone());
                        self.push_output_log(stderr);
                    }
                    if let Some(err) = error {
                        self.terminal_log.push(format!("error: {err}"));
                        self.push_output_log(format!("Terminal error: {err}"));
                    }
                    Self::cap_vec(&mut self.terminal_log, 500);
                }
                AsyncCommandTarget::TerminalSecondary => {
                    if !stdout.is_empty() {
                        self.terminal_log_secondary.push(stdout.clone());
                        self.push_output_log(stdout);
                    }
                    if !stderr.is_empty() {
                        self.terminal_log_secondary.push(stderr.clone());
                        self.push_output_log(stderr);
                    }
                    if let Some(err) = error {
                        self.terminal_log_secondary.push(format!("error: {err}"));
                        self.push_output_log(format!("Terminal error: {err}"));
                    }
                    Self::cap_vec(&mut self.terminal_log_secondary, 500);
                }
                AsyncCommandTarget::Task => {
                    if !stdout.is_empty() {
                        self.push_output_log(stdout);
                    }
                    if !stderr.is_empty() {
                        self.push_output_log(stderr);
                    }
                    if let Some(err) = error {
                        self.push_output_log(format!("Task error: {err}"));
                    } else {
                        self.push_output_log(format!("Task finished: {success} ({label})"));
                    }
                }
                AsyncCommandTarget::RunConfig => {
                    if !stdout.is_empty() {
                        self.push_output_log(stdout);
                    }
                    if !stderr.is_empty() {
                        self.push_output_log(stderr);
                    }
                    if let Some(err) = error {
                        self.push_output_log(format!("Run config error: {err}"));
                    } else {
                        self.push_output_log(format!("Run config finished: {success} ({label})"));
                    }
                }
            }
        }
    }
}
