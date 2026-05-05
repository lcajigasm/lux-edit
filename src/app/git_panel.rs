use super::*;
use eframe::egui;

impl LuxApp {
    pub(super) fn show_git_panel_ui(&mut self, ui: &mut egui::Ui) {
        if !self.show_git_panel {
            return;
        }
        let Some(repo) = self.active_repo_dir() else {
            return;
        };

        egui::SidePanel::right("git_panel")
            .resizable(true)
            .default_width(360.0)
            .show_inside(ui, |ui| {
                ui.heading("Git");
                ui.label(repo.to_string_lossy());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Refresh").clicked() {
                        self.git_panel.last_refresh = 0.0;
                    }
                    if ui.button("Commit").clicked()
                        && !self.git_panel.commit_message.trim().is_empty()
                    {
                        let _ = git_commit(&repo, self.git_panel.commit_message.trim());
                        self.git_panel.commit_message.clear();
                        self.git_panel.last_refresh = 0.0;
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.git_panel.commit_message)
                        .hint_text("Commit message"),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.git_panel.branch_input)
                        .hint_text("Branch for checkout/merge/rebase"),
                );
                ui.horizontal(|ui| {
                    if ui.button("Checkout").clicked() {
                        if git_checkout_branch(&repo, self.git_panel.branch_input.trim()) {
                            self.git_panel.op_status = "Checkout ok".to_string();
                            self.git_panel.last_refresh = 0.0;
                        } else {
                            self.git_panel.op_status = "Checkout failed".to_string();
                        }
                    }
                    if ui.button("Merge").clicked() {
                        if git_merge_branch(&repo, self.git_panel.branch_input.trim()) {
                            self.git_panel.op_status = "Merge ok".to_string();
                            self.git_panel.last_refresh = 0.0;
                        } else {
                            self.git_panel.op_status = "Merge failed".to_string();
                        }
                    }
                    if ui.button("Rebase").clicked() {
                        if git_rebase_branch(&repo, self.git_panel.branch_input.trim()) {
                            self.git_panel.op_status = "Rebase ok".to_string();
                            self.git_panel.last_refresh = 0.0;
                        } else {
                            self.git_panel.op_status = "Rebase failed".to_string();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.git_panel.stash_message)
                            .hint_text("Stash message"),
                    );
                    if ui.button("Stash Push").clicked() {
                        if git_stash_push(&repo, self.git_panel.stash_message.trim()) {
                            self.git_panel.op_status = "Stash push ok".to_string();
                            self.git_panel.last_refresh = 0.0;
                        } else {
                            self.git_panel.op_status = "Stash push failed".to_string();
                        }
                    }
                    if ui.button("Stash Pop").clicked() {
                        if git_stash_pop(&repo) {
                            self.git_panel.op_status = "Stash pop ok".to_string();
                            self.git_panel.last_refresh = 0.0;
                        } else {
                            self.git_panel.op_status = "Stash pop failed".to_string();
                        }
                    }
                });
                if !self.git_panel.op_status.is_empty() {
                    ui.label(egui::RichText::new(&self.git_panel.op_status).monospace());
                }

                ui.separator();
                ui.label("Changed files");
                egui::ScrollArea::vertical()
                    .max_height(170.0)
                    .show(ui, |ui| {
                        for file in self.git_panel.files.clone() {
                            ui.horizontal(|ui| {
                                let selected = self.git_panel.selected_file.as_deref()
                                    == Some(file.path.as_str());
                                if ui
                                    .selectable_label(
                                        selected,
                                        format!(
                                            "[{}{}] {}",
                                            if file.staged { "S" } else { "U" },
                                            file.status,
                                            file.path
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.git_panel.selected_file = Some(file.path.clone());
                                    self.git_panel.diff_text =
                                        read_git_diff_for_file(&repo, &file.path);
                                }
                                if file.staged {
                                    if ui.small_button("Unstage").clicked() {
                                        let _ = git_unstage_file(&repo, &file.path);
                                        self.git_panel.last_refresh = 0.0;
                                    }
                                } else if ui.small_button("Stage").clicked() {
                                    let _ = git_stage_file(&repo, &file.path);
                                    self.git_panel.last_refresh = 0.0;
                                }
                            });
                        }
                    });

                ui.separator();
                ui.label("Diff");
                egui::ScrollArea::vertical()
                    .max_height(210.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&self.git_panel.diff_text)
                                .monospace()
                                .size(11.0),
                        );
                    });

                ui.separator();
                ui.label("Recent commits");
                egui::ScrollArea::vertical()
                    .max_height(110.0)
                    .show(ui, |ui| {
                        for commit in &self.git_panel.commits {
                            ui.label(
                                egui::RichText::new(format!("{} {}", commit.hash, commit.summary))
                                    .monospace()
                                    .size(11.0),
                            );
                        }
                    });

                ui.separator();
                ui.label("Blame (active line)");
                ui.label(
                    egui::RichText::new(&self.git_panel.blame_text)
                        .monospace()
                        .size(11.0),
                );
            });
    }

    pub(super) fn refresh_sidebar_files(&mut self, ctx: &egui::Context) {
        if !self.show_sidebar {
            return;
        }
        let now = ctx.input(|i| i.time);
        if now - self.sidebar_last_scan < 4.0 {
            return;
        }
        self.sidebar_last_scan = now;
        let output = Command::new("rg")
            .arg("--files")
            .current_dir(self.workspace_root.clone())
            .output();
        let mut files = Vec::new();
        if let Ok(output) = output {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    files.push(PathBuf::from(line));
                }
            }
        }
        self.sidebar_files = files;
    }

    pub(super) fn run_sidebar_search(&mut self) {
        self.sidebar_search_results.clear();
        let query = self.sidebar_search_query.trim();
        if query.is_empty() {
            return;
        }
        let mut cmd = Command::new("rg");
        cmd.arg("-n");
        if self.sidebar_search_case_sensitive {
            cmd.arg("--case-sensitive");
        } else {
            cmd.arg("--smart-case");
        }
        if !self.sidebar_search_regex {
            cmd.arg("--fixed-strings");
        }
        if !self.sidebar_search_include_glob.trim().is_empty() {
            cmd.arg("-g").arg(self.sidebar_search_include_glob.trim());
        }
        if !self.sidebar_search_exclude_glob.trim().is_empty() {
            cmd.arg("-g")
                .arg(format!("!{}", self.sidebar_search_exclude_glob.trim()));
        }
        for pattern in &self.project_gitignore_patterns {
            let p = pattern.trim();
            if !p.is_empty() {
                cmd.arg("-g").arg(format!("!{}", p));
            }
        }
        let output = cmd
            .arg(query)
            .current_dir(self.workspace_root.clone())
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                self.sidebar_search_results = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .take(250)
                    .map(|s| s.to_string())
                    .collect();
            }
        }
    }

    pub(super) fn update_search_preview(&mut self, hit: &str) {
        let Some((path, line_num)) = parse_search_hit_location(hit) else {
            self.sidebar_search_preview.clear();
            return;
        };
        let line_num = line_num.saturating_sub(1);
        let full_path = self.workspace_join(&path);
        let Ok(content) = std::fs::read_to_string(full_path) else {
            self.sidebar_search_preview = "Preview unavailable".to_string();
            return;
        };
        let lines: Vec<&str> = content.lines().collect();
        let start = line_num.saturating_sub(2);
        let end = (line_num + 3).min(lines.len());
        let mut preview = String::new();
        for (idx, line) in lines.iter().enumerate().take(end).skip(start) {
            preview.push_str(&format!("{:>4}  {}\n", idx + 1, line));
        }
        self.sidebar_search_preview = preview;
    }

    pub(super) fn replace_all_search_results(&mut self) {
        self.sidebar_search_message.clear();
        let find = self.sidebar_search_query.trim().to_string();
        let replace = self.sidebar_replace_input.clone();
        if find.is_empty() {
            self.sidebar_search_message = "Search query is empty".to_string();
            return;
        }
        if self.sidebar_search_regex {
            self.sidebar_search_message = "Regex replace is not supported yet".to_string();
            return;
        }
        let mut changed_files = 0usize;
        let mut visited_paths = HashSet::new();
        for hit in self.sidebar_search_results.clone() {
            let Some((path, _line)) = parse_search_hit_location(&hit) else {
                continue;
            };
            let full_path = self.workspace_join(&path);
            if !visited_paths.insert(full_path.clone()) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                if content.contains(&find) {
                    let replaced = content.replace(&find, &replace);
                    if replaced != content && std::fs::write(&full_path, replaced).is_ok() {
                        changed_files += 1;
                    }
                }
            }
        }
        self.sidebar_search_message = format!("Replaced in {} files", changed_files);
        self.run_sidebar_search();
    }

    pub(super) fn run_symbol_search(&mut self) {
        self.sidebar_symbol_results.clear();
        let query = self.sidebar_symbol_query.trim().to_lowercase();
        if query.is_empty() {
            return;
        }
        let mut ranked: Vec<(i32, String)> = Vec::new();
        for file in self.sidebar_files.clone() {
            let full_path = self.workspace_join(file.to_string_lossy().as_ref());
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                for (line_idx, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if !looks_like_symbol_header(trimmed) {
                        continue;
                    }
                    let line_lower = trimmed.to_lowercase();
                    if let Some(score) = symbol_score(&line_lower, &query) {
                        ranked.push((
                            score,
                            format!(
                                "{}:{}: {}",
                                full_path.to_string_lossy(),
                                line_idx + 1,
                                trimmed
                            ),
                        ));
                    }
                }
            }
        }
        ranked.sort_by(|a, b| b.0.cmp(&a.0));
        self.sidebar_symbol_results = ranked.into_iter().take(300).map(|(_, s)| s).collect();
    }

    pub(super) fn open_path_in_tab(&mut self, path: &Path) {
        if let Some((idx, _)) = self
            .editors
            .iter()
            .enumerate()
            .find(|(_, e)| e.file_path.as_deref() == Some(path))
        {
            self.active_tab = idx;
            return;
        }
        if let Ok(mut editor) = Editor::from_file(path.to_path_buf()) {
            self.apply_project_conventions(&mut editor);
            self.apply_language_toolchain_defaults(&editor);
            self.editors.push(editor);
            self.active_tab = self.editors.len().saturating_sub(1);
        }
    }

    pub(super) fn apply_project_conventions(&self, editor: &mut Editor) {
        let editorconfig = self.workspace_root.join(".editorconfig");
        if let Ok(content) = std::fs::read_to_string(editorconfig) {
            for line in content.lines().map(|l| l.trim()) {
                if let Some((k, v)) = line.split_once('=') {
                    let key = k.trim();
                    let val = v.trim();
                    match key {
                        "indent_style" if val.eq_ignore_ascii_case("tab") => {
                            editor.indent_style = crate::editor::IndentStyle::Tabs;
                        }
                        "indent_style" if val.eq_ignore_ascii_case("space") => {
                            editor.indent_style = crate::editor::IndentStyle::Spaces;
                        }
                        "indent_size" => {
                            if let Ok(n) = val.parse::<usize>() {
                                editor.indent_width = n.max(1);
                            }
                        }
                        "end_of_line" if val.eq_ignore_ascii_case("lf") => {
                            editor.line_ending = crate::editor::LineEnding::Lf;
                        }
                        "end_of_line" if val.eq_ignore_ascii_case("crlf") => {
                            editor.line_ending = crate::editor::LineEnding::CrLf;
                        }
                        _ => {}
                    }
                }
            }
        }
        let gitattributes = self.workspace_root.join(".gitattributes");
        if let Ok(content) = std::fs::read_to_string(gitattributes) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('*') && line.contains("eol=lf") {
                    editor.line_ending = crate::editor::LineEnding::Lf;
                }
                if line.starts_with('*') && line.contains("eol=crlf") {
                    editor.line_ending = crate::editor::LineEnding::CrLf;
                }
            }
        }
    }

    pub(super) fn apply_language_toolchain_defaults(&mut self, editor: &Editor) {
        let lang = self.highlighter.syntax_name_for(
            editor.file_path.as_deref(),
            editor.first_line_text().as_deref(),
            editor.syntax_override.as_deref(),
        );
        let formatter = if lang.contains("Rust") {
            "rustfmt"
        } else if lang.contains("Python") {
            "black"
        } else if lang.contains("Go") {
            "gofmt"
        } else if lang.contains("JavaScript")
            || lang.contains("TypeScript")
            || lang.contains("JSON")
            || lang.contains("CSS")
            || lang.contains("HTML")
        {
            "prettier"
        } else if lang.contains("C#") {
            "dotnet-format"
        } else {
            "lsp-default"
        };
        self.formatter_by_language
            .entry(lang)
            .or_insert(formatter.to_string());
    }

    pub(super) fn import_external_settings(&mut self) {
        let Some(path) = rfd::FileDialog::new().pick_file() else {
            return;
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return;
        };
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file.ends_with(".json") {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(size) = value.get("editor.fontSize").and_then(|v| v.as_f64()) {
                    let workspace = self.active_workspace_key();
                    let entry =
                        self.workspace_fonts
                            .entry(workspace)
                            .or_insert(WorkspaceFontSettings {
                                size: 13.5,
                                family: FontFamilyKind::Monospace,
                                ligatures: false,
                            });
                    entry.size = size as f32;
                }
                if let Some(tab) = value.get("editor.tabSize").and_then(|v| v.as_u64()) {
                    self.active_editor().indent_width = tab as usize;
                }
                if let Some(insert_spaces) =
                    value.get("editor.insertSpaces").and_then(|v| v.as_bool())
                {
                    self.active_editor().indent_style = if insert_spaces {
                        crate::editor::IndentStyle::Spaces
                    } else {
                        crate::editor::IndentStyle::Tabs
                    };
                }
                self.packaging_status = "Imported VSCode-style settings".to_string();
                return;
            }
        }
        if file.ends_with(".sublime-settings") {
            if raw.contains("\"tab_size\"") {
                if let Some(n) = raw
                    .split("\"tab_size\"")
                    .nth(1)
                    .and_then(|s| s.split(':').nth(1))
                    .and_then(|s| s.split(',').next())
                    .and_then(|s| s.trim().parse::<usize>().ok())
                {
                    self.active_editor().indent_width = n.max(1);
                }
            }
            self.packaging_status = "Imported Sublime settings".to_string();
            return;
        }
        if file.ends_with(".xml") && raw.contains("JetBrains") {
            if raw.contains("LINE_SEPARATOR value=\"LF\"") {
                self.active_editor().line_ending = crate::editor::LineEnding::Lf;
            }
            if raw.contains("LINE_SEPARATOR value=\"CRLF\"") {
                self.active_editor().line_ending = crate::editor::LineEnding::CrLf;
            }
            self.packaging_status = "Imported JetBrains settings".to_string();
            return;
        }
        self.packaging_status = "Unsupported settings format".to_string();
    }

    pub(super) fn workspace_join(&self, relative_or_abs: &str) -> PathBuf {
        let p = PathBuf::from(relative_or_abs);
        if p.is_absolute() {
            p
        } else {
            self.workspace_root.join(p)
        }
    }

    pub(super) fn create_file_or_folder(&mut self, is_folder: bool) {
        let target = self.file_ops_target.trim();
        if target.is_empty() {
            self.file_ops_message = "Target path is empty".to_string();
            return;
        }
        let full = self.workspace_join(target);
        let result = if is_folder {
            std::fs::create_dir_all(&full)
        } else {
            if let Some(parent) = full.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&full, "")
        };
        self.file_ops_message = match result {
            Ok(_) => {
                self.sidebar_last_scan = 0.0;
                "Created".to_string()
            }
            Err(err) => format!("Create failed: {err}"),
        };
    }

    pub(super) fn duplicate_active_file(&mut self) {
        let Some(src) = self.editors[self.active_tab].file_path.clone() else {
            self.file_ops_message = "No active file".to_string();
            return;
        };
        let target = self.file_ops_target.trim();
        if target.is_empty() {
            self.file_ops_message = "Target path is empty".to_string();
            return;
        }
        let dst = self.workspace_join(target);
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        self.file_ops_message = match std::fs::copy(src, dst) {
            Ok(_) => {
                self.sidebar_last_scan = 0.0;
                "Duplicated".to_string()
            }
            Err(err) => format!("Duplicate failed: {err}"),
        };
    }

    pub(super) fn rename_or_move_active_file(&mut self) {
        let Some(src) = self.editors[self.active_tab].file_path.clone() else {
            self.file_ops_message = "No active file".to_string();
            return;
        };
        let target = self.file_ops_target.trim();
        if target.is_empty() {
            self.file_ops_message = "Target path is empty".to_string();
            return;
        }
        let dst = self.workspace_join(target);
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        self.file_ops_message = match std::fs::rename(&src, &dst) {
            Ok(_) => {
                self.editors[self.active_tab].file_path = Some(dst.clone());
                self.editors[self.active_tab].title = dst
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Untitled")
                    .to_string();
                self.sidebar_last_scan = 0.0;
                "Renamed/Moved".to_string()
            }
            Err(err) => format!("Rename/move failed: {err}"),
        };
    }

    pub(super) fn request_delete_active_file(&mut self) {
        let Some(path) = self.editors[self.active_tab].file_path.clone() else {
            self.file_ops_message = "No active file".to_string();
            return;
        };
        self.pending_delete_confirm = Some(DeletePrompt {
            tab_idx: self.active_tab,
            path,
        });
    }

    pub(super) fn delete_active_file(&mut self, tab_idx: usize, path: &Path) {
        let trash_path = self.trash_path_for(path);
        if let Some(parent) = trash_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let moved = match std::fs::rename(path, &trash_path) {
            Ok(_) => true,
            Err(_) => {
                if std::fs::copy(path, &trash_path).is_ok() {
                    std::fs::remove_file(path).is_ok()
                } else {
                    false
                }
            }
        };

        self.file_ops_message = if moved {
            self.deleted_file_backup = Some((path.to_path_buf(), trash_path));
            self.sidebar_last_scan = 0.0;
            if tab_idx < self.editors.len() {
                if self.editors.len() > 1 {
                    self.force_close_tab(tab_idx);
                } else {
                    self.editors[0] = Editor::new();
                    self.active_tab = 0;
                }
            }
            "Moved to trash (Undo available)".to_string()
        } else {
            "Delete failed: unable to move to trash".to_string()
        };
    }

    pub(super) fn undo_last_deleted_file(&mut self) {
        let Some((path, trash_path)) = self.deleted_file_backup.take() else {
            self.file_ops_message = "Nothing to undo".to_string();
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let restored = match std::fs::rename(&trash_path, &path) {
            Ok(_) => true,
            Err(_) => {
                if std::fs::copy(&trash_path, &path).is_ok() {
                    let _ = std::fs::remove_file(&trash_path);
                    true
                } else {
                    false
                }
            }
        };
        if restored {
            self.sidebar_last_scan = 0.0;
            self.file_ops_message = format!("Restored {}", path.to_string_lossy());
            self.open_path_in_tab(path.as_path());
        } else {
            self.file_ops_message = "Undo delete failed".to_string();
            self.deleted_file_backup = Some((path, trash_path));
        }
    }

    pub(super) fn trash_path_for(&self, path: &Path) -> PathBuf {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("deleted-file");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        self.workspace_root
            .join(".lux")
            .join("trash")
            .join(format!("{stamp}-{name}"))
    }

    pub(super) fn sidebar_tab_title(tab: SidebarTab) -> &'static str {
        match tab {
            SidebarTab::Explorer => "Explorer",
            SidebarTab::Search => "Search",
            SidebarTab::Git => "Source Control",
            SidebarTab::Debug => "Run and Debug",
            SidebarTab::Collab => "Collab",
        }
    }

    pub(super) fn sidebar_tab_hint(tab: SidebarTab) -> &'static str {
        match tab {
            SidebarTab::Explorer => "Ctrl+Alt+1",
            SidebarTab::Search => "Ctrl+Alt+2",
            SidebarTab::Git => "Ctrl+Alt+3",
            SidebarTab::Debug => "Ctrl+Alt+4",
            SidebarTab::Collab => "Ctrl+Alt+5",
        }
    }
}
