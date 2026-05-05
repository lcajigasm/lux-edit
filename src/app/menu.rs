use eframe::egui;
use super::*;

impl LuxApp {
    pub(super) fn show_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("lux_menu_bar")
            .frame(
                egui::Frame::none()
                    .fill(MENU_BG)
                    .stroke(MENU_STROKE)
                    .inner_margin(egui::Margin::symmetric(12.0, 6.0)),
            )
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    let rich_label = |label: &str| {
                        egui::RichText::new(label)
                            .color(egui::Color32::from_rgb(215, 215, 215))
                            .size(13.0)
                    };

                    ui.menu_button(rich_label("File"), |ui| {
                        if ui.button("New Tab\tCtrl+N").clicked() {
                            self.new_tab();
                            ui.close_menu();
                        }
                        if ui.button("Open...\tCtrl+O").clicked() {
                            self.open_file();
                            ui.close_menu();
                        }
                        if ui.button("Save\tCtrl+S").clicked() {
                            self.save_file();
                            ui.close_menu();
                        }
                        if ui.button("Save As...\tCtrl+Shift+S").clicked() {
                            self.save_file_as();
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.menu_button("New from Template", |ui| {
                            if ui.button("Web App").clicked() {
                                self.create_from_template("web");
                                ui.close_menu();
                            }
                            if ui.button("CLI").clicked() {
                                self.create_from_template("cli");
                                ui.close_menu();
                            }
                            if ui.button("Library").clicked() {
                                self.create_from_template("library");
                                ui.close_menu();
                            }
                            if ui.button("Doc Site").clicked() {
                                self.create_from_template("docs");
                                ui.close_menu();
                            }
                        });
                        if ui.button("Clone Repository...").clicked() {
                            self.show_clone_repo_dialog = true;
                            ui.close_menu();
                        }
                        if !self.recent_workspaces.is_empty() {
                            ui.separator();
                            ui.label("Recent Workspaces");
                            for workspace in self.recent_workspaces.clone() {
                                let label = workspace.to_string_lossy().to_string();
                                if ui.button(label).clicked() {
                                    self.workspace_root = workspace;
                                    ui.close_menu();
                                }
                            }
                        }
                        ui.separator();
                        if ui.button("Close Tab\tCtrl+W").clicked() {
                            self.close_tab();
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("Edit"), |ui| {
                        if ui.button("Undo\tCtrl+Z").clicked() {
                            self.active_editor().undo();
                            ui.close_menu();
                        }
                        if ui.button("Redo\tCtrl+Shift+Z").clicked() {
                            self.active_editor().redo();
                            ui.close_menu();
                        }
                        if ui.button("Format Document\tCtrl+Shift+F").clicked() {
                            self.active_editor().request_formatting = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Cut\tCtrl+X").clicked() {
                            self.cut();
                            ui.close_menu();
                        }
                        if ui.button("Copy\tCtrl+C").clicked() {
                            self.copy();
                            ui.close_menu();
                        }
                        if ui.button("Paste\tCtrl+V").clicked() {
                            self.paste();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Select All\tCtrl+A").clicked() {
                            self.active_editor().select_all();
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("Refactor"), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.refactor_name_input)
                                .hint_text("Name (for rename/extract)"),
                        );
                        if ui.button("Rename Symbol (Document)").clicked() {
                            let tab_idx = self.active_tab;
                            let before = self.editors[tab_idx].rope.to_string();
                            let refactor_name = self.refactor_name_input.trim().to_string();
                            let from = {
                                let editor = &self.editors[self.active_tab];
                                let selected = editor.selected_text();
                                if selected.trim().is_empty() {
                                    editor.symbol_at_primary_cursor()
                                } else {
                                    selected
                                }
                            };
                            let ok = self
                                .active_editor()
                                .rename_symbol_in_document(from.trim(), &refactor_name);
                            self.refactor_status = if ok {
                                self.capture_refactor_preview(tab_idx, "Rename Symbol", before);
                                "Rename applied".to_string()
                            } else {
                                "Rename not applied".to_string()
                            };
                            ui.close_menu();
                        }
                        if ui.button("Extract Variable").clicked() {
                            let tab_idx = self.active_tab;
                            let before = self.editors[tab_idx].rope.to_string();
                            let refactor_name = self.refactor_name_input.trim().to_string();
                            let ok = self.active_editor().extract_variable(&refactor_name);
                            self.refactor_status = if ok {
                                self.capture_refactor_preview(tab_idx, "Extract Variable", before);
                                "Extract variable applied".to_string()
                            } else {
                                "Select expression and provide name".to_string()
                            };
                            ui.close_menu();
                        }
                        if ui.button("Extract Method").clicked() {
                            let tab_idx = self.active_tab;
                            let before = self.editors[tab_idx].rope.to_string();
                            let refactor_name = self.refactor_name_input.trim().to_string();
                            let ok = self.active_editor().extract_method(&refactor_name);
                            self.refactor_status = if ok {
                                self.capture_refactor_preview(tab_idx, "Extract Method", before);
                                "Extract method applied".to_string()
                            } else {
                                "Select block and provide name".to_string()
                            };
                            ui.close_menu();
                        }
                        if ui.button("Inline Variable").clicked() {
                            let tab_idx = self.active_tab;
                            let before = self.editors[tab_idx].rope.to_string();
                            let ok = self.active_editor().inline_variable_at_cursor();
                            self.refactor_status = if ok {
                                self.capture_refactor_preview(tab_idx, "Inline Variable", before);
                                "Inline variable applied".to_string()
                            } else {
                                "Place cursor on `let name = expr;`".to_string()
                            };
                            ui.close_menu();
                        }
                        if ui.button("Organize Imports").clicked() {
                            let tab_idx = self.active_tab;
                            let before = self.editors[tab_idx].rope.to_string();
                            let ok = self.active_editor().organize_imports();
                            self.refactor_status = if ok {
                                self.capture_refactor_preview(tab_idx, "Organize Imports", before);
                                "Imports organized".to_string()
                            } else {
                                "No imports to organize".to_string()
                            };
                            ui.close_menu();
                        }
                        if !self.refactor_status.is_empty() {
                            ui.separator();
                            ui.label(self.refactor_status.clone());
                        }
                    });

                    ui.menu_button(rich_label("Find"), |ui| {
                        if ui.button("Find\tCtrl+F").clicked() {
                            self.show_search = true;
                            self.show_replace = false;
                            ui.close_menu();
                        }
                        if ui.button("Replace\tCtrl+H").clicked() {
                            self.show_search = true;
                            self.show_replace = true;
                            ui.close_menu();
                        }
                        if ui.button("Go to Line\tCtrl+G").clicked() {
                            self.show_goto_line = true;
                            self.show_search = false;
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("View"), |ui| {
                        if ui.button("Command Palette\tCtrl+Shift+P").clicked() {
                            self.command_palette.toggle();
                            ui.close_menu();
                        }
                        if ui.button("Toggle Search Panel").clicked() {
                            self.show_search = !self.show_search;
                            ui.close_menu();
                        }
                        if ui.selectable_label(self.show_sidebar, "Sidebar").clicked() {
                            self.show_sidebar = !self.show_sidebar;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(self.show_dock_panel, "Terminal/Output Panel")
                            .clicked()
                        {
                            self.show_dock_panel = !self.show_dock_panel;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui
                            .selectable_label(
                                self.split_mode == SplitMode::Vertical,
                                "Split Vertical",
                            )
                            .clicked()
                        {
                            self.split_mode = if self.split_mode == SplitMode::Vertical {
                                SplitMode::None
                            } else {
                                SplitMode::Vertical
                            };
                            self.ensure_split_secondary();
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.split_mode == SplitMode::Horizontal,
                                "Split Horizontal",
                            )
                            .clicked()
                        {
                            self.split_mode = if self.split_mode == SplitMode::Horizontal {
                                SplitMode::None
                            } else {
                                SplitMode::Horizontal
                            };
                            self.ensure_split_secondary();
                            ui.close_menu();
                        }
                        if ui.selectable_label(self.zen_mode, "Zen Mode").clicked() {
                            self.zen_mode = !self.zen_mode;
                            if self.zen_mode {
                                self.focus_mode = true;
                                self.show_sidebar = false;
                                self.show_git_panel = false;
                            }
                            ui.close_menu();
                        }
                        if ui.selectable_label(self.focus_mode, "Focus Mode").clicked() {
                            self.focus_mode = !self.focus_mode;
                            if self.focus_mode {
                                self.show_sidebar = false;
                                self.show_git_panel = false;
                            }
                            ui.close_menu();
                        }
                        if ui.button("Toggle Go To Line").clicked() {
                            self.show_goto_line = !self.show_goto_line;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(self.show_git_panel, "Git Panel")
                            .clicked()
                        {
                            self.show_git_panel = !self.show_git_panel;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Preset: Coding").clicked() {
                            self.show_sidebar = true;
                            self.show_git_panel = true;
                            self.split_mode = SplitMode::Vertical;
                            self.zen_mode = false;
                            self.focus_mode = false;
                            self.ensure_split_secondary();
                            ui.close_menu();
                        }
                        if ui.button("Preset: Writing").clicked() {
                            self.show_sidebar = false;
                            self.show_git_panel = false;
                            self.split_mode = SplitMode::None;
                            self.zen_mode = true;
                            self.focus_mode = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        let editor_idx = self.active_tab;
                        let (is_markdown, preview_on) = {
                            let editor = &self.editors[editor_idx];
                            (editor.is_markdown(), editor.markdown_preview)
                        };
                        if is_markdown
                            && ui
                                .selectable_label(preview_on, "Markdown Preview")
                                .clicked()
                        {
                            self.editors[editor_idx].markdown_preview = !preview_on;
                            ui.close_menu();
                        }
                        let minimap_enabled = self.active_editor().minimap_enabled;
                        if ui
                            .selectable_label(minimap_enabled, "Toggle Minimap")
                            .clicked()
                        {
                            self.active_editor().minimap_enabled = !minimap_enabled;
                            ui.close_menu();
                        }
                        if ui.button("Minimap Width +").clicked() {
                            let editor = self.active_editor();
                            editor.minimap_width = (editor.minimap_width + 10.0).clamp(80.0, 200.0);
                            ui.close_menu();
                        }
                        if ui.button("Minimap Width -").clicked() {
                            let editor = self.active_editor();
                            editor.minimap_width = (editor.minimap_width - 10.0).clamp(80.0, 200.0);
                            ui.close_menu();
                        }
                        if ui.button("Minimap Opacity +").clicked() {
                            let editor = self.active_editor();
                            editor.minimap_opacity = (editor.minimap_opacity + 0.1).clamp(0.2, 1.0);
                            ui.close_menu();
                        }
                        if ui.button("Minimap Opacity -").clicked() {
                            let editor = self.active_editor();
                            editor.minimap_opacity = (editor.minimap_opacity - 0.1).clamp(0.2, 1.0);
                            ui.close_menu();
                        }
                    });

                    ui.menu_button(rich_label("Theme"), |ui| {
                        for theme_option in [
                            EditorThemeKind::Dark,
                            EditorThemeKind::Light,
                            EditorThemeKind::Monokai,
                            EditorThemeKind::SolarizedDark,
                            EditorThemeKind::HighContrast,
                        ] {
                            let selected = self.editor_theme == theme_option;
                            let label = theme_option.name();
                            if ui.selectable_label(selected, label).clicked() {
                                self.editor_theme = theme_option;
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("Export Theme JSON").clicked() {
                            self.export_theme_json();
                            ui.close_menu();
                        }
                        if ui.button("Import Theme JSON").clicked() {
                            self.import_theme_json();
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.label("UI Density");
                        ui.add(
                            egui::Slider::new(&mut self.theme_ui_density, 0.85..=1.35)
                                .text("scale"),
                        );
                        if ui.button("Font Preset: Small").clicked() {
                            let workspace = self.active_workspace_key();
                            let entry = self.workspace_fonts.entry(workspace).or_insert(
                                WorkspaceFontSettings {
                                    size: 13.5,
                                    family: FontFamilyKind::Monospace,
                                    ligatures: false,
                                },
                            );
                            entry.size = 12.0;
                        }
                        if ui.button("Font Preset: Medium").clicked() {
                            let workspace = self.active_workspace_key();
                            let entry = self.workspace_fonts.entry(workspace).or_insert(
                                WorkspaceFontSettings {
                                    size: 13.5,
                                    family: FontFamilyKind::Monospace,
                                    ligatures: false,
                                },
                            );
                            entry.size = 14.0;
                        }
                        if ui.button("Font Preset: Large").clicked() {
                            let workspace = self.active_workspace_key();
                            let entry = self.workspace_fonts.entry(workspace).or_insert(
                                WorkspaceFontSettings {
                                    size: 13.5,
                                    family: FontFamilyKind::Monospace,
                                    ligatures: false,
                                },
                            );
                            entry.size = 16.0;
                        }
                        ui.separator();
                        ui.label("Color Overrides");
                        let mut bg = self
                            .theme_override_bg
                            .unwrap_or(egui::Color32::from_rgb(39, 40, 34));
                        let mut text = self
                            .theme_override_text
                            .unwrap_or(egui::Color32::from_rgb(248, 248, 242));
                        ui.horizontal(|ui| {
                            ui.label("Background");
                            ui.color_edit_button_srgba(&mut bg);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Text");
                            ui.color_edit_button_srgba(&mut text);
                        });
                        self.theme_override_bg = Some(bg);
                        self.theme_override_text = Some(text);
                        if ui.button("Clear Overrides").clicked() {
                            self.theme_override_bg = None;
                            self.theme_override_text = None;
                        }
                        ui.separator();
                        ui.label("Workspace Font");
                        let workspace_root = self.workspace_root.clone();
                        let workspace = self.active_workspace_key();
                        let entry = self.workspace_fonts.entry(workspace).or_insert(
                            WorkspaceFontSettings {
                                size: 13.5,
                                family: FontFamilyKind::Monospace,
                                ligatures: false,
                            },
                        );
                        ui.add(egui::Slider::new(&mut entry.size, 10.0..=24.0).text("Font Size"));
                        ui.horizontal(|ui| {
                            ui.label("Family");
                            ui.selectable_value(
                                &mut entry.family,
                                FontFamilyKind::Monospace,
                                "Monospace",
                            );
                            ui.selectable_value(
                                &mut entry.family,
                                FontFamilyKind::Proportional,
                                "Proportional",
                            );
                        });
                        ui.checkbox(&mut entry.ligatures, "Ligatures");
                        ui.separator();
                        ui.label("Per-folder override");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.folder_override_path_input)
                                .hint_text("folder path (relative or absolute)"),
                        );
                        if ui.button("Apply override from current font").clicked() {
                            let relative = PathBuf::from(self.folder_override_path_input.trim());
                            let folder = if relative.is_absolute() {
                                relative
                            } else {
                                workspace_root.join(relative)
                            };
                            self.folder_font_overrides.insert(folder, entry.clone());
                        }
                        if ui.button("Remove override").clicked() {
                            let relative = PathBuf::from(self.folder_override_path_input.trim());
                            let folder = if relative.is_absolute() {
                                relative
                            } else {
                                workspace_root.join(relative)
                            };
                            self.folder_font_overrides.remove(&folder);
                        }
                        if ui.button("Save Workspace Settings").clicked() {
                            self.save_workspace_settings();
                        }
                    });

                    ui.menu_button(rich_label("Keymap"), |ui| {
                        ui.label("Preset");
                        ui.selectable_value(
                            &mut self.keymap_preset,
                            KeymapPreset::Vscode,
                            "VSCode",
                        );
                        ui.selectable_value(
                            &mut self.keymap_preset,
                            KeymapPreset::Sublime,
                            "Sublime",
                        );
                        ui.selectable_value(
                            &mut self.keymap_preset,
                            KeymapPreset::JetBrains,
                            "JetBrains",
                        );
                        ui.separator();
                        ui.label("Custom bindings (e.g. Cmd+Shift+P)");
                        ui.horizontal(|ui| {
                            ui.label("Palette");
                            ui.text_edit_singleline(&mut self.binding_palette);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Open");
                            ui.text_edit_singleline(&mut self.binding_open);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Save");
                            ui.text_edit_singleline(&mut self.binding_save);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Find");
                            ui.text_edit_singleline(&mut self.binding_find);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Format");
                            ui.text_edit_singleline(&mut self.binding_format);
                        });
                        if ui.button("Apply Custom Bindings").clicked() {
                            let palette = self.binding_palette.clone();
                            let open = self.binding_open.clone();
                            let save = self.binding_save.clone();
                            let find = self.binding_find.clone();
                            let format = self.binding_format.clone();
                            self.apply_custom_binding("palette", &palette);
                            self.apply_custom_binding("open", &open);
                            self.apply_custom_binding("save", &save);
                            self.apply_custom_binding("find", &find);
                            self.apply_custom_binding("format", &format);
                        }
                        if ui.button("Clear Custom Bindings").clicked() {
                            self.custom_shortcuts.clear();
                        }
                    });

                    ui.menu_button(rich_label("Platform"), |ui| {
                        ui.checkbox(
                            &mut self.telemetry_opt_in,
                            "Telemetry Opt-in (explicit consent)",
                        );
                        ui.checkbox(&mut self.plugin_sandbox_enabled, "Plugin Sandbox Enabled");
                        ui.label(format!(
                            "Safe Mode: {}",
                            if self.safe_mode {
                                "ON (untrusted workspace)"
                            } else {
                                "OFF"
                            }
                        ));
                        ui.label(format!("Loaded plugins: {}", self.plugins.len()));
                        let syntax_count: usize =
                            self.plugins.iter().map(|p| p.syntax_packages.len()).sum();
                        let formatter_count: usize =
                            self.plugins.iter().map(|p| p.formatters.len()).sum();
                        let task_count: usize = self.plugins.iter().map(|p| p.tasks.len()).sum();
                        let keymap_count: usize =
                            self.plugins.iter().map(|p| p.keymaps.len()).sum();
                        let script_count: usize =
                            self.plugins.iter().map(|p| p.scripts.len()).sum();
                        ui.label(format!(
                            "Contribs syntax:{} formatters:{} tasks:{} keymaps:{} scripts:{}",
                            syntax_count, formatter_count, task_count, keymap_count, script_count
                        ));
                        ui.separator();
                        ui.label("Marketplace / Registry");
                        if ui.button("Reload Registry").clicked() {
                            self.registry_entries =
                                crate::plugin::load_registry(&self.workspace_root);
                        }
                        for entry in self.registry_entries.clone() {
                            ui.horizontal(|ui| {
                                ui.label(format!("{} ({})", entry.name, entry.id));
                                if ui.button("Install/Update").clicked() {
                                    self.marketplace_status =
                                        match crate::plugin::install_or_update_registry_plugin(
                                            &self.workspace_root,
                                            &entry,
                                        ) {
                                            Ok(_) => "Plugin installed/updated".to_string(),
                                            Err(err) => format!("Install failed: {err}"),
                                        };
                                    self.plugins =
                                        crate::plugin::load_plugin_manifests(&self.workspace_root);
                                    for plugin in &self.plugins {
                                        let tuples: Vec<(String, String, String)> = plugin
                                            .commands
                                            .iter()
                                            .map(|cmd| {
                                                (
                                                    cmd.title.clone(),
                                                    cmd.shortcut.clone(),
                                                    cmd.id.clone(),
                                                )
                                            })
                                            .collect();
                                        self.command_palette.register_extension_commands(
                                            plugin.name.as_str(),
                                            tuples.iter().map(|(title, shortcut, id)| {
                                                (title.as_str(), shortcut.as_str(), id.as_str())
                                            }),
                                        );
                                    }
                                }
                            });
                        }
                        if !self.marketplace_status.is_empty() {
                            ui.label(self.marketplace_status.clone());
                        }
                        ui.separator();
                        ui.label("Update Channel");
                        ui.selectable_value(
                            &mut self.update_channel,
                            UpdateChannel::Stable,
                            "Stable",
                        );
                        ui.selectable_value(&mut self.update_channel, UpdateChannel::Beta, "Beta");
                        ui.selectable_value(
                            &mut self.update_channel,
                            UpdateChannel::Nightly,
                            "Nightly",
                        );
                        ui.separator();
                        ui.checkbox(&mut self.portable_mode, "Portable Mode");
                        if ui.button("Build Release Artifacts").clicked() {
                            self.build_release_artifacts();
                        }
                        if ui.button("Check Updates").clicked() {
                            self.check_update_channel();
                        }
                        if ui.button("Export App Config").clicked() {
                            self.export_app_config();
                        }
                        if ui.button("Import App Config").clicked() {
                            self.import_app_config();
                        }
                        if ui.button("Import External Settings").clicked() {
                            self.import_external_settings();
                        }
                        if !self.packaging_status.is_empty() {
                            ui.label(self.packaging_status.clone());
                        }
                        ui.separator();
                        ui.label("Settings & Sync");
                        ui.checkbox(&mut self.settings_sync_enabled, "Enable settings sync");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings_sync_path)
                                .hint_text("sync file path"),
                        );
                        ui.horizontal(|ui| {
                            ui.label("Project profile");
                            ui.text_edit_singleline(&mut self.settings_profile);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Role profile");
                            ui.text_edit_singleline(&mut self.settings_role_profile);
                        });
                        if ui.button("Sync Now").clicked() {
                            self.sync_settings_now();
                        }
                        if !self.settings_sync_status.is_empty() {
                            ui.label(self.settings_sync_status.clone());
                        }
                        ui.separator();
                        ui.label("Secure Secrets Storage");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.secret_key_input)
                                .hint_text("secret key"),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.secret_value_input)
                                .password(true)
                                .hint_text("secret value"),
                        );
                        if ui.button("Store Secret").clicked() {
                            self.store_secret_securely();
                        }
                        if !self.secrets_status.is_empty() {
                            ui.label(self.secrets_status.clone());
                        }
                        ui.separator();
                        ui.label("Observability");
                        ui.horizontal(|ui| {
                            ui.label("Level");
                            ui.selectable_value(
                                &mut self.observability_level,
                                LogLevel::Error,
                                "Error",
                            );
                            ui.selectable_value(
                                &mut self.observability_level,
                                LogLevel::Warn,
                                "Warn",
                            );
                            ui.selectable_value(
                                &mut self.observability_level,
                                LogLevel::Info,
                                "Info",
                            );
                            ui.selectable_value(
                                &mut self.observability_level,
                                LogLevel::Debug,
                                "Debug",
                            );
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut self.observability_module_filter)
                                .hint_text("module filter"),
                        );
                        if ui.button("Run Health Checks").clicked() {
                            self.run_health_checks();
                        }
                        if ui.button("Export Diagnostics Bundle").clicked() {
                            self.export_diagnostics_bundle();
                        }
                        if !self.observability_health_status.is_empty() {
                            ui.label(self.observability_health_status.clone());
                        }
                    });

                    ui.menu_button(rich_label("Diagnostics"), |ui| {
                        ui.checkbox(&mut self.diagnostics_show_error, "Show Errors");
                        ui.checkbox(&mut self.diagnostics_show_warning, "Show Warnings");
                        ui.checkbox(&mut self.diagnostics_show_info, "Show Info");
                        ui.separator();
                        let active_lang = self.active_language_label();
                        let enabled = self
                            .diagnostics_language_enabled
                            .entry(active_lang.clone())
                            .or_insert(true);
                        ui.checkbox(enabled, format!("Diagnostics enabled for {}", active_lang));
                        ui.separator();
                        ui.checkbox(&mut self.format_on_save, "Format On Save");
                        ui.checkbox(&mut self.format_on_type, "Format On Type");
                        let formatter = self
                            .formatter_by_language
                            .entry(active_lang.clone())
                            .or_insert("lsp-default".to_string());
                        ui.horizontal(|ui| {
                            ui.label("Formatter");
                            ui.text_edit_singleline(formatter);
                        });
                        ui.separator();
                        ui.label("Lint Workspace Override");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.lint_override_rule_input)
                                    .hint_text("rule"),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.lint_override_value_input)
                                    .hint_text("value"),
                            );
                        });
                        if ui.button("Set Workspace Override").clicked() {
                            let rule = self.lint_override_rule_input.trim().to_string();
                            let value = self.lint_override_value_input.trim().to_string();
                            if !rule.is_empty() && !value.is_empty() {
                                self.lint_workspace_overrides.insert(rule, value);
                            }
                        }
                        if ui
                            .button("Set Folder Override (active file folder)")
                            .clicked()
                        {
                            let rule = self.lint_override_rule_input.trim().to_string();
                            let value = self.lint_override_value_input.trim().to_string();
                            if !rule.is_empty() && !value.is_empty() {
                                if let Some(folder) = self
                                    .editors
                                    .get(self.active_tab)
                                    .and_then(|e| e.file_path.as_ref())
                                    .and_then(|p| p.parent())
                                    .map(Path::to_path_buf)
                                {
                                    self.lint_folder_overrides
                                        .entry(folder)
                                        .or_default()
                                        .insert(rule, value);
                                }
                            }
                        }
                    });

                    ui.menu_button(rich_label("Help"), |ui| {
                        if ui.button("Shortcut Cheat Sheet").clicked() {
                            self.show_help_window = true;
                            ui.close_menu();
                        }
                        if ui.button("Interactive Onboarding").clicked() {
                            self.show_onboarding = true;
                            self.onboarding_step = 0;
                            ui.close_menu();
                        }
                        if ui.button("Troubleshooting").clicked() {
                            self.show_troubleshooting = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Run UI Test Harness").clicked() {
                            self.run_ui_test_harness();
                        }
                        if ui.button("Run Golden Snapshot Check").clicked() {
                            self.run_golden_snapshot_check();
                        }
                        if ui.button("Run Performance Benchmark").clicked() {
                            self.run_performance_benchmark();
                        }
                        if ui.button("Export Crash Triage Bundle").clicked() {
                            self.export_crash_triage_bundle();
                        }
                        if !self.qa_status.is_empty() {
                            ui.label(self.qa_status.clone());
                        }
                        ui.separator();
                        ui.label("Internationalization");
                        ui.horizontal(|ui| {
                            ui.label("Locale");
                            ui.text_edit_singleline(&mut self.locale_code);
                        });
                        ui.checkbox(&mut self.rtl_layout, "RTL layout");
                        ui.horizontal(|ui| {
                            ui.label("IME test");
                            ui.text_edit_singleline(&mut self.ime_test_input);
                        });
                    });
                });
            });
    }
}
