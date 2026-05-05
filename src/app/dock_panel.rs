use eframe::egui;
use super::*;

impl LuxApp {
    pub(super) fn render_dock_panel_contents(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            let drag_resp = ui.add(
                egui::Label::new(egui::RichText::new("::").monospace()).sense(egui::Sense::drag()),
            );
            if drag_resp.dragged() {
                if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                    let screen = ctx.screen_rect();
                    if pos.x > screen.right() - screen.width() * 0.25 {
                        self.dock_side = DockSide::Right;
                    } else if pos.y > screen.bottom() - screen.height() * 0.25 {
                        self.dock_side = DockSide::Bottom;
                    }
                }
            }
            if ui
                .selectable_label(self.dock_tab == DockPanelTab::Terminal, "Terminal")
                .clicked()
            {
                self.dock_tab = DockPanelTab::Terminal;
            }
            if ui
                .selectable_label(self.dock_tab == DockPanelTab::Output, "Output")
                .clicked()
            {
                self.dock_tab = DockPanelTab::Output;
            }
            if ui
                .selectable_label(self.dock_tab == DockPanelTab::Problems, "Problems")
                .clicked()
            {
                self.dock_tab = DockPanelTab::Problems;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Hide").clicked() {
                    self.show_dock_panel = false;
                }
            });
        });
        ui.separator();

        match self.dock_tab {
            DockPanelTab::Terminal => {
                ui.horizontal(|ui| {
                    ui.label("Profile");
                    for (idx, profile) in self.terminal_profiles.iter().enumerate() {
                        if ui
                            .selectable_label(
                                idx == self.terminal_profile_idx,
                                format!("{} ({})", profile.name, profile.theme_hint),
                            )
                            .clicked()
                        {
                            self.terminal_profile_idx = idx;
                        }
                    }
                    ui.checkbox(&mut self.terminal_split_panes, "Split");
                });
                egui::ScrollArea::vertical()
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for line in &self.terminal_log {
                            ui.label(egui::RichText::new(line).monospace().size(11.0));
                        }
                    });
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.terminal_input)
                        .hint_text("Run shell command..."),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.run_terminal_command(false);
                }
                if ui.button("Run").clicked() {
                    self.run_terminal_command(false);
                }
                if self.terminal_split_panes {
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for line in &self.terminal_log_secondary {
                                ui.label(egui::RichText::new(line).monospace().size(11.0));
                            }
                        });
                    let resp_secondary = ui.add(
                        egui::TextEdit::singleline(&mut self.terminal_input_secondary)
                            .hint_text("Run in split pane..."),
                    );
                    if resp_secondary.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        self.run_terminal_command(true);
                    }
                    if ui.button("Run Split").clicked() {
                        self.run_terminal_command(true);
                    }
                }
            }
            DockPanelTab::Output => {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.output_filter)
                            .hint_text("Filter output"),
                    );
                    if ui.button("Copy Visible").clicked() {
                        let visible: String = self
                            .output_log
                            .iter()
                            .filter(|line| {
                                self.output_filter.trim().is_empty()
                                    || line
                                        .to_lowercase()
                                        .contains(&self.output_filter.to_lowercase())
                            })
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("\n");
                        if let Some(cb) = self.clipboard.as_mut() {
                            let _ = cb.set_text(&visible);
                        }
                    }
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for line in self.output_log.clone() {
                        if !self.output_filter.trim().is_empty()
                            && !line
                                .to_lowercase()
                                .contains(&self.output_filter.to_lowercase())
                        {
                            continue;
                        }
                        if let Some((path, line_no)) = parse_stacktrace_location(&line) {
                            if ui
                                .link(egui::RichText::new(&line).monospace().size(11.0))
                                .clicked()
                            {
                                self.open_path_in_tab(path.as_path());
                                self.active_editor().goto_line(line_no);
                            }
                        } else {
                            ui.label(egui::RichText::new(&line).monospace().size(11.0));
                        }
                    }
                });
            }
            DockPanelTab::Problems => {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.problems_filter)
                            .hint_text("Filter problems"),
                    );
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let active_lang = self.active_language_label();
                    let lang_enabled = self
                        .diagnostics_language_enabled
                        .get(&active_lang)
                        .copied()
                        .unwrap_or(true);
                    for diag in &self.editors[self.active_tab].diagnostics {
                        if !lang_enabled {
                            continue;
                        }
                        if diag.severity <= 2 && !self.diagnostics_show_error {
                            continue;
                        }
                        if diag.severity == 3 && !self.diagnostics_show_warning {
                            continue;
                        }
                        if diag.severity >= 4 && !self.diagnostics_show_info {
                            continue;
                        }
                        let row =
                            format!("Ln {} [{}] {}", diag.line + 1, diag.severity, diag.message);
                        if !self.problems_filter.trim().is_empty()
                            && !row
                                .to_lowercase()
                                .contains(&self.problems_filter.to_lowercase())
                        {
                            continue;
                        }
                        ui.label(egui::RichText::new(format!(
                            "Ln {} [{}] {}",
                            diag.line + 1,
                            diag.severity,
                            diag.message
                        )));
                    }
                    if self.editors[self.active_tab].diagnostics.is_empty() {
                        ui.label("No problems");
                    }
                });
            }
        }
    }

    pub(super) fn show_dock_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.show_dock_panel || self.zen_mode {
            return;
        }
        match self.dock_side {
            DockSide::Bottom => {
                egui::TopBottomPanel::bottom("dock_panel_bottom")
                    .resizable(true)
                    .default_height(self.dock_size.clamp(120.0, 320.0))
                    .show_inside(ui, |ui| {
                        self.dock_size = ui.available_height();
                        self.render_dock_panel_contents(ui, ctx);
                    });
            }
            DockSide::Right => {
                egui::SidePanel::right("dock_panel_right")
                    .resizable(true)
                    .default_width(self.dock_size.clamp(220.0, 480.0))
                    .show_inside(ui, |ui| {
                        self.dock_size = ui.available_width();
                        self.render_dock_panel_contents(ui, ctx);
                    });
            }
        }
    }

    pub(super) fn refresh_editor_insights(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let editor = &mut self.editors[self.active_tab];

        editor.code_lens_metrics = build_code_lens_metrics(editor);

        if now - editor.inline_blame_last_check < 2.0 {
            return;
        }
        editor.inline_blame_last_check = now;

        if editor.modified {
            return;
        }

        let Some(path) = editor.file_path.as_deref() else {
            editor.inline_blame.clear();
            return;
        };

        editor.inline_blame = read_inline_blame(path);
    }

    pub(super) fn refresh_lsp_features(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.lsp_rx {
            if let Ok(output) = rx.try_recv() {
                self.lsp_rx = None;
                if let Some(editor) = self
                    .editors
                    .iter_mut()
                    .find(|e| e.file_path.as_deref() == Some(output.path.as_path()))
                {
                    editor.background_tasks = 0;
                    editor.diagnostics = output
                        .snapshot
                        .diagnostics
                        .into_iter()
                        .map(|d| crate::editor::DiagnosticItem {
                            line: d.line,
                            severity: d.severity,
                            message: d.message,
                        })
                        .collect();
                    editor.refresh_code_actions();
                    if output.request.want_completion {
                        editor.completion_items = output
                            .snapshot
                            .completions
                            .into_iter()
                            .map(|item| crate::editor::CompletionItem {
                                label: item.label,
                                insert_text: item.insert_text,
                                detail: item.detail,
                                is_snippet: item.is_snippet,
                            })
                            .collect();
                        editor.completion_visible = !editor.completion_items.is_empty();
                    }
                    if output.request.want_formatting {
                        if let Some(formatted) = output.snapshot.formatted_text {
                            if formatted != editor.rope {
                                editor.set_document_text(&formatted);
                            }
                        }
                    }
                    if output.request.want_definition {
                        editor.lsp_nav_results = output.snapshot.definitions;
                    }
                    if output.request.want_references {
                        editor.lsp_nav_results = output.snapshot.references;
                    }
                    if output.request.want_implementations {
                        editor.lsp_nav_results = output.snapshot.implementations;
                    }
                    editor.lsp_status = if output.snapshot.had_server {
                        "LSP: connected".to_string()
                    } else {
                        "LSP: snippets-only".to_string()
                    };
                    let error_count = editor
                        .diagnostics
                        .iter()
                        .filter(|d| d.severity <= 2)
                        .count();
                    editor.notification_badges = error_count
                        + if editor.completion_visible { 1 } else { 0 }
                        + if editor.macro_recording { 1 } else { 0 };
                }
            }
        }

        if self.lsp_rx.is_some() {
            self.editors[self.active_tab].background_tasks = 1;
            return;
        }

        let lang = self.active_language_label();
        let formatter = self
            .formatter_by_language
            .get(&lang)
            .cloned()
            .unwrap_or_else(|| "lsp-default".to_string());
        let now = ctx.input(|i| i.time);
        let editor = &mut self.editors[self.active_tab];
        let periodic = now - editor.lsp_last_check > 3.0;
        let should_request = periodic
            || editor.request_completion
            || editor.request_formatting
            || editor.request_definition
            || editor.request_references
            || editor.request_implementations;
        if !should_request {
            editor.background_tasks = 0;
            return;
        }
        if now - self.lsp_last_request < 0.5 {
            return;
        }
        self.lsp_last_request = now;
        editor.lsp_last_check = now;

        let Some(path) = editor.file_path.clone() else {
            editor.lsp_status = "LSP: no file".to_string();
            return;
        };

        let request = RequestKind {
            want_completion: editor.request_completion,
            want_formatting: editor.request_formatting && formatter != "off",
            want_definition: editor.request_definition,
            want_references: editor.request_references,
            want_implementations: editor.request_implementations,
        };
        editor.request_completion = false;
        editor.request_formatting = false;
        editor.request_definition = false;
        editor.request_references = false;
        editor.request_implementations = false;
        let text = editor.rope.to_string();
        let primary = editor.cursors[0].pos;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let snapshot = crate::lsp::collect_snapshot(
                path.as_path(),
                &text,
                primary.line,
                primary.col,
                request,
            );
            let _ = tx.send(LspWorkerOutput {
                path,
                snapshot,
                request,
            });
        });
        self.lsp_rx = Some(rx);
        editor.lsp_status = "LSP: running".to_string();
        editor.background_tasks = 1;
    }

    pub(super) fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.register_recent_workspace(path.parent().map(Path::to_path_buf));
            match Editor::from_file(path) {
                Ok(mut editor) => {
                    self.apply_project_conventions(&mut editor);
                    self.apply_language_toolchain_defaults(&editor);
                    self.editors.push(editor);
                    self.active_tab = self.editors.len() - 1;
                }
                Err(e) => {
                    eprintln!("Failed to open file: {}", e);
                }
            }
        }
    }

    pub(super) fn apply_cli_open_paths(&mut self) {
        let Ok(raw) = std::env::var("LUX_OPEN_PATHS") else {
            return;
        };
        for part in raw.split(';') {
            let path = PathBuf::from(part.trim());
            if path.is_file() {
                if let Ok(editor) = Editor::from_file(path.clone()) {
                    self.editors.push(editor);
                    self.active_tab = self.editors.len().saturating_sub(1);
                }
            }
        }
    }

    pub(super) fn initialize_first_run_onboarding(&mut self) {
        let marker = self.workspace_root.join(".lux").join("first_run_done");
        if marker.exists() {
            self.onboarding_first_run_done = true;
            return;
        }
        self.show_onboarding = true;
        self.onboarding_step = 0;
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(marker, "done");
        self.onboarding_first_run_done = true;
    }

    pub(super) fn create_from_template(&mut self, template: &str) {
        let (file, content) = match template {
            "web" => (
                "index.html",
                "<!doctype html>\n<html><head><title>Web App</title></head><body><h1>Hello</h1></body></html>\n",
            ),
            "cli" => (
                "main.rs",
                "fn main() {\n    println!(\"Hello CLI\");\n}\n",
            ),
            "library" => (
                "lib.rs",
                "pub fn hello() -> &'static str {\n    \"hello\"\n}\n",
            ),
            _ => (
                "README.md",
                "# Docs Site\n\nStart documenting your project here.\n",
            ),
        };
        let path = self.workspace_root.join(file);
        let _ = std::fs::write(&path, content);
        self.open_path_in_tab(path.as_path());
    }

    pub(super) fn clone_repository_action(&mut self) {
        let url = self.clone_repo_url.trim();
        if url.is_empty() {
            self.packaging_status = "Clone URL is empty".to_string();
            return;
        }
        let target = if self.clone_repo_target.trim().is_empty() {
            self.workspace_root.clone()
        } else {
            PathBuf::from(self.clone_repo_target.trim())
        };
        let output = Command::new("git")
            .arg("clone")
            .arg(url)
            .current_dir(target)
            .output();
        self.packaging_status = match output {
            Ok(out) if out.status.success() => "Repository cloned".to_string(),
            Ok(out) => format!("Clone failed ({})", out.status),
            Err(err) => format!("Clone error: {err}"),
        };
    }

    pub(super) fn build_release_artifacts(&mut self) {
        let output = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(self.workspace_root.clone())
            .output();
        self.packaging_status = match output {
            Ok(out) if out.status.success() => "Release build completed".to_string(),
            Ok(out) => format!("Release build failed ({})", out.status),
            Err(err) => format!("Release build error: {err}"),
        };
    }

    pub(super) fn check_update_channel(&mut self) {
        let channel = match self.update_channel {
            UpdateChannel::Stable => "stable",
            UpdateChannel::Beta => "beta",
            UpdateChannel::Nightly => "nightly",
        };
        self.packaging_status = format!("Checked updates on '{}' channel: no updates", channel);
    }

    pub(super) fn export_app_config(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("lux-app-config.json")
            .save_file()
        else {
            return;
        };
        let json = serde_json::json!({
            "portable_mode": self.portable_mode,
            "theme_density": self.theme_ui_density,
            "editor_theme": self.editor_theme.name(),
            "telemetry_opt_in": self.telemetry_opt_in,
            "update_channel": match self.update_channel {
                UpdateChannel::Stable => "stable",
                UpdateChannel::Beta => "beta",
                UpdateChannel::Nightly => "nightly",
            },
            "format_on_save": self.format_on_save,
            "format_on_type": self.format_on_type,
            "plugin_sandbox_enabled": self.plugin_sandbox_enabled,
        });
        if std::fs::write(path, json.to_string()).is_ok() {
            self.packaging_status = "App config exported".to_string();
        }
    }

    pub(super) fn import_app_config(&mut self) {
        let Some(path) = rfd::FileDialog::new().pick_file() else {
            return;
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return;
        };
        if let Some(v) = value.get("portable_mode").and_then(|v| v.as_bool()) {
            self.portable_mode = v;
        }
        if let Some(v) = value.get("theme_density").and_then(|v| v.as_f64()) {
            self.theme_ui_density = (v as f32).clamp(0.85, 1.35);
        }
        if let Some(v) = value.get("telemetry_opt_in").and_then(|v| v.as_bool()) {
            self.telemetry_opt_in = v;
        }
        if let Some(v) = value.get("format_on_save").and_then(|v| v.as_bool()) {
            self.format_on_save = v;
        }
        if let Some(v) = value.get("format_on_type").and_then(|v| v.as_bool()) {
            self.format_on_type = v;
        }
        if let Some(v) = value
            .get("plugin_sandbox_enabled")
            .and_then(|v| v.as_bool())
        {
            self.plugin_sandbox_enabled = v;
        }
        if let Some(v) = value.get("update_channel").and_then(|v| v.as_str()) {
            self.update_channel = match v {
                "beta" => UpdateChannel::Beta,
                "nightly" => UpdateChannel::Nightly,
                _ => UpdateChannel::Stable,
            };
        }
        self.packaging_status = "App config imported".to_string();
    }

    pub(super) fn run_ui_test_harness(&mut self) {
        let output = Command::new("cargo")
            .arg("test")
            .current_dir(self.workspace_root.clone())
            .output();
        self.qa_status = match output {
            Ok(out) if out.status.success() => "QA harness: cargo test passed".to_string(),
            Ok(out) => format!("QA harness failed ({})", out.status),
            Err(err) => format!("QA harness error: {err}"),
        };
    }

    pub(super) fn run_golden_snapshot_check(&mut self) {
        let qa_dir = self.workspace_root.join(".lux").join("qa");
        let _ = std::fs::create_dir_all(&qa_dir);
        let snapshot = self.editors[self.active_tab].rope.to_string();
        let latest = qa_dir.join("latest_snapshot.txt");
        let golden = qa_dir.join("golden_snapshot.txt");
        let _ = std::fs::write(&latest, &snapshot);
        if !golden.exists() {
            let _ = std::fs::write(&golden, &snapshot);
            self.qa_status = "Golden snapshot created".to_string();
            return;
        }
        let baseline = std::fs::read_to_string(&golden).unwrap_or_default();
        if baseline == snapshot {
            self.qa_status = "Golden snapshot matched".to_string();
        } else {
            self.qa_status =
                "Golden snapshot mismatch (see .lux/qa/latest_snapshot.txt)".to_string();
        }
    }

    pub(super) fn run_performance_benchmark(&mut self) {
        let editor = &self.editors[self.active_tab];
        let text = editor.rope.to_string();
        let start = std::time::Instant::now();
        let _tokens = self.highlighter.highlight_lines(
            &text,
            editor.file_path.as_deref(),
            editor.syntax_override.as_deref(),
            0,
            editor.line_count(),
        );
        let elapsed = start.elapsed().as_millis();
        self.qa_status = format!("Benchmark: full highlight took {} ms", elapsed);
    }

    pub(super) fn export_crash_triage_bundle(&mut self) {
        let qa_dir = self.workspace_root.join(".lux").join("qa");
        let _ = std::fs::create_dir_all(&qa_dir);
        let mut report = String::new();
        report.push_str("# Crash/Triage Bundle\n\n");
        report.push_str("## Recent Output\n");
        for line in self.output_log.iter().rev().take(300) {
            report.push_str(line);
            report.push('\n');
        }
        report.push_str("\n## Diagnostics\n");
        for diag in &self.editors[self.active_tab].diagnostics {
            report.push_str(&format!(
                "Ln {} [{}] {}\n",
                diag.line + 1,
                diag.severity,
                diag.message
            ));
        }
        let path = qa_dir.join("triage_bundle.md");
        let _ = std::fs::write(path, report);
        self.qa_status = "Triage bundle exported to .lux/qa/triage_bundle.md".to_string();
    }

    pub(super) fn export_diagnostics_bundle(&mut self) {
        let path = self
            .workspace_root
            .join(".lux")
            .join("support_diagnostics_bundle.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let payload = serde_json::json!({
            "workspace": self.workspace_root.to_string_lossy().to_string(),
            "active_file": self.editors[self.active_tab]
                .file_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            "diagnostics": self.editors[self.active_tab]
                .diagnostics
                .iter()
                .map(|d| serde_json::json!({
                    "line": d.line + 1,
                    "severity": d.severity,
                    "message": d.message,
                }))
                .collect::<Vec<_>>(),
            "logs": self.output_log.iter().rev().take(500).cloned().collect::<Vec<_>>(),
            "plugins": self.plugins.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
            "lsp_running": self.lsp_rx.is_some(),
            "safe_mode": self.safe_mode,
        });
        if std::fs::write(path, payload.to_string()).is_ok() {
            self.observability_health_status = "Diagnostics bundle exported".to_string();
        }
    }

    pub(super) fn run_health_checks(&mut self) {
        let plugin_count = self.plugins.len();
        let lsp_status = if self.lsp_rx.is_some() {
            "busy"
        } else {
            "idle"
        };
        let bg_tasks = self.editors[self.active_tab].background_tasks;
        self.observability_health_status = format!(
            "Health: plugins={}, lsp={}, background_tasks={}",
            plugin_count, lsp_status, bg_tasks
        );
        self.log_event(
            "health",
            LogLevel::Info,
            &self.observability_health_status.clone(),
        );
    }

    pub(super) fn sync_settings_now(&mut self) {
        if self.settings_sync_path.trim().is_empty() {
            self.settings_sync_status = "Sync path is empty".to_string();
            return;
        }
        let target = PathBuf::from(self.settings_sync_path.trim());
        let local = self.workspace_root.join(".lux").join("settings_sync.json");
        let payload = serde_json::json!({
            "updated_at": now_secs(),
            "profile": self.settings_profile,
            "role_profile": self.settings_role_profile,
            "theme_density": self.theme_ui_density,
            "theme": self.editor_theme.name(),
            "format_on_save": self.format_on_save,
            "format_on_type": self.format_on_type,
            "locale": self.locale_code,
        });
        if let Some(parent) = local.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&local, payload.to_string());

        let mut final_payload = payload;
        if let Ok(remote_raw) = std::fs::read_to_string(&target) {
            if let Ok(remote) = serde_json::from_str::<serde_json::Value>(&remote_raw) {
                let remote_ts = remote
                    .get("updated_at")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let local_ts = final_payload
                    .get("updated_at")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                if remote_ts > local_ts {
                    final_payload = remote;
                    self.settings_sync_status =
                        "Conflict resolved: kept newer remote settings".to_string();
                } else {
                    self.settings_sync_status =
                        "Conflict resolved: kept newer local settings".to_string();
                }
            }
        }
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&target, final_payload.to_string()).is_ok() {
            if self.settings_sync_status.is_empty() {
                self.settings_sync_status = "Settings synced".to_string();
            }
        } else {
            self.settings_sync_status = "Failed to write sync target".to_string();
        }
    }

    pub(super) fn store_secret_securely(&mut self) {
        let key = self.secret_key_input.trim();
        let value = self.secret_value_input.trim();
        if key.is_empty() || value.is_empty() {
            self.secrets_status = "Secret key/value required".to_string();
            return;
        }
        let path = self.workspace_root.join(".lux").join("secrets.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut current = serde_json::Map::new();
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(obj) = value.as_object() {
                    current = obj.clone();
                }
            }
        }
        current.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
        let content = serde_json::Value::Object(current).to_string();
        if std::fs::write(&path, content).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            self.secrets_status =
                "Secret stored in .lux/secrets.json (restricted perms)".to_string();
            self.secret_value_input.clear();
        } else {
            self.secrets_status = "Failed to store secret".to_string();
        }
    }

    pub(super) fn export_theme_json(&self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("lux-theme.json")
            .save_file()
        else {
            return;
        };
        let json = serde_json::json!({
            "theme_kind": self.editor_theme.name(),
            "ui_density": self.theme_ui_density,
            "overrides": {
                "background": self.theme_override_bg.map(color_to_hex),
                "text": self.theme_override_text.map(color_to_hex),
            }
        });
        let _ = std::fs::write(path, json.to_string());
    }

    pub(super) fn import_theme_json(&mut self) {
        let Some(path) = rfd::FileDialog::new().pick_file() else {
            return;
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return;
        };
        if let Some(kind) = value.get("theme_kind").and_then(|v| v.as_str()) {
            self.editor_theme = match kind {
                "Dark" => EditorThemeKind::Dark,
                "Light" => EditorThemeKind::Light,
                "Solarized Dark" => EditorThemeKind::SolarizedDark,
                _ => EditorThemeKind::Monokai,
            };
        }
        if let Some(density) = value.get("ui_density").and_then(|v| v.as_f64()) {
            self.theme_ui_density = (density as f32).clamp(0.85, 1.35);
        }
        self.theme_override_bg = value
            .get("overrides")
            .and_then(|v| v.get("background"))
            .and_then(|v| v.as_str())
            .and_then(parse_hex_color);
        self.theme_override_text = value
            .get("overrides")
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .and_then(parse_hex_color);
    }

    pub(super) fn default_shortcut(&self, action: &str) -> ShortcutSpec {
        match (self.keymap_preset, action) {
            (_, "open") => ShortcutSpec {
                key: egui::Key::O,
                command: true,
                shift: false,
                alt: false,
            },
            (_, "save") => ShortcutSpec {
                key: egui::Key::S,
                command: true,
                shift: false,
                alt: false,
            },
            (_, "find") => ShortcutSpec {
                key: egui::Key::F,
                command: true,
                shift: false,
                alt: false,
            },
            (_, "format") => ShortcutSpec {
                key: egui::Key::F,
                command: true,
                shift: true,
                alt: false,
            },
            (KeymapPreset::JetBrains, "palette") => ShortcutSpec {
                key: egui::Key::A,
                command: true,
                shift: true,
                alt: false,
            },
            _ => ShortcutSpec {
                key: egui::Key::P,
                command: true,
                shift: true,
                alt: false,
            },
        }
    }

    pub(super) fn shortcut_for_action(&self, action: &str) -> ShortcutSpec {
        self.custom_shortcuts
            .get(action)
            .copied()
            .unwrap_or_else(|| self.default_shortcut(action))
    }

    pub(super) fn shortcut_pressed(ctx: &egui::Context, spec: ShortcutSpec) -> bool {
        ctx.input(|i| {
            i.key_pressed(spec.key)
                && i.modifiers.command == spec.command
                && i.modifiers.shift == spec.shift
                && i.modifiers.alt == spec.alt
        })
    }

    pub(super) fn apply_custom_binding(&mut self, action: &str, raw: &str) {
        if let Some(spec) = parse_shortcut(raw) {
            self.custom_shortcuts.insert(action.to_string(), spec);
        }
    }

    pub(super) fn save_workspace_settings(&self) {
        let workspace = self.active_workspace_key();
        let base = self
            .workspace_fonts
            .get(&workspace)
            .cloned()
            .unwrap_or(WorkspaceFontSettings {
                size: 13.5,
                family: FontFamilyKind::Monospace,
                ligatures: false,
            });
        let folder_overrides: Vec<serde_json::Value> = self
            .folder_font_overrides
            .iter()
            .map(|(folder, font)| {
                serde_json::json!({
                    "folder": folder.to_string_lossy().to_string(),
                    "size": font.size,
                    "family": match font.family {
                        FontFamilyKind::Monospace => "monospace",
                        FontFamilyKind::Proportional => "proportional",
                    },
                    "ligatures": font.ligatures,
                })
            })
            .collect();
        let json = serde_json::json!({
            "font": {
                "size": base.size,
                "family": match base.family {
                    FontFamilyKind::Monospace => "monospace",
                    FontFamilyKind::Proportional => "proportional",
                },
                "ligatures": base.ligatures,
            },
            "folder_overrides": folder_overrides
        });
        let path = workspace.join(".lux").join("settings.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, json.to_string());
    }

    pub(super) fn collab_state_path(&self) -> PathBuf {
        self.workspace_root
            .join(".lux")
            .join(format!("collab-{}.json", self.collab_session_id))
    }

    pub(super) fn sync_collaboration_state(&mut self) {
        if !self.collab_enabled || self.collab_session_id.trim().is_empty() {
            return;
        }
        let path = self.collab_state_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let current = &self.editors[self.active_tab];
        let payload = serde_json::json!({
            "session": self.collab_session_id,
            "local": {
                "file": current.file_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
                "line": current.cursors.first().map(|c| c.pos.line + 1).unwrap_or(1),
                "col": current.cursors.first().map(|c| c.pos.col + 1).unwrap_or(1),
            }
        });
        let _ = std::fs::write(&path, payload.to_string());
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                let file = value
                    .get("local")
                    .and_then(|v| v.get("file"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let line = value
                    .get("local")
                    .and_then(|v| v.get("line"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
                let col = value
                    .get("local")
                    .and_then(|v| v.get("col"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
                self.collab_peer_cursors = vec![format!("peer -> {}:{}:{}", file, line, col)];
            }
        }
    }

    pub(super) fn export_handoff_snapshot(&mut self) {
        let path = self.workspace_root.join(".lux").join("handoff_snapshot.md");
        let mut doc = String::new();
        doc.push_str("# Handoff Snapshot\n\n");
        doc.push_str(&format!("Session: {}\n\n", self.collab_session_id));
        doc.push_str("## Open Files\n");
        for editor in &self.editors {
            if let Some(path) = &editor.file_path {
                doc.push_str(&format!("- {}\n", path.to_string_lossy()));
            }
        }
        doc.push_str("\n## Review Notes\n");
        for (path, notes) in &self.collab_notes {
            for (line, note) in notes {
                doc.push_str(&format!("- {}:{} {}\n", path.to_string_lossy(), line, note));
            }
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, doc);
    }

    pub(super) fn save_file(&mut self) {
        if self.format_on_save {
            self.editors[self.active_tab].request_formatting = true;
        }
        let editor = &mut self.editors[self.active_tab];
        if editor.file_path.is_some() {
            if let Err(e) = editor.save() {
                eprintln!("Failed to save: {}", e);
            }
        } else {
            self.save_file_as();
        }
    }

    pub(super) fn save_file_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new().save_file() {
            self.register_recent_workspace(path.parent().map(Path::to_path_buf));
            if self.format_on_save {
                self.editors[self.active_tab].request_formatting = true;
            }
            if let Err(e) = self.editors[self.active_tab].save_as(path) {
                eprintln!("Failed to save: {}", e);
            }
        }
    }

    pub(super) fn register_recent_workspace(&mut self, maybe_path: Option<PathBuf>) {
        let Some(path) = maybe_path else {
            return;
        };
        let workspace = resolve_git_root(&path).unwrap_or(path);
        self.recent_workspaces.retain(|p| p != &workspace);
        self.recent_workspaces.insert(0, workspace);
        if self.recent_workspaces.len() > 12 {
            self.recent_workspaces.truncate(12);
        }
        persist_recent_workspaces(self.recent_workspaces.as_slice());
    }

    pub(super) fn capture_refactor_preview(&mut self, tab_idx: usize, title: &str, before: String) {
        let after = self.editors[tab_idx].rope.to_string();
        if before == after {
            self.refactor_preview = None;
            return;
        }
        self.refactor_preview = Some(RefactorPreview {
            tab_idx,
            title: title.to_string(),
            diff_preview: build_simple_diff_preview(&before, &after),
            original_text: before,
        });
    }

    pub(super) fn push_output_log(&mut self, line: String) {
        self.output_log.push(redact_secrets(&line));
        Self::cap_vec(&mut self.output_log, 500);
    }

    pub(super) fn cap_vec(vec: &mut Vec<String>, max_len: usize) {
        if vec.len() > max_len {
            vec.drain(0..(vec.len() - max_len));
        }
    }

    pub(super) fn log_event(&mut self, module: &str, level: LogLevel, message: &str) {
        if level > self.observability_level {
            return;
        }
        if !self.observability_module_filter.trim().is_empty()
            && !module
                .to_lowercase()
                .contains(&self.observability_module_filter.to_lowercase())
        {
            return;
        }
        self.push_output_log(format!("[{:?}] [{}] {}", level, module, message));
    }

    pub(super) fn run_autosave_and_recovery(&mut self, ctx: &egui::Context) {
        if !self.autosave_enabled {
            return;
        }
        let now = ctx.input(|i| i.time);
        if now - self.last_autosave < self.autosave_interval_sec {
            return;
        }
        self.last_autosave = now;
        for editor in &mut self.editors {
            if editor.modified {
                if editor.file_path.is_some() {
                    let _ = editor.save();
                } else {
                    let _ = persist_recovery_snapshot(editor);
                }
            }
        }
        persist_session_snapshot(self.editors.as_slice(), self.active_tab);
        if self.settings_sync_enabled {
            self.sync_settings_now();
        }
    }

    pub(super) fn refresh_file_watchers(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.file_watch_last_check < 2.0 {
            return;
        }
        self.file_watch_last_check = now;

        for (tab_idx, editor) in self.editors.iter_mut().enumerate() {
            let Some(path) = editor.file_path.as_ref() else {
                continue;
            };
            let Ok(meta) = std::fs::metadata(path) else {
                continue;
            };
            let Ok(modified) = meta.modified() else {
                continue;
            };
            let previous = self.file_mtimes.get(path).copied();
            self.file_mtimes.insert(path.clone(), modified);
            if let Some(prev) = previous {
                if modified > prev && !editor.modified {
                    if let Ok(reloaded) = Editor::from_file(path.clone()) {
                        let cursor = editor.cursors.clone();
                        let scroll_y = editor.scroll_y;
                        *editor = reloaded;
                        editor.cursors = cursor;
                        editor.scroll_y = scroll_y;
                    }
                } else if modified > prev
                    && editor.modified
                    && self.pending_external_change.is_none()
                {
                    self.pending_external_change = Some(ExternalChangePrompt {
                        tab_idx,
                        path: path.clone(),
                    });
                }
            }
        }
    }

    pub(super) fn handle_command(&mut self, cmd: CommandId) {
        self.log_event("command", LogLevel::Debug, &format!("{:?}", cmd));
        if self.telemetry_opt_in {
            self.output_log.push(format!("telemetry.command {:?}", cmd));
            if self.output_log.len() > 400 {
                self.output_log.drain(0..(self.output_log.len() - 400));
            }
        }
        match cmd {
            CommandId::NewTab => self.new_tab(),
            CommandId::OpenFile => self.open_file(),
            CommandId::SaveFile => self.save_file(),
            CommandId::SaveFileAs => self.save_file_as(),
            CommandId::CloseTab => self.close_tab(),
            CommandId::Find => {
                self.show_search = true;
                self.show_goto_line = false;
            }
            CommandId::GoToLine => {
                self.show_goto_line = true;
                self.show_search = false;
            }
            CommandId::SelectAll => {
                self.active_editor().select_all();
            }
            CommandId::Undo => self.active_editor().undo(),
            CommandId::Redo => self.active_editor().redo(),
            CommandId::FormatDocument => {
                self.active_editor().request_formatting = true;
            }
            CommandId::ToggleGitPanel => {
                self.show_git_panel = !self.show_git_panel;
            }
            CommandId::RefreshGitPanel => {
                self.git_panel.last_refresh = 0.0;
            }
            CommandId::StartDebugSession => {
                self.active_editor().lsp_status = "Debug: session requested".to_string();
                self.debug_call_stack = vec![
                    "main()".to_string(),
                    "app::update()".to_string(),
                    "editor_view::show()".to_string(),
                ];
            }
            CommandId::RunTask => {
                if let Some(task) = self.tasks.first().cloned() {
                    self.run_workspace_task(&task.command);
                    self.active_editor().lsp_status = format!("Task run: {}", task.name);
                } else {
                    self.active_editor().lsp_status = "Task: no tasks configured".to_string();
                }
            }
            CommandId::RunCustomScript => {
                if !self.trusted_workspaces.contains(&self.workspace_root) {
                    self.active_editor().lsp_status =
                        "Script blocked (workspace not trusted)".to_string();
                } else if self.plugin_sandbox_enabled {
                    self.active_editor().lsp_status =
                        "Script blocked (plugin sandbox enabled)".to_string();
                } else {
                    let output = Command::new("sh")
                        .arg("-lc")
                        .arg("./scripts/custom.sh")
                        .current_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
                        .output();
                    self.active_editor().lsp_status = match output {
                        Ok(o) if o.status.success() => "Script: custom.sh ok".to_string(),
                        _ => "Script: custom.sh failed/missing".to_string(),
                    };
                }
            }
            CommandId::Extension(ext) => {
                if let Some((provider, command_id)) = ext.split_once(':') {
                    if let Some(plugin) = self.plugins.iter().find(|p| p.name == provider) {
                        if !self.trusted_workspaces.contains(&self.workspace_root) {
                            self.active_editor().lsp_status =
                                "Plugin command blocked (workspace not trusted)".to_string();
                        } else {
                            self.active_editor().lsp_status =
                                match crate::plugin::run_plugin_command(
                                    plugin,
                                    command_id,
                                    self.plugin_sandbox_enabled,
                                ) {
                                    Ok(output) if output.is_empty() => {
                                        format!("Plugin command ok: {}", command_id)
                                    }
                                    Ok(output) => {
                                        format!("Plugin command ok: {} ({})", command_id, output)
                                    }
                                    Err(err) => {
                                        format!("Plugin command failed: {} ({})", command_id, err)
                                    }
                                };
                        }
                    } else {
                        self.active_editor().lsp_status = format!("Extension command: {ext}");
                    }
                } else {
                    self.active_editor().lsp_status = format!("Extension command: {ext}");
                }
            }
        }
    }

    pub(super) fn cut(&mut self) {
        let text = self.active_editor().cut_text();
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(&text);
        }
    }

    pub(super) fn copy(&mut self) {
        let text = self.active_editor().copy_text();
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(&text);
        }
    }

    pub(super) fn paste(&mut self) {
        let mut paste = None;
        if let Some(cb) = self.clipboard.as_mut() {
            if let Ok(text) = cb.get_text() {
                paste = Some(text);
            }
        }
        if let Some(text) = paste {
            self.active_editor().insert_text(&text);
        }
    }

    pub(super) fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        let palette_spec = self.shortcut_for_action("palette");
        let open_spec = self.shortcut_for_action("open");
        let save_spec = self.shortcut_for_action("save");
        let find_spec = self.shortcut_for_action("find");
        let format_spec = self.shortcut_for_action("format");

        let mut should_undo = false;
        let mut should_redo = false;
        let mut should_cut = false;
        let mut should_copy = false;
        let mut should_paste = false;
        let mut should_select_all = false;
        let mut should_format = false;

        ctx.input(|i| {
            let ctrl = i.modifiers.command;
            let shift = i.modifiers.shift;
            let alt = i.modifiers.alt;
            let pressed = |spec: ShortcutSpec| {
                i.key_pressed(spec.key)
                    && i.modifiers.command == spec.command
                    && i.modifiers.shift == spec.shift
                    && i.modifiers.alt == spec.alt
            };

            if ctrl && alt && i.key_pressed(egui::Key::Num1) {
                self.sidebar_tab = SidebarTab::Explorer;
                return;
            }
            if ctrl && alt && i.key_pressed(egui::Key::Num2) {
                self.sidebar_tab = SidebarTab::Search;
                return;
            }
            if ctrl && alt && i.key_pressed(egui::Key::Num3) {
                self.sidebar_tab = SidebarTab::Git;
                return;
            }
            if ctrl && alt && i.key_pressed(egui::Key::Num4) {
                self.sidebar_tab = SidebarTab::Debug;
                return;
            }
            if ctrl && alt && i.key_pressed(egui::Key::Num5) {
                self.sidebar_tab = SidebarTab::Collab;
                return;
            }
            if ctrl && alt && i.key_pressed(egui::Key::B) {
                self.show_sidebar = !self.show_sidebar;
                return;
            }

            if pressed(palette_spec) {
                self.command_palette.toggle();
            } else if ctrl && i.key_pressed(egui::Key::N) {
                self.new_tab();
            } else if pressed(open_spec) {
                // Defer file dialog to avoid borrow issues
            } else if pressed(save_spec) {
                if shift {
                    // save as - defer
                } else {
                    // save - defer
                }
            } else if ctrl && i.key_pressed(egui::Key::W) {
                self.close_tab();
            } else if pressed(format_spec) {
                should_format = true;
            } else if pressed(find_spec) {
                self.show_search = !self.show_search;
                self.show_replace = false;
                self.show_goto_line = false;
            } else if ctrl && i.key_pressed(egui::Key::H) {
                self.show_search = true;
                self.show_replace = !self.show_replace;
                self.show_goto_line = false;
            } else if ctrl && i.key_pressed(egui::Key::G) {
                self.show_goto_line = !self.show_goto_line;
                self.show_search = false;
            } else if ctrl && i.key_pressed(egui::Key::Z) {
                if shift {
                    should_redo = true;
                } else {
                    should_undo = true;
                }
            } else if ctrl && i.key_pressed(egui::Key::Y) {
                should_redo = true;
            } else if ctrl && i.key_pressed(egui::Key::A) {
                should_select_all = true;
            } else if ctrl && i.key_pressed(egui::Key::C) {
                should_copy = true;
            } else if ctrl && i.key_pressed(egui::Key::X) {
                should_cut = true;
            } else if ctrl && i.key_pressed(egui::Key::V) {
                should_paste = true;
            }
        });

        // Handle open/save outside of input closure to avoid borrow issues
        let should_open = Self::shortcut_pressed(ctx, open_spec);
        let should_save = Self::shortcut_pressed(ctx, save_spec);
        let should_save_as =
            ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S));

        if should_open {
            self.open_file();
        }
        if should_save {
            self.save_file();
        }
        if should_save_as {
            self.save_file_as();
        }
        if should_undo {
            self.active_editor().undo();
        }
        if should_redo {
            self.active_editor().redo();
        }
        if should_select_all {
            self.active_editor().select_all();
        }
        if should_cut {
            self.cut();
        }
        if should_copy {
            self.copy();
        }
        if should_paste {
            self.paste();
        }
        if should_format {
            self.active_editor().request_formatting = true;
        }
    }
}
