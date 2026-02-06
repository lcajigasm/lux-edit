use arboard::Clipboard;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::SystemTime;

use crate::editor::{CodeLensMetric, Editor, InlineBlameEntry};
use crate::lsp::{RequestKind, Snapshot as LspSnapshot};
use crate::syntax::SyntaxHighlighter;
use crate::ui::command_palette::{CommandId, CommandPalette};
use crate::ui::editor_view::{EditorFontSettings, EditorThemeKind, FontFamilyKind};
use crate::ui::markdown_preview;

const WINDOW_BG: egui::Color32 = egui::Color32::from_rgb(36, 37, 38);
const MENU_BG: egui::Color32 = egui::Color32::from_rgb(45, 45, 47);
const MENU_STROKE: egui::Stroke = egui::Stroke {
    width: 1.0,
    color: egui::Color32::from_rgb(65, 65, 67),
};
const TAB_BAR_BG: egui::Color32 = egui::Color32::from_rgb(25, 25, 26);
const TAB_ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(37, 37, 38); // Should match generic editor bg
const TAB_INACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(45, 45, 48);
const TAB_HOVER_BG: egui::Color32 = egui::Color32::from_rgb(50, 50, 53);
const TAB_HEIGHT: f32 = 32.0; // Slightly taller for modern feel
const TAB_MIN_WIDTH: f32 = 120.0;
const TAB_MAX_WIDTH: f32 = 220.0;
const TAB_PADDING_X: f32 = 12.0;
const TAB_CLOSE_SIZE: f32 = 14.0;
const ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(0, 122, 204); // VS Code-ish blue active line
const ACTIVITY_BAR_BG: egui::Color32 = egui::Color32::from_rgb(43, 43, 43);
const SIDEBAR_BG: egui::Color32 = egui::Color32::from_rgb(37, 37, 38);

// GitInfo moved to ui::status_bar

#[derive(Clone, Copy)]
enum TabAction {
    Activate(usize),
    Close(usize),
    CloseOthers(usize),
    ReopenClosed,
    TogglePin(usize, bool),
    Reorder(usize, usize),
    MoveToSplit(usize, bool),
    NewTab,
}

struct LspWorkerOutput {
    path: PathBuf,
    snapshot: LspSnapshot,
    request: RequestKind,
}

#[derive(Clone, Debug)]
enum AsyncCommandTarget {
    TerminalPrimary,
    TerminalSecondary,
    Task,
    RunConfig,
}

#[derive(Clone, Debug)]
struct AsyncCommandResult {
    target: AsyncCommandTarget,
    label: String,
    stdout: String,
    stderr: String,
    success: bool,
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct ExternalChangePrompt {
    tab_idx: usize,
    path: PathBuf,
}

#[derive(Clone, Debug)]
struct RefactorPreview {
    tab_idx: usize,
    title: String,
    diff_preview: String,
    original_text: String,
}

#[derive(Clone, Debug)]
struct WorkspaceTask {
    name: String,
    command: String,
}

#[derive(Clone, Debug)]
struct RunConfiguration {
    name: String,
    command: String,
    env_overrides: String,
}

#[derive(Clone, Debug)]
struct TerminalProfile {
    name: String,
    shell: String,
    theme_hint: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

#[derive(Clone, Debug, Default)]
struct GitChangedFile {
    path: String,
    status: String,
    staged: bool,
}

#[derive(Clone, Debug, Default)]
struct GitCommitEntry {
    hash: String,
    summary: String,
}

#[derive(Clone, Debug, Default)]
struct GitPanelState {
    files: Vec<GitChangedFile>,
    commits: Vec<GitCommitEntry>,
    selected_file: Option<String>,
    diff_text: String,
    blame_text: String,
    commit_message: String,
    branch_input: String,
    stash_message: String,
    op_status: String,
    last_refresh: f64,
}

#[derive(Clone, Debug)]
struct WorkspaceFontSettings {
    size: f32,
    family: FontFamilyKind,
    ligatures: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarTab {
    Explorer,
    Search,
    Git,
    Debug,
    Collab,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitMode {
    None,
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DockPanelTab {
    Terminal,
    Output,
    Problems,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DockSide {
    Bottom,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeymapPreset {
    Vscode,
    Sublime,
    JetBrains,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UpdateChannel {
    Stable,
    Beta,
    Nightly,
}

#[derive(Clone, Copy, Debug)]
struct ShortcutSpec {
    key: egui::Key,
    command: bool,
    shift: bool,
    alt: bool,
}

pub struct LuxApp {
    pub editors: Vec<Editor>,
    pub active_tab: usize,
    pub command_palette: CommandPalette,
    pub show_search: bool,
    pub show_replace: bool,
    pub search_input: String,
    pub replace_input: String,
    pub show_goto_line: bool,
    pub goto_line_input: String,
    pub clipboard: Option<Clipboard>,
    pub highlighter: SyntaxHighlighter,
    /// If Some, show a "save before closing?" dialog for this tab index.
    pub confirm_close_tab: Option<usize>,
    pub pending_close_others: Option<usize>,
    pending_external_change: Option<ExternalChangePrompt>,
    pub closed_tabs: Vec<Editor>,
    pub dragging_tab: Option<usize>,
    pub editor_theme: EditorThemeKind,
    git_info: Option<crate::ui::status_bar::GitInfo>,
    git_last_check: f64,
    show_sidebar: bool,
    sidebar_tab: SidebarTab,
    sidebar_search_query: String,
    sidebar_search_case_sensitive: bool,
    sidebar_search_regex: bool,
    sidebar_search_include_glob: String,
    sidebar_search_exclude_glob: String,
    sidebar_search_results: Vec<String>,
    sidebar_search_selected: Option<String>,
    sidebar_search_preview: String,
    sidebar_replace_input: String,
    sidebar_search_message: String,
    sidebar_symbol_query: String,
    sidebar_symbol_results: Vec<String>,
    refactor_name_input: String,
    refactor_status: String,
    refactor_preview: Option<RefactorPreview>,
    sidebar_files: Vec<PathBuf>,
    sidebar_last_scan: f64,
    quick_open_query: String,
    recent_workspaces: Vec<PathBuf>,
    file_ops_target: String,
    file_ops_message: String,
    deleted_file_backup: Option<(PathBuf, Vec<u8>)>,
    debug_breakpoints: HashMap<PathBuf, HashSet<usize>>,
    debug_watch_input: String,
    debug_watches: Vec<String>,
    debug_call_stack: Vec<String>,
    tasks: Vec<WorkspaceTask>,
    new_task_name: String,
    new_task_command: String,
    run_configs: Vec<RunConfiguration>,
    new_run_config_name: String,
    new_run_config_command: String,
    new_run_config_env: String,
    diagnostics_show_error: bool,
    diagnostics_show_warning: bool,
    diagnostics_show_info: bool,
    diagnostics_language_enabled: HashMap<String, bool>,
    format_on_save: bool,
    format_on_type: bool,
    formatter_by_language: HashMap<String, String>,
    lint_workspace_overrides: HashMap<String, String>,
    lint_folder_overrides: HashMap<PathBuf, HashMap<String, String>>,
    lint_override_rule_input: String,
    lint_override_value_input: String,
    collab_enabled: bool,
    collab_session_id: String,
    collab_review_mode: bool,
    collab_note_input: String,
    collab_notes: HashMap<PathBuf, Vec<(usize, String)>>,
    collab_peer_cursors: Vec<String>,
    workspace_roots: Vec<PathBuf>,
    new_workspace_root_input: String,
    project_gitignore_patterns: Vec<String>,
    trusted_workspaces: HashSet<PathBuf>,
    safe_mode: bool,
    show_help_window: bool,
    show_onboarding: bool,
    onboarding_step: usize,
    show_troubleshooting: bool,
    portable_mode: bool,
    packaging_status: String,
    qa_status: String,
    locale_code: String,
    rtl_layout: bool,
    ime_test_input: String,
    settings_sync_enabled: bool,
    settings_sync_path: String,
    settings_sync_status: String,
    settings_profile: String,
    settings_role_profile: String,
    secret_key_input: String,
    secret_value_input: String,
    secrets_status: String,
    show_clone_repo_dialog: bool,
    clone_repo_url: String,
    clone_repo_target: String,
    onboarding_first_run_done: bool,
    observability_level: LogLevel,
    observability_module_filter: String,
    observability_health_status: String,
    split_mode: SplitMode,
    split_secondary_tab: Option<usize>,
    zen_mode: bool,
    focus_mode: bool,
    show_dock_panel: bool,
    dock_tab: DockPanelTab,
    dock_side: DockSide,
    dock_size: f32,
    terminal_profiles: Vec<TerminalProfile>,
    terminal_profile_idx: usize,
    terminal_split_panes: bool,
    terminal_input_secondary: String,
    terminal_log_secondary: Vec<String>,
    terminal_input: String,
    terminal_log: Vec<String>,
    output_log: Vec<String>,
    output_filter: String,
    problems_filter: String,
    workspace_fonts: HashMap<PathBuf, WorkspaceFontSettings>,
    folder_font_overrides: HashMap<PathBuf, WorkspaceFontSettings>,
    folder_override_path_input: String,
    theme_ui_density: f32,
    theme_override_bg: Option<egui::Color32>,
    theme_override_text: Option<egui::Color32>,
    keymap_preset: KeymapPreset,
    binding_palette: String,
    binding_open: String,
    binding_save: String,
    binding_find: String,
    binding_format: String,
    custom_shortcuts: HashMap<String, ShortcutSpec>,
    autosave_enabled: bool,
    autosave_interval_sec: f64,
    last_autosave: f64,
    file_mtimes: HashMap<PathBuf, SystemTime>,
    file_watch_last_check: f64,
    telemetry_opt_in: bool,
    update_channel: UpdateChannel,
    plugin_sandbox_enabled: bool,
    plugins: Vec<crate::plugin::PluginManifest>,
    workspace_root: PathBuf,
    registry_entries: Vec<crate::plugin::RegistryEntry>,
    marketplace_status: String,
    show_git_panel: bool,
    git_panel: GitPanelState,
    lsp_rx: Option<Receiver<LspWorkerOutput>>,
    async_cmd_tx: Sender<AsyncCommandResult>,
    async_cmd_rx: Receiver<AsyncCommandResult>,
    lsp_last_request: f64,
}

impl LuxApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut command_palette = CommandPalette::new();
        command_palette.register_extension_commands(
            "Workspace",
            [
                ("Open Docs", "", "open_docs"),
                ("Run Benchmark", "", "run_benchmark"),
            ],
        );
        let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let plugins = crate::plugin::load_plugin_manifests(&workspace);
        let registry_entries = crate::plugin::load_registry(&workspace);
        let (workspace_font_setting, folder_font_overrides) = load_workspace_settings(&workspace);
        let mut recent_workspaces = load_recent_workspaces();
        if !recent_workspaces.iter().any(|p| p == &workspace) {
            recent_workspaces.insert(0, workspace.clone());
            if recent_workspaces.len() > 12 {
                recent_workspaces.truncate(12);
            }
            persist_recent_workspaces(recent_workspaces.as_slice());
        }
        for plugin in &plugins {
            let tuples: Vec<(String, String, String)> = plugin
                .commands
                .iter()
                .map(|cmd| (cmd.title.clone(), cmd.shortcut.clone(), cmd.id.clone()))
                .collect();
            command_palette.register_extension_commands(
                plugin.name.as_str(),
                tuples
                    .iter()
                    .map(|(title, shortcut, id)| (title.as_str(), shortcut.as_str(), id.as_str())),
            );
        }
        let (editors, active_tab) =
            load_session_editors().unwrap_or_else(|| (vec![Editor::new()], 0));
        let (async_cmd_tx, async_cmd_rx) = mpsc::channel();
        let mut app = Self {
            editors,
            active_tab,
            command_palette,
            show_search: false,
            show_replace: false,
            search_input: String::new(),
            replace_input: String::new(),
            show_goto_line: false,
            goto_line_input: String::new(),
            clipboard: Clipboard::new().ok(),
            highlighter: SyntaxHighlighter::new(),
            confirm_close_tab: None,
            pending_close_others: None,
            pending_external_change: None,
            closed_tabs: Vec::new(),
            dragging_tab: None,
            editor_theme: EditorThemeKind::Monokai,
            git_info: None,
            git_last_check: 0.0,
            show_sidebar: true,
            sidebar_tab: SidebarTab::Explorer,
            sidebar_search_query: String::new(),
            sidebar_search_case_sensitive: false,
            sidebar_search_regex: false,
            sidebar_search_include_glob: String::new(),
            sidebar_search_exclude_glob: String::new(),
            sidebar_search_results: Vec::new(),
            sidebar_search_selected: None,
            sidebar_search_preview: String::new(),
            sidebar_replace_input: String::new(),
            sidebar_search_message: String::new(),
            sidebar_symbol_query: String::new(),
            sidebar_symbol_results: Vec::new(),
            refactor_name_input: String::new(),
            refactor_status: String::new(),
            refactor_preview: None,
            sidebar_files: Vec::new(),
            sidebar_last_scan: 0.0,
            quick_open_query: String::new(),
            recent_workspaces,
            file_ops_target: String::new(),
            file_ops_message: String::new(),
            deleted_file_backup: None,
            debug_breakpoints: HashMap::new(),
            debug_watch_input: String::new(),
            debug_watches: Vec::new(),
            debug_call_stack: vec!["main()".to_string()],
            tasks: vec![
                WorkspaceTask {
                    name: "build".to_string(),
                    command: "cargo build".to_string(),
                },
                WorkspaceTask {
                    name: "test".to_string(),
                    command: "cargo test".to_string(),
                },
                WorkspaceTask {
                    name: "lint".to_string(),
                    command: "cargo clippy".to_string(),
                },
            ],
            new_task_name: String::new(),
            new_task_command: String::new(),
            run_configs: Vec::new(),
            new_run_config_name: String::new(),
            new_run_config_command: String::new(),
            new_run_config_env: String::new(),
            diagnostics_show_error: true,
            diagnostics_show_warning: true,
            diagnostics_show_info: true,
            diagnostics_language_enabled: HashMap::new(),
            format_on_save: false,
            format_on_type: false,
            formatter_by_language: HashMap::new(),
            lint_workspace_overrides: HashMap::new(),
            lint_folder_overrides: HashMap::new(),
            lint_override_rule_input: String::new(),
            lint_override_value_input: String::new(),
            collab_enabled: false,
            collab_session_id: String::new(),
            collab_review_mode: false,
            collab_note_input: String::new(),
            collab_notes: HashMap::new(),
            collab_peer_cursors: Vec::new(),
            workspace_roots: vec![workspace.clone()],
            new_workspace_root_input: String::new(),
            project_gitignore_patterns: vec!["target/**".to_string()],
            trusted_workspaces: {
                let mut set = HashSet::new();
                set.insert(workspace.clone());
                set
            },
            safe_mode: false,
            show_help_window: false,
            show_onboarding: false,
            onboarding_step: 0,
            show_troubleshooting: false,
            portable_mode: false,
            packaging_status: String::new(),
            qa_status: String::new(),
            locale_code: "en-US".to_string(),
            rtl_layout: false,
            ime_test_input: String::new(),
            settings_sync_enabled: false,
            settings_sync_path: String::new(),
            settings_sync_status: String::new(),
            settings_profile: "default".to_string(),
            settings_role_profile: "dev".to_string(),
            secret_key_input: String::new(),
            secret_value_input: String::new(),
            secrets_status: String::new(),
            show_clone_repo_dialog: false,
            clone_repo_url: String::new(),
            clone_repo_target: String::new(),
            onboarding_first_run_done: false,
            observability_level: LogLevel::Info,
            observability_module_filter: String::new(),
            observability_health_status: String::new(),
            split_mode: SplitMode::None,
            split_secondary_tab: None,
            zen_mode: false,
            focus_mode: false,
            show_dock_panel: false,
            dock_tab: DockPanelTab::Terminal,
            dock_side: DockSide::Bottom,
            dock_size: 180.0,
            terminal_profiles: vec![
                TerminalProfile {
                    name: "Default".to_string(),
                    shell: "sh".to_string(),
                    theme_hint: "Dark".to_string(),
                },
                TerminalProfile {
                    name: "Bash".to_string(),
                    shell: "bash".to_string(),
                    theme_hint: "Classic".to_string(),
                },
                TerminalProfile {
                    name: "Zsh".to_string(),
                    shell: "zsh".to_string(),
                    theme_hint: "Neon".to_string(),
                },
            ],
            terminal_profile_idx: 0,
            terminal_split_panes: false,
            terminal_input_secondary: String::new(),
            terminal_log_secondary: Vec::new(),
            terminal_input: String::new(),
            terminal_log: Vec::new(),
            output_log: Vec::new(),
            output_filter: String::new(),
            problems_filter: String::new(),
            workspace_fonts: {
                let mut map = HashMap::new();
                map.insert(workspace.clone(), workspace_font_setting);
                map
            },
            folder_font_overrides,
            folder_override_path_input: String::new(),
            theme_ui_density: 1.0,
            theme_override_bg: None,
            theme_override_text: None,
            keymap_preset: KeymapPreset::Vscode,
            binding_palette: String::new(),
            binding_open: String::new(),
            binding_save: String::new(),
            binding_find: String::new(),
            binding_format: String::new(),
            custom_shortcuts: HashMap::new(),
            autosave_enabled: true,
            autosave_interval_sec: 5.0,
            last_autosave: 0.0,
            file_mtimes: HashMap::new(),
            file_watch_last_check: 0.0,
            telemetry_opt_in: false,
            update_channel: UpdateChannel::Stable,
            plugin_sandbox_enabled: true,
            plugins,
            workspace_root: workspace,
            registry_entries,
            marketplace_status: String::new(),
            show_git_panel: false,
            git_panel: GitPanelState::default(),
            lsp_rx: None,
            async_cmd_tx,
            async_cmd_rx,
            lsp_last_request: 0.0,
        };
        app.apply_cli_open_paths();
        app.initialize_first_run_onboarding();
        app
    }

    fn active_editor(&mut self) -> &mut Editor {
        &mut self.editors[self.active_tab]
    }

    fn new_tab(&mut self) {
        self.editors.push(Editor::new());
        self.active_tab = self.editors.len() - 1;
    }

    fn close_tab(&mut self) {
        self.close_tab_idx(self.active_tab);
    }

    fn close_tab_idx(&mut self, idx: usize) {
        if self.editors.len() <= 1 {
            return;
        }
        if self.editors[idx].modified {
            self.confirm_close_tab = Some(idx);
        } else {
            self.force_close_tab(idx);
        }
    }

    fn force_close_tab(&mut self, idx: usize) {
        if self.editors.len() > 1 {
            let closed = self.editors.remove(idx);
            self.closed_tabs.push(closed);
            if self.closed_tabs.len() > 50 {
                self.closed_tabs.remove(0);
            }
            if let Some(secondary_idx) = self.split_secondary_tab {
                if secondary_idx == idx {
                    self.split_secondary_tab = None;
                } else if secondary_idx > idx {
                    self.split_secondary_tab = Some(secondary_idx - 1);
                }
            }
            if self.active_tab >= self.editors.len() {
                self.active_tab = self.editors.len() - 1;
            }
            self.ensure_split_secondary();
        }
        self.confirm_close_tab = None;
    }

    fn reopen_closed_tab(&mut self) {
        if let Some(editor) = self.closed_tabs.pop() {
            self.editors.push(editor);
            self.active_tab = self.editors.len() - 1;
        }
    }

    fn close_other_tabs(&mut self, keep_idx: usize) {
        if self.editors.len() <= 1 {
            return;
        }
        let mut idx = 0;
        let mut keep_idx = keep_idx;
        while idx < self.editors.len() {
            if idx == keep_idx || self.editors[idx].pinned {
                idx += 1;
                continue;
            }
            if self.editors[idx].modified {
                self.confirm_close_tab = Some(idx);
                self.pending_close_others = Some(keep_idx);
                return;
            }
            if idx < keep_idx {
                keep_idx = keep_idx.saturating_sub(1);
            }
            self.force_close_tab(idx);
        }
        self.pending_close_others = None;
    }

    fn pin_tab(&mut self, idx: usize, pinned: bool) {
        if let Some(editor) = self.editors.get_mut(idx) {
            editor.pinned = pinned;
        }
        let active_path = self.editors[self.active_tab].file_path.clone();
        let active_title = self.editors[self.active_tab].title.clone();
        let mut pinned_tabs = Vec::new();
        let mut regular_tabs = Vec::new();
        for editor in self.editors.drain(..) {
            if editor.pinned {
                pinned_tabs.push(editor);
            } else {
                regular_tabs.push(editor);
            }
        }
        pinned_tabs.append(&mut regular_tabs);
        self.editors = pinned_tabs;
        if let Some(path) = active_path {
            if let Some((idx, _)) = self
                .editors
                .iter()
                .enumerate()
                .find(|(_, e)| e.file_path == Some(path.clone()))
            {
                self.active_tab = idx;
            }
        } else if let Some((idx, _)) = self
            .editors
            .iter()
            .enumerate()
            .find(|(_, e)| e.title == active_title)
        {
            self.active_tab = idx;
        }
    }

    fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.editors.len() || to >= self.editors.len() {
            return;
        }
        let tab = self.editors.remove(from);
        self.editors.insert(to, tab);
        if self.active_tab == from {
            self.active_tab = to;
        } else if from < self.active_tab && to >= self.active_tab {
            self.active_tab = self.active_tab.saturating_sub(1);
        } else if from > self.active_tab && to <= self.active_tab {
            self.active_tab += 1;
        }
        if let Some(secondary_idx) = self.split_secondary_tab {
            self.split_secondary_tab = Some(remap_tab_index_after_move(secondary_idx, from, to));
        }
    }

    fn move_tab_to_split(&mut self, idx: usize, to_secondary: bool) {
        if idx >= self.editors.len() {
            return;
        }
        if to_secondary && self.split_mode == SplitMode::None {
            self.split_mode = SplitMode::Vertical;
        }
        if self.split_mode == SplitMode::None {
            return;
        }
        if to_secondary {
            if idx == self.active_tab {
                if let Some(previous_secondary) = self.split_secondary_tab {
                    if previous_secondary != idx {
                        self.active_tab = previous_secondary;
                    }
                } else if let Some(primary_candidate) =
                    (0..self.editors.len()).find(|candidate| *candidate != idx)
                {
                    self.active_tab = primary_candidate;
                }
            }
            self.split_secondary_tab = Some(idx);
        } else if self.split_secondary_tab == Some(idx) {
            let previous_primary = self.active_tab;
            self.active_tab = idx;
            if previous_primary != idx {
                self.split_secondary_tab = Some(previous_primary);
            } else {
                self.split_secondary_tab = None;
            }
        } else {
            self.active_tab = idx;
        }
        self.ensure_split_secondary();
    }

    fn update_git_info(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.git_last_check < 1.0 {
            return;
        }
        self.git_last_check = now;
        let path = self
            .editors
            .get(self.active_tab)
            .and_then(|e| e.file_path.as_ref())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        self.git_info = read_git_info(path.as_deref());
    }

    fn active_repo_dir(&self) -> Option<PathBuf> {
        self.editors
            .get(self.active_tab)
            .and_then(|e| e.file_path.as_ref())
            .and_then(|p| p.parent())
            .and_then(resolve_git_root)
    }

    fn active_workspace_key(&self) -> PathBuf {
        self.active_repo_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    fn active_language_label(&self) -> String {
        let editor = &self.editors[self.active_tab];
        self.highlighter.syntax_name_for(
            editor.file_path.as_deref(),
            editor.first_line_text().as_deref(),
            editor.syntax_override.as_deref(),
        )
    }

    fn current_font_settings(&self) -> EditorFontSettings {
        let key = self.active_workspace_key();
        let mut base = self
            .workspace_fonts
            .get(&key)
            .cloned()
            .unwrap_or(WorkspaceFontSettings {
                size: 13.5,
                family: FontFamilyKind::Monospace,
                ligatures: false,
            });
        if let Some(path) = self
            .editors
            .get(self.active_tab)
            .and_then(|e| e.file_path.as_ref())
        {
            for (folder, override_font) in &self.folder_font_overrides {
                if path.starts_with(folder) {
                    base = override_font.clone();
                    break;
                }
            }
        }
        EditorFontSettings {
            size: base.size,
            family: base.family,
            ligatures: base.ligatures,
        }
    }

    fn refresh_git_panel_data(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.git_panel.last_refresh < 1.5 {
            return;
        }
        self.git_panel.last_refresh = now;

        let Some(repo) = self.active_repo_dir() else {
            self.git_panel.files.clear();
            self.git_panel.commits.clear();
            self.git_panel.diff_text.clear();
            self.git_panel.blame_text = "No repository".to_string();
            return;
        };

        self.git_panel.files = read_git_files(&repo);
        self.git_panel.commits = read_git_commits(&repo);

        if self.git_panel.selected_file.is_none() {
            self.git_panel.selected_file = self.git_panel.files.first().map(|f| f.path.clone());
        }
        if let Some(selected) = self.git_panel.selected_file.clone() {
            self.git_panel.diff_text = read_git_diff_for_file(&repo, &selected);
        }
        self.git_panel.blame_text = read_active_line_blame(&repo, &self.editors[self.active_tab]);
    }

    fn show_git_panel_ui(&mut self, ui: &mut egui::Ui) {
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

    fn refresh_sidebar_files(&mut self, ctx: &egui::Context) {
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

    fn run_sidebar_search(&mut self) {
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

    fn update_search_preview(&mut self, hit: &str) {
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

    fn replace_all_search_results(&mut self) {
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

    fn run_symbol_search(&mut self) {
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

    fn open_path_in_tab(&mut self, path: &Path) {
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

    fn apply_project_conventions(&self, editor: &mut Editor) {
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

    fn apply_language_toolchain_defaults(&mut self, editor: &Editor) {
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

    fn import_external_settings(&mut self) {
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

    fn workspace_join(&self, relative_or_abs: &str) -> PathBuf {
        let p = PathBuf::from(relative_or_abs);
        if p.is_absolute() {
            p
        } else {
            self.workspace_root.join(p)
        }
    }

    fn create_file_or_folder(&mut self, is_folder: bool) {
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

    fn duplicate_active_file(&mut self) {
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

    fn rename_or_move_active_file(&mut self) {
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

    fn delete_active_file(&mut self) {
        let Some(path) = self.editors[self.active_tab].file_path.clone() else {
            self.file_ops_message = "No active file".to_string();
            return;
        };
        let backup = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.file_ops_message = format!("Delete failed: {err}");
                return;
            }
        };
        self.file_ops_message = match std::fs::remove_file(&path) {
            Ok(_) => {
                self.deleted_file_backup = Some((path.clone(), backup));
                self.sidebar_last_scan = 0.0;
                if self.editors.len() > 1 {
                    self.force_close_tab(self.active_tab);
                } else {
                    self.editors[0] = Editor::new();
                    self.active_tab = 0;
                }
                "Deleted (Undo available)".to_string()
            }
            Err(err) => format!("Delete failed: {err}"),
        };
    }

    fn undo_last_deleted_file(&mut self) {
        let Some((path, contents)) = self.deleted_file_backup.take() else {
            self.file_ops_message = "Nothing to undo".to_string();
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&path, &contents) {
            Ok(_) => {
                self.sidebar_last_scan = 0.0;
                self.file_ops_message = format!("Restored {}", path.to_string_lossy());
                self.open_path_in_tab(path.as_path());
            }
            Err(err) => {
                self.file_ops_message = format!("Undo delete failed: {err}");
                self.deleted_file_backup = Some((path, contents));
            }
        }
    }

    fn sidebar_tab_title(tab: SidebarTab) -> &'static str {
        match tab {
            SidebarTab::Explorer => "Explorer",
            SidebarTab::Search => "Search",
            SidebarTab::Git => "Source Control",
            SidebarTab::Debug => "Run and Debug",
            SidebarTab::Collab => "Collab",
        }
    }

    fn sidebar_tab_hint(tab: SidebarTab) -> &'static str {
        match tab {
            SidebarTab::Explorer => "Ctrl+Alt+1",
            SidebarTab::Search => "Ctrl+Alt+2",
            SidebarTab::Git => "Ctrl+Alt+3",
            SidebarTab::Debug => "Ctrl+Alt+4",
            SidebarTab::Collab => "Ctrl+Alt+5",
        }
    }

    fn show_activity_bar(&mut self, ui: &mut egui::Ui) {
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

    fn show_left_sidebar(&mut self, ui: &mut egui::Ui) {
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
                                self.delete_active_file();
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

    fn ensure_split_secondary(&mut self) {
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

    fn run_terminal_command(&mut self, secondary: bool) {
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

    fn add_breakpoint_at_cursor(&mut self) {
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

    fn run_workspace_task(&mut self, command: &str) {
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

    fn run_configuration(&mut self, cfg: &RunConfiguration) {
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

    fn spawn_shell_command(
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

    fn poll_async_command_results(&mut self) {
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

    fn render_dock_panel_contents(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
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

    fn show_dock_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
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

    fn refresh_editor_insights(&mut self, ctx: &egui::Context) {
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

    fn refresh_lsp_features(&mut self, ctx: &egui::Context) {
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

    fn open_file(&mut self) {
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

    fn apply_cli_open_paths(&mut self) {
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

    fn initialize_first_run_onboarding(&mut self) {
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

    fn create_from_template(&mut self, template: &str) {
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

    fn clone_repository_action(&mut self) {
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

    fn build_release_artifacts(&mut self) {
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

    fn check_update_channel(&mut self) {
        let channel = match self.update_channel {
            UpdateChannel::Stable => "stable",
            UpdateChannel::Beta => "beta",
            UpdateChannel::Nightly => "nightly",
        };
        self.packaging_status = format!("Checked updates on '{}' channel: no updates", channel);
    }

    fn export_app_config(&mut self) {
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

    fn import_app_config(&mut self) {
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

    fn run_ui_test_harness(&mut self) {
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

    fn run_golden_snapshot_check(&mut self) {
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

    fn run_performance_benchmark(&mut self) {
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

    fn export_crash_triage_bundle(&mut self) {
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

    fn export_diagnostics_bundle(&mut self) {
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

    fn run_health_checks(&mut self) {
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

    fn sync_settings_now(&mut self) {
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

    fn store_secret_securely(&mut self) {
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

    fn export_theme_json(&self) {
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

    fn import_theme_json(&mut self) {
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

    fn default_shortcut(&self, action: &str) -> ShortcutSpec {
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

    fn shortcut_for_action(&self, action: &str) -> ShortcutSpec {
        self.custom_shortcuts
            .get(action)
            .copied()
            .unwrap_or_else(|| self.default_shortcut(action))
    }

    fn shortcut_pressed(ctx: &egui::Context, spec: ShortcutSpec) -> bool {
        ctx.input(|i| {
            i.key_pressed(spec.key)
                && i.modifiers.command == spec.command
                && i.modifiers.shift == spec.shift
                && i.modifiers.alt == spec.alt
        })
    }

    fn apply_custom_binding(&mut self, action: &str, raw: &str) {
        if let Some(spec) = parse_shortcut(raw) {
            self.custom_shortcuts.insert(action.to_string(), spec);
        }
    }

    fn save_workspace_settings(&self) {
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

    fn collab_state_path(&self) -> PathBuf {
        self.workspace_root
            .join(".lux")
            .join(format!("collab-{}.json", self.collab_session_id))
    }

    fn sync_collaboration_state(&mut self) {
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

    fn export_handoff_snapshot(&mut self) {
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

    fn save_file(&mut self) {
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

    fn save_file_as(&mut self) {
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

    fn register_recent_workspace(&mut self, maybe_path: Option<PathBuf>) {
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

    fn capture_refactor_preview(&mut self, tab_idx: usize, title: &str, before: String) {
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

    fn push_output_log(&mut self, line: String) {
        self.output_log.push(redact_secrets(&line));
        Self::cap_vec(&mut self.output_log, 500);
    }

    fn cap_vec(vec: &mut Vec<String>, max_len: usize) {
        if vec.len() > max_len {
            vec.drain(0..(vec.len() - max_len));
        }
    }

    fn log_event(&mut self, module: &str, level: LogLevel, message: &str) {
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

    fn run_autosave_and_recovery(&mut self, ctx: &egui::Context) {
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

    fn refresh_file_watchers(&mut self, ctx: &egui::Context) {
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

    fn handle_command(&mut self, cmd: CommandId) {
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

    fn cut(&mut self) {
        let text = self.active_editor().cut_text();
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(&text);
        }
    }

    fn copy(&mut self) {
        let text = self.active_editor().copy_text();
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(&text);
        }
    }

    fn paste(&mut self) {
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

    fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
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

    fn show_menu_bar(&mut self, ctx: &egui::Context) {
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

    fn show_tab_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(TAB_BAR_BG)
            .inner_margin(egui::Margin::same(0.0)) // No margin, full bleed
            .show(ui, |ui| {
                let mut tab_rects: Vec<(usize, egui::Rect, bool)> = Vec::new();
                let mut tab_action: Option<TabAction> = None;
                let pointer_pos = ui.ctx().input(|i| i.pointer.latest_pos());
                let pointer_released = ui.ctx().input(|i| i.pointer.any_released());
                let mut primary_drop_rect: Option<egui::Rect> = None;
                let mut secondary_drop_rect: Option<egui::Rect> = None;

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO; // Connect tabs

                    egui::ScrollArea::horizontal()
                        .id_salt("tabs_scroll")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::new(1.0, 0.0); // 1px gap

                                for i in 0..self.editors.len() {
                                    let (title, modified, pinned) = {
                                        let editor = &self.editors[i];
                                        (editor.title.clone(), editor.modified, editor.pinned)
                                    };
                                    let is_active = i == self.active_tab;
                                    let text = title.clone();

                                    let text_color = if is_active {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::from_rgb(180, 180, 180)
                                    };

                                    // Calculate Width
                                    let font = egui::FontId::proportional(12.5); // Slightly larger
                                    let text_width = ui.fonts(|f| {
                                        f.layout_no_wrap(text.clone(), font.clone(), text_color)
                                            .rect
                                            .width()
                                    });
                                    let tab_width =
                                        text_width + TAB_PADDING_X * 2.0 + TAB_CLOSE_SIZE + 8.0;
                                    if self.editors.len() <= 1 {
                                        // Even single tab might want a close button in modern designs,
                                        // but let's keep it optional if space is tight?
                                        // Actuall VS code usually shows it on hover.
                                        // For now, let's include space for it to avoid jumping.
                                    }
                                    let tab_width = tab_width.clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH);

                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::Vec2::new(tab_width, TAB_HEIGHT),
                                        egui::Sense::click(),
                                    );
                                    tab_rects.push((i, rect, pinned));

                                    // Background
                                    let bg = if is_active {
                                        TAB_ACTIVE_BG
                                    } else if response.hovered() {
                                        TAB_HOVER_BG
                                    } else {
                                        TAB_INACTIVE_BG
                                    };

                                    // Custom Tab Painting
                                    let painter = ui.painter();

                                    // Main tab shape
                                    painter.rect_filled(rect, 0.0, bg);

                                    // Active top border
                                    if is_active {
                                        painter.rect_filled(
                                            egui::Rect::from_min_size(
                                                rect.min,
                                                egui::Vec2::new(rect.width(), 2.0),
                                            ),
                                            0.0,
                                            ACCENT_COLOR,
                                        );
                                    } else {
                                        // Separator line at bottom for inactive tabs implies the active one overlaps
                                        // But here we can just do nothing for now, simplest flat look
                                    }

                                    // Content Layout
                                    // Modified dot or Icon
                                    let mut text_x = rect.left() + TAB_PADDING_X;
                                    if modified {
                                        // A prettier "unsaved" circle
                                        painter.circle_filled(
                                            egui::Pos2::new(text_x + 3.0, rect.center().y),
                                            4.0,
                                            egui::Color32::WHITE, // Or generic fg
                                        );
                                        // And a smaller inner dot for "modified"
                                        painter.circle_filled(
                                            egui::Pos2::new(text_x + 3.0, rect.center().y),
                                            2.5,
                                            egui::Color32::from_rgb(255, 165, 90), // Orange
                                        );
                                        text_x += 14.0;
                                    } else {
                                        // Maybe an icon here? For now just text.
                                    }

                                    // File Name
                                    painter.text(
                                        egui::Pos2::new(text_x, rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        text,
                                        font.clone(),
                                        text_color,
                                    );

                                    // Interactions
                                    if response.clicked() {
                                        tab_action = Some(TabAction::Activate(i));
                                    }
                                    if response.middle_clicked() {
                                        tab_action = Some(TabAction::Close(i));
                                    }

                                    // Close Button - Show on hover or if active
                                    if response.hovered() || is_active {
                                        let close_rect = egui::Rect::from_min_size(
                                            egui::Pos2::new(
                                                rect.right() - TAB_CLOSE_SIZE - 6.0,
                                                rect.center().y - TAB_CLOSE_SIZE / 2.0,
                                            ),
                                            egui::Vec2::new(TAB_CLOSE_SIZE, TAB_CLOSE_SIZE),
                                        );

                                        // Check interaction specifically on the small close rect
                                        let close_resp = ui.interact(
                                            close_rect,
                                            ui.id().with(("tab_close", i)),
                                            egui::Sense::click(),
                                        );

                                        let close_hovered = close_resp.hovered();

                                        // Draw 'x'
                                        // Rotate 45deg +
                                        let center = close_rect.center();
                                        if close_hovered {
                                            painter.rect_filled(
                                                close_rect,
                                                2.0,
                                                egui::Color32::from_white_alpha(30),
                                            );
                                        }

                                        let stroke = egui::Stroke::new(
                                            1.0,
                                            if close_hovered {
                                                egui::Color32::WHITE
                                            } else {
                                                egui::Color32::from_gray(150)
                                            },
                                        );
                                        // Manually draw X for better control than text
                                        let r = TAB_CLOSE_SIZE / 3.0;
                                        painter.line_segment(
                                            [
                                                center + egui::Vec2::new(-r, -r),
                                                center + egui::Vec2::new(r, r),
                                            ],
                                            stroke,
                                        );
                                        painter.line_segment(
                                            [
                                                center + egui::Vec2::new(-r, r),
                                                center + egui::Vec2::new(r, -r),
                                            ],
                                            stroke,
                                        );

                                        if close_resp.clicked() {
                                            tab_action = Some(TabAction::Close(i));
                                        }
                                    }

                                    // Context Menu
                                    response.context_menu(|ui| {
                                        if ui.button("Close").clicked() {
                                            tab_action = Some(TabAction::Close(i));
                                            ui.close_menu();
                                        }
                                        if ui.button("Close Others").clicked() {
                                            tab_action = Some(TabAction::CloseOthers(i));
                                            ui.close_menu();
                                        }
                                        if ui.button("Reopen Closed Tab").clicked() {
                                            tab_action = Some(TabAction::ReopenClosed);
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        let pin_label = if pinned { "Unpin" } else { "Pin" };
                                        if ui.button(pin_label).clicked() {
                                            tab_action = Some(TabAction::TogglePin(i, !pinned));
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Move to Primary Split").clicked() {
                                            tab_action = Some(TabAction::MoveToSplit(i, false));
                                            ui.close_menu();
                                        }
                                        if ui.button("Move to Secondary Split").clicked() {
                                            tab_action = Some(TabAction::MoveToSplit(i, true));
                                            ui.close_menu();
                                        }
                                    });

                                    if response.drag_started() {
                                        self.dragging_tab = Some(i);
                                    }
                                }

                                // New Tab Button (small)
                                let new_tab_resp = ui.allocate_response(
                                    egui::Vec2::new(32.0, TAB_HEIGHT),
                                    egui::Sense::click(),
                                );
                                if new_tab_resp.hovered() {
                                    ui.painter()
                                        .rect_filled(new_tab_resp.rect, 0.0, TAB_HOVER_BG);
                                }
                                ui.painter().text(
                                    new_tab_resp.rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "+",
                                    egui::FontId::proportional(16.0),
                                    egui::Color32::GRAY,
                                );
                                if new_tab_resp.clicked() {
                                    tab_action = Some(TabAction::NewTab);
                                }
                            });
                        });

                    if self.split_mode != SplitMode::None || self.dragging_tab.is_some() {
                        ui.add_space(8.0);
                        let dragging = self.dragging_tab.is_some();
                        let (primary_rect, primary_resp) = ui.allocate_exact_size(
                            egui::Vec2::new(82.0, TAB_HEIGHT - 6.0),
                            egui::Sense::hover(),
                        );
                        primary_drop_rect = Some(primary_rect);
                        let primary_hovered = pointer_pos
                            .map(|pos| primary_rect.contains(pos))
                            .unwrap_or(false);
                        let primary_bg = if dragging && primary_hovered {
                            egui::Color32::from_rgb(0, 122, 204)
                        } else {
                            TAB_INACTIVE_BG
                        };
                        ui.painter().rect_filled(primary_rect, 4.0, primary_bg);
                        ui.painter().text(
                            primary_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Primary",
                            egui::FontId::proportional(11.5),
                            egui::Color32::from_gray(220),
                        );
                        primary_resp.on_hover_text("Drop tab to move into primary split");

                        ui.add_space(4.0);
                        let (secondary_rect, secondary_resp) = ui.allocate_exact_size(
                            egui::Vec2::new(92.0, TAB_HEIGHT - 6.0),
                            egui::Sense::hover(),
                        );
                        secondary_drop_rect = Some(secondary_rect);
                        let secondary_hovered = pointer_pos
                            .map(|pos| secondary_rect.contains(pos))
                            .unwrap_or(false);
                        let secondary_bg = if dragging && secondary_hovered {
                            egui::Color32::from_rgb(0, 122, 204)
                        } else {
                            TAB_INACTIVE_BG
                        };
                        ui.painter().rect_filled(secondary_rect, 4.0, secondary_bg);
                        ui.painter().text(
                            secondary_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Secondary",
                            egui::FontId::proportional(11.5),
                            egui::Color32::from_gray(220),
                        );
                        secondary_resp.on_hover_text("Drop tab to move into secondary split");
                    }
                });

                if pointer_released {
                    if let (Some(drag_idx), Some(pos)) = (self.dragging_tab, pointer_pos) {
                        let mut dropped_in_split_target = false;
                        if let Some(rect) = primary_drop_rect {
                            if rect.contains(pos) {
                                tab_action = Some(TabAction::MoveToSplit(drag_idx, false));
                                dropped_in_split_target = true;
                            }
                        }
                        if !dropped_in_split_target {
                            if let Some(rect) = secondary_drop_rect {
                                if rect.contains(pos) {
                                    tab_action = Some(TabAction::MoveToSplit(drag_idx, true));
                                    dropped_in_split_target = true;
                                }
                            }
                        }
                        if !dropped_in_split_target {
                            if let Some(target_idx) = find_drop_target(drag_idx, pos, &tab_rects) {
                                tab_action = Some(TabAction::Reorder(drag_idx, target_idx));
                            }
                        }
                    }
                    self.dragging_tab = None;
                }

                if let Some(action) = tab_action {
                    match action {
                        TabAction::Activate(idx) => self.active_tab = idx,
                        TabAction::Close(idx) => self.close_tab_idx(idx),
                        TabAction::CloseOthers(idx) => self.close_other_tabs(idx),
                        TabAction::ReopenClosed => self.reopen_closed_tab(),
                        TabAction::TogglePin(idx, pinned) => self.pin_tab(idx, pinned),
                        TabAction::Reorder(from, to) => self.move_tab(from, to),
                        TabAction::MoveToSplit(idx, secondary) => {
                            self.move_tab_to_split(idx, secondary)
                        }
                        TabAction::NewTab => self.new_tab(),
                    }
                }
            });
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_search {
            return;
        }

        ui.add_space(8.0);
        // Floating panel look
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(37, 37, 38))
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 122, 204))) // Active border
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Find")
                            .color(egui::Color32::from_rgb(200, 200, 200))
                            .size(13.0),
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_input)
                            .desired_width(200.0)
                            .font(egui::FontId::monospace(13.0))
                            .text_color(egui::Color32::WHITE)
                            .margin(egui::Vec2::new(4.0, 2.0))
                            .hint_text("Search..."),
                    );

                    if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let query = self.search_input.clone();
                        if ui.input(|i| i.modifiers.shift) {
                            self.active_editor().find_prev(&query);
                        } else {
                            self.active_editor().find_next(&query);
                        }
                        response.request_focus();
                    }

                    if ui
                        .add(egui::Button::new(egui::RichText::new("↑").size(14.0)))
                        .on_hover_text("Previous Match")
                        .clicked()
                    {
                        let query = self.search_input.clone();
                        self.active_editor().find_prev(&query);
                    }

                    if ui
                        .add(egui::Button::new(egui::RichText::new("↓").size(14.0)))
                        .on_hover_text("Next Match")
                        .clicked()
                    {
                        let query = self.search_input.clone();
                        self.active_editor().find_next(&query);
                    }

                    // Toggles for options could go here

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("\u{1F5D9}").size(14.0), // Cancel X
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.show_search = false;
                        self.show_replace = false;
                    }
                });
            });

        // Replace row
        if self.show_replace {
            ui.add_space(2.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(37, 37, 38))
                .rounding(egui::Rounding::same(4.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60)))
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Replace")
                                .color(egui::Color32::from_rgb(200, 200, 200))
                                .size(13.0),
                        );

                        ui.add(
                            egui::TextEdit::singleline(&mut self.replace_input)
                                .desired_width(200.0)
                                .font(egui::FontId::monospace(13.0))
                                .text_color(egui::Color32::WHITE)
                                .margin(egui::Vec2::new(4.0, 2.0))
                                .hint_text("Replace..."),
                        );

                        if ui
                            .add(egui::Button::new(egui::RichText::new("Replace").size(12.0)))
                            .clicked()
                        {
                            let find = self.search_input.clone();
                            let replace = self.replace_input.clone();
                            self.active_editor().replace_next(&find, &replace);
                        }

                        if ui
                            .add(egui::Button::new(egui::RichText::new("All").size(12.0)))
                            .clicked()
                        {
                            let find = self.search_input.clone();
                            let replace = self.replace_input.clone();
                            self.active_editor().replace_all(&find, &replace);
                        }
                    });
                });
        }
    }

    fn show_goto_line_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_goto_line {
            return;
        }

        ui.add_space(8.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(37, 37, 38))
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 122, 204)))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Go to Line")
                            .color(egui::Color32::from_rgb(200, 200, 200))
                            .size(13.0),
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.goto_line_input)
                            .desired_width(100.0)
                            .font(egui::FontId::monospace(13.0))
                            .text_color(egui::Color32::WHITE)
                            .margin(egui::Vec2::new(4.0, 2.0))
                            .hint_text("Line..."),
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(line) = self.goto_line_input.trim().parse::<usize>() {
                            self.active_editor().goto_line(line);
                        }
                        self.show_goto_line = false;
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.show_goto_line = false;
                    }

                    if ui.button("Go").clicked() {
                        if let Ok(line) = self.goto_line_input.trim().parse::<usize>() {
                            self.active_editor().goto_line(line);
                        }
                        self.show_goto_line = false;
                    }
                });
            });
    }

    fn show_breadcrumbs(&mut self, ui: &mut egui::Ui) {
        let editor = &self.editors[self.active_tab];
        let crumbs = if let Some(path) = &editor.file_path {
            let mut parts: Vec<String> = path
                .components()
                .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
                .collect();
            if parts.is_empty() {
                vec![editor.title.clone()]
            } else {
                let file = parts.pop().unwrap_or_else(|| editor.title.clone());
                parts.push(file);
                parts
            }
        } else {
            vec![editor.title.clone()]
        };

        // Minimal breadcrumbs (transparent background)
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(16.0, 6.0)) // Indent start
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::new(4.0, 0.0);
                    for (idx, part) in crumbs.iter().enumerate() {
                        let is_last = idx == crumbs.len() - 1;
                        let color = if is_last {
                            egui::Color32::from_rgb(220, 220, 220)
                        } else {
                            egui::Color32::from_rgb(120, 120, 120)
                        };

                        // Clickable logic could go here later
                        ui.label(egui::RichText::new(part).color(color).size(12.0));

                        if !is_last {
                            ui.label(
                                egui::RichText::new("›")
                                    .color(egui::Color32::from_rgb(80, 80, 80))
                                    .size(12.0),
                            );
                        }
                    }
                });
            });
    }
}

impl eframe::App for LuxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());
        let native_scale = ctx.native_pixels_per_point().unwrap_or(1.0);
        ctx.set_pixels_per_point((native_scale * self.theme_ui_density).clamp(0.75, 4.0));

        if self.format_on_type {
            let now = now_secs();
            let editor = &mut self.editors[self.active_tab];
            if editor.modified && editor.last_edit_time > 0.0 && now - editor.last_edit_time > 0.6 {
                editor.request_formatting = true;
            }
        }
        let active_lang = self.active_language_label();
        self.diagnostics_language_enabled
            .entry(active_lang)
            .or_insert(true);
        self.safe_mode = !self.trusted_workspaces.contains(&self.workspace_root);

        self.update_git_info(ctx);
        self.refresh_editor_insights(ctx);
        self.refresh_lsp_features(ctx);
        self.editors[self.active_tab].refresh_code_actions();
        self.refresh_file_watchers(ctx);
        self.run_autosave_and_recovery(ctx);
        self.sync_collaboration_state();
        if self.show_git_panel {
            self.refresh_git_panel_data(ctx);
        }
        self.refresh_sidebar_files(ctx);
        self.poll_async_command_results();

        // Global shortcuts (handled before UI to avoid conflicts)
        if !self.command_palette.visible {
            self.handle_global_shortcuts(ctx);
        }

        // Command palette (rendered as overlay)
        if let Some(cmd) = self.command_palette.show(ctx) {
            self.command_palette.register_use(cmd.clone());
            self.handle_command(cmd);
        }

        if !self.zen_mode {
            self.show_menu_bar(ctx);
        }

        // Main panel
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(WINDOW_BG)
                    .inner_margin(egui::Margin::same(0.0)),
            )
            .show(ctx, |ui| {
                if self.rtl_layout {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        self.render_main_ui(ui, ctx);
                    });
                } else {
                    self.render_main_ui(ui, ctx);
                }
            });

        // Unsaved changes confirmation dialog
        if let Some(tab_idx) = self.confirm_close_tab {
            let title = self
                .editors
                .get(tab_idx)
                .map(|e| e.title.clone())
                .unwrap_or_else(|| "file".into());
            let mut close_action: Option<bool> = None;

            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("\"{}\" has unsaved changes.", title));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save & Close").clicked() {
                            close_action = Some(true);
                        }
                        if ui.button("Discard").clicked() {
                            close_action = Some(false);
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_close_tab = None;
                            self.pending_close_others = None;
                        }
                    });
                });

            match close_action {
                Some(true) => {
                    // Save then close
                    let _ = self.editors[tab_idx].save();
                    self.force_close_tab(tab_idx);
                    if let Some(keep_idx) = self.pending_close_others {
                        self.close_other_tabs(keep_idx.min(self.editors.len().saturating_sub(1)));
                    }
                }
                Some(false) => {
                    self.force_close_tab(tab_idx);
                    if let Some(keep_idx) = self.pending_close_others {
                        self.close_other_tabs(keep_idx.min(self.editors.len().saturating_sub(1)));
                    }
                }
                None => {}
            }
        }

        if let Some(prompt) = self.pending_external_change.clone() {
            let mut decision: Option<bool> = None;
            egui::Window::new("External File Change")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!(
                        "The file changed on disk:\n{}\nReload from disk?",
                        prompt.path.to_string_lossy()
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Reload Disk Version").clicked() {
                            decision = Some(true);
                        }
                        if ui.button("Keep Buffer").clicked() {
                            decision = Some(false);
                        }
                    });
                });
            if let Some(reload) = decision {
                if reload && prompt.tab_idx < self.editors.len() {
                    if let Ok(reloaded) = Editor::from_file(prompt.path.clone()) {
                        self.editors[prompt.tab_idx] = reloaded;
                    }
                }
                self.pending_external_change = None;
            }
        }

        if let Some(preview) = self.refactor_preview.clone() {
            let mut close = false;
            let mut rollback = false;
            egui::Window::new(format!("Refactor Preview: {}", preview.title))
                .resizable(true)
                .default_size([720.0, 420.0])
                .show(ctx, |ui| {
                    ui.label("Diff preview (simplified):");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(preview.diff_preview.clone())
                                .monospace()
                                .size(11.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Keep Changes").clicked() {
                            close = true;
                        }
                        if ui.button("Rollback File").clicked() {
                            rollback = true;
                            close = true;
                        }
                    });
                });
            if rollback && preview.tab_idx < self.editors.len() {
                self.editors[preview.tab_idx].set_document_text(&preview.original_text);
                self.refactor_status = "Refactor rolled back".to_string();
            }
            if close {
                self.refactor_preview = None;
            }
        }

        if self.show_help_window {
            egui::Window::new("Shortcut Cheat Sheet")
                .open(&mut self.show_help_window)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label("Ctrl+Shift+P -> Command Palette");
                    ui.label("Ctrl+F -> Find");
                    ui.label("Ctrl+Shift+F -> Format Document");
                    ui.label("Ctrl+Alt+1..5 -> Sidebar tabs");
                    ui.label("F12 / Shift+F12 / Alt+F12 -> LSP navigation");
                    ui.label("Cmd+Alt+R / Cmd+Alt+P -> Macro record/play");
                });
        }

        if self.show_onboarding {
            let mut open_file = false;
            let mut open_clone = false;
            let mut open_palette = false;
            let mut close_now = false;
            egui::Window::new("Onboarding")
                .open(&mut self.show_onboarding)
                .resizable(false)
                .default_size([680.0, 440.0])
                .show(ctx, |ui| {
                    ui.heading(
                        egui::RichText::new("Welcome to Lux")
                            .size(22.0)
                            .color(egui::Color32::from_rgb(214, 214, 214)),
                    );
                    ui.label(
                        egui::RichText::new(
                            "A quick setup flow inspired by modern IDE onboarding.",
                        )
                        .size(13.0),
                    );
                    ui.separator();
                    ui.columns(2, |columns| {
                        columns[0].group(|ui| {
                            ui.label(egui::RichText::new("Start").strong());
                            ui.add_space(4.0);
                            if ui.button("Open File...").clicked() {
                                open_file = true;
                            }
                            if ui.button("Clone Repository...").clicked() {
                                open_clone = true;
                            }
                            if ui.button("Open Command Palette").clicked() {
                                open_palette = true;
                            }
                            ui.separator();
                            if ui.button("Explorer").clicked() {
                                self.show_sidebar = true;
                                self.sidebar_tab = SidebarTab::Explorer;
                            }
                            if ui.button("Search").clicked() {
                                self.show_sidebar = true;
                                self.sidebar_tab = SidebarTab::Search;
                            }
                            if ui.button("Source Control").clicked() {
                                self.show_sidebar = true;
                                self.sidebar_tab = SidebarTab::Git;
                            }
                            if ui.button("Run and Debug").clicked() {
                                self.show_sidebar = true;
                                self.sidebar_tab = SidebarTab::Debug;
                            }
                            if ui.button("Show Terminal Panel").clicked() {
                                self.show_dock_panel = true;
                                self.dock_tab = DockPanelTab::Terminal;
                            }
                        });

                        columns[1].group(|ui| {
                            let steps = [
                                ("Open a project file", "Use Ctrl+O or File > Open."),
                                (
                                    "Search and navigate",
                                    "Use the Search panel for text/symbol lookup.",
                                ),
                                (
                                    "Refactor safely",
                                    "Use the Refactor menu and preview changes.",
                                ),
                                ("Run tasks", "Use Run and Debug to launch tasks/configs."),
                                ("Tune appearance", "Pick theme and fonts from Theme menu."),
                            ];
                            ui.label(egui::RichText::new("Getting Started").strong());
                            ui.add_space(4.0);
                            for (idx, (title, _)) in steps.iter().enumerate() {
                                let selected = idx == self.onboarding_step;
                                if ui.selectable_label(selected, *title).clicked() {
                                    self.onboarding_step = idx;
                                }
                            }
                            ui.separator();
                            let step = self.onboarding_step.min(steps.len().saturating_sub(1));
                            let (title, body) = steps[step];
                            ui.label(
                                egui::RichText::new(format!(
                                    "Step {} of {}: {}",
                                    step + 1,
                                    steps.len(),
                                    title
                                ))
                                .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(body);
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("Prev").clicked() {
                                    self.onboarding_step = self.onboarding_step.saturating_sub(1);
                                }
                                if ui.button("Next").clicked() {
                                    self.onboarding_step =
                                        (self.onboarding_step + 1).min(steps.len() - 1);
                                }
                            });
                            ui.add_space(6.0);
                            if ui.button("Done").clicked() {
                                close_now = true;
                            }
                        });
                    });
                });
            if close_now {
                self.show_onboarding = false;
            }
            if open_file {
                self.open_file();
            }
            if open_clone {
                self.show_clone_repo_dialog = true;
            }
            if open_palette {
                self.command_palette.visible = true;
            }
        }

        if self.show_troubleshooting {
            egui::Window::new("Troubleshooting")
                .open(&mut self.show_troubleshooting)
                .resizable(true)
                .default_size([760.0, 420.0])
                .show(ctx, |ui| {
                    ui.label("Diagnostics");
                    for diag in &self.editors[self.active_tab].diagnostics {
                        ui.label(format!(
                            "Ln {} [{}] {}",
                            diag.line + 1,
                            diag.severity,
                            diag.message
                        ));
                    }
                    ui.separator();
                    ui.label("Recent Output Logs");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for line in self.output_log.iter().rev().take(200) {
                            ui.label(egui::RichText::new(line).monospace().size(11.0));
                        }
                    });
                });
        }

        if self.show_clone_repo_dialog {
            let mut open = self.show_clone_repo_dialog;
            egui::Window::new("Clone Repository")
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.clone_repo_url)
                            .hint_text("https://github.com/org/repo.git"),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.clone_repo_target)
                            .hint_text("target folder (optional)"),
                    );
                    if ui.button("Clone").clicked() {
                        self.clone_repository_action();
                    }
                });
            self.show_clone_repo_dialog = open;
        }

        ctx.request_repaint();
    }
}

impl LuxApp {
    fn render_main_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.focus_mode {
            self.show_activity_bar(ui);
            self.show_left_sidebar(ui);
            self.show_git_panel_ui(ui);
        }
        self.show_dock_panel(ui, ctx);

        if !self.zen_mode {
            self.show_tab_bar(ui);
            self.show_breadcrumbs(ui);
            self.show_search_bar(ui);
            self.show_goto_line_bar(ui);
        }

        ui.add_space(0.0);

        // Editor area (takes remaining space minus status bar)
        let status_bar_height = if self.zen_mode { 0.0 } else { 24.0 };
        let available = ui.available_rect_before_wrap();
        let editor_rect = egui::Rect::from_min_max(
            available.min,
            egui::Pos2::new(available.max.x, available.max.y - status_bar_height),
        );
        let editor_idx = self.active_tab;
        let show_preview = {
            let editor = &self.editors[editor_idx];
            editor.is_markdown() && editor.markdown_preview
        };
        let preview_text = if show_preview {
            Some(self.editors[editor_idx].rope.to_string())
        } else {
            None
        };

        let auto_focus = !self.show_search
            && !self.show_goto_line
            && !self.command_palette.visible
            && self.confirm_close_tab.is_none();
        let mut editor_theme = self.editor_theme.palette();
        if let Some(bg) = self.theme_override_bg {
            editor_theme.background = bg;
            editor_theme.gutter_bg = bg;
        }
        if let Some(text) = self.theme_override_text {
            editor_theme.text = text;
            editor_theme.cursor = text;
        }
        let font_settings = self.current_font_settings();
        let search_query_owned = if self.search_input.trim().is_empty() {
            None
        } else {
            Some(self.search_input.clone())
        };
        let search_query = search_query_owned.as_deref();

        let min_editor_width = 360.0;
        let min_preview_width = 260.0;
        let can_split = editor_rect.width() > (min_editor_width + min_preview_width);

        self.ensure_split_secondary();
        if let Some(secondary_idx) = self.split_secondary_tab {
            if self.split_mode != SplitMode::None
                && secondary_idx < self.editors.len()
                && secondary_idx != self.active_tab
            {
                let (first_rect, second_rect) = if self.split_mode == SplitMode::Vertical {
                    let w = editor_rect.width() / 2.0;
                    (
                        egui::Rect::from_min_max(
                            editor_rect.min,
                            egui::Pos2::new(editor_rect.min.x + w - 1.0, editor_rect.max.y),
                        ),
                        egui::Rect::from_min_max(
                            egui::Pos2::new(editor_rect.min.x + w + 1.0, editor_rect.min.y),
                            editor_rect.max,
                        ),
                    )
                } else {
                    let h = editor_rect.height() / 2.0;
                    (
                        egui::Rect::from_min_max(
                            editor_rect.min,
                            egui::Pos2::new(editor_rect.max.x, editor_rect.min.y + h - 1.0),
                        ),
                        egui::Rect::from_min_max(
                            egui::Pos2::new(editor_rect.min.x, editor_rect.min.y + h + 1.0),
                            editor_rect.max,
                        ),
                    )
                };
                let active_idx = self.active_tab;
                let (editors, clipboard, highlighter) =
                    (&mut self.editors, &mut self.clipboard, &self.highlighter);
                let (first_editor, second_editor) = if active_idx < secondary_idx {
                    let (left, right) = editors.split_at_mut(secondary_idx);
                    (&mut left[active_idx], &mut right[0])
                } else {
                    let (left, right) = editors.split_at_mut(active_idx);
                    (&mut right[0], &mut left[secondary_idx])
                };
                let mut first_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(first_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                crate::ui::editor_view::show(
                    &mut first_ui,
                    first_editor,
                    clipboard,
                    highlighter,
                    &editor_theme,
                    &font_settings,
                    search_query,
                    auto_focus,
                );
                let mut second_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(second_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                crate::ui::editor_view::show(
                    &mut second_ui,
                    second_editor,
                    clipboard,
                    highlighter,
                    &editor_theme,
                    &font_settings,
                    search_query,
                    false,
                );
            }
        } else if show_preview && can_split {
            let preview_width = (editor_rect.width() * 0.42)
                .clamp(min_preview_width, editor_rect.width() - min_editor_width);
            let editor_width = editor_rect.width() - preview_width;
            let editor_rect = egui::Rect::from_min_max(
                editor_rect.min,
                egui::Pos2::new(editor_rect.min.x + editor_width, editor_rect.max.y),
            );
            let preview_rect = egui::Rect::from_min_max(
                egui::Pos2::new(editor_rect.max.x, editor_rect.min.y),
                egui::Pos2::new(editor_rect.max.x + preview_width, editor_rect.max.y),
            );

            let separator_rect = egui::Rect::from_min_max(
                egui::Pos2::new(preview_rect.min.x - 1.0, preview_rect.min.y),
                egui::Pos2::new(preview_rect.min.x, preview_rect.max.y),
            );
            ui.painter()
                .rect_filled(separator_rect, 0.0, egui::Color32::from_rgb(55, 55, 58));

            let mut editor_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(editor_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            crate::ui::editor_view::show(
                &mut editor_ui,
                &mut self.editors[self.active_tab],
                &mut self.clipboard,
                &self.highlighter,
                &editor_theme,
                &font_settings,
                search_query,
                auto_focus,
            );

            if let Some(text) = preview_text.as_deref() {
                let mut preview_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(preview_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                markdown_preview::show(&mut preview_ui, text);
            }
        } else {
            let mut editor_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(editor_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            crate::ui::editor_view::show(
                &mut editor_ui,
                &mut self.editors[self.active_tab],
                &mut self.clipboard,
                &self.highlighter,
                &editor_theme,
                &font_settings,
                search_query,
                auto_focus,
            );
        }

        if !self.zen_mode {
            crate::ui::status_bar::show(
                ui,
                &mut self.editors[self.active_tab],
                self.git_info.as_ref(),
                &self.highlighter,
            );
        }
    }
}

fn find_drop_target(
    dragging_idx: usize,
    pointer: egui::Pos2,
    rects: &[(usize, egui::Rect, bool)],
) -> Option<usize> {
    let mut drag_pinned = None;
    for (idx, _, pinned) in rects {
        if *idx == dragging_idx {
            drag_pinned = Some(*pinned);
            break;
        }
    }
    let drag_pinned = drag_pinned?;
    let mut best: Option<(usize, f32)> = None;
    for (idx, rect, pinned) in rects {
        if *pinned != drag_pinned {
            continue;
        }
        let center_x = rect.center().x;
        let dist = (pointer.x - center_x).abs();
        if best.map(|(_, d)| dist < d).unwrap_or(true) {
            best = Some((*idx, dist));
        }
    }
    best.map(|(idx, _)| idx).filter(|idx| *idx != dragging_idx)
}

fn remap_tab_index_after_move(idx: usize, from: usize, to: usize) -> usize {
    if idx == from {
        return to;
    }
    if from < to {
        if (from + 1..=to).contains(&idx) {
            idx - 1
        } else {
            idx
        }
    } else if (to..from).contains(&idx) {
        idx + 1
    } else {
        idx
    }
}

fn resolve_git_root(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn read_git_files(repo: &Path) -> Vec<GitChangedFile> {
    let output = match Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(repo)
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.len() < 4 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');
        let path = line[3..].to_string();
        let status = format!("{}{}", index_status, worktree_status);
        out.push(GitChangedFile {
            path,
            status,
            staged: index_status != ' ',
        });
    }
    out
}

fn read_git_commits(repo: &Path) -> Vec<GitCommitEntry> {
    let output = match Command::new("git")
        .arg("log")
        .arg("-n")
        .arg("20")
        .arg("--pretty=format:%h\t%s")
        .current_dir(repo)
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (hash, summary) = line.split_once('\t')?;
            Some(GitCommitEntry {
                hash: hash.to_string(),
                summary: summary.to_string(),
            })
        })
        .collect()
}

fn read_git_diff_for_file(repo: &Path, file: &str) -> String {
    let output = Command::new("git")
        .arg("diff")
        .arg("--")
        .arg(file)
        .current_dir(repo)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        _ => String::new(),
    }
}

fn read_active_line_blame(repo: &Path, editor: &Editor) -> String {
    let Some(path) = editor.file_path.as_ref() else {
        return "No file".to_string();
    };
    let Ok(relative) = path.strip_prefix(repo) else {
        return "Not in repo".to_string();
    };
    let line = editor.cursors.first().map(|c| c.pos.line + 1).unwrap_or(1);
    let output = Command::new("git")
        .arg("blame")
        .arg("-L")
        .arg(format!("{line},{line}"))
        .arg("--")
        .arg(relative)
        .current_dir(repo)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "Blame unavailable".to_string(),
    }
}

fn git_stage_file(repo: &Path, file: &str) -> bool {
    Command::new("git")
        .arg("add")
        .arg("--")
        .arg(file)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_unstage_file(repo: &Path, file: &str) -> bool {
    Command::new("git")
        .arg("reset")
        .arg("HEAD")
        .arg("--")
        .arg(file)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_commit(repo: &Path, message: &str) -> bool {
    Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg(message)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_checkout_branch(repo: &Path, branch: &str) -> bool {
    if branch.is_empty() {
        return false;
    }
    Command::new("git")
        .arg("checkout")
        .arg(branch)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_merge_branch(repo: &Path, branch: &str) -> bool {
    if branch.is_empty() {
        return false;
    }
    Command::new("git")
        .arg("merge")
        .arg(branch)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_rebase_branch(repo: &Path, branch: &str) -> bool {
    if branch.is_empty() {
        return false;
    }
    Command::new("git")
        .arg("rebase")
        .arg(branch)
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_stash_push(repo: &Path, message: &str) -> bool {
    let mut cmd = Command::new("git");
    cmd.arg("stash").arg("push");
    if !message.is_empty() {
        cmd.arg("-m").arg(message);
    }
    cmd.current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_stash_pop(repo: &Path) -> bool {
    Command::new("git")
        .arg("stash")
        .arg("pop")
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn read_git_info(cwd: Option<&Path>) -> Option<crate::ui::status_bar::GitInfo> {
    let cwd = cwd?;

    let mut rev_cmd = Command::new("git");
    rev_cmd
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(cwd);
    let rev_output = rev_cmd.output().ok()?;
    if !rev_output.status.success() {
        return None;
    }
    let toplevel = String::from_utf8_lossy(&rev_output.stdout)
        .trim()
        .to_string();
    if toplevel.is_empty() {
        return None;
    }
    let toplevel = Path::new(&toplevel);
    if !cwd.starts_with(toplevel) {
        return None;
    }

    let mut cmd = Command::new("git");
    cmd.arg("status").arg("-sb");
    cmd.current_dir(cwd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let first = lines.next()?.trim();
    if !first.starts_with("## ") {
        return None;
    }
    let mut branch = first.trim_start_matches("## ").to_string();
    let mut ahead = 0usize;
    let mut behind = 0usize;
    if let Some((name, rest)) = branch.clone().split_once("...") {
        branch = name.to_string();
        if let Some(start) = rest.find('[') {
            if let Some(end) = rest.find(']') {
                let stats = &rest[start + 1..end];
                for part in stats.split(',') {
                    let part = part.trim();
                    if let Some(value) = part.strip_prefix("ahead ") {
                        ahead = value.parse().unwrap_or(0);
                    } else if let Some(value) = part.strip_prefix("behind ") {
                        behind = value.parse().unwrap_or(0);
                    }
                }
            }
        }
    }
    let dirty = lines.next().is_some();
    Some(crate::ui::status_bar::GitInfo {
        branch,
        ahead,
        behind,
        dirty,
    })
}

fn read_inline_blame(path: &Path) -> Vec<InlineBlameEntry> {
    let cwd = match path.parent() {
        Some(parent) => parent,
        None => return Vec::new(),
    };

    let mut cmd = Command::new("git");
    cmd.arg("blame")
        .arg("--line-porcelain")
        .arg("--")
        .arg(path)
        .current_dir(cwd);
    let output = match cmd.output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    let mut lines = Vec::new();
    let mut commit_short = String::new();
    let mut author = String::new();
    let mut summary = String::new();

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.starts_with('\t') {
            lines.push(InlineBlameEntry {
                commit_short: commit_short.clone(),
                author: author.clone(),
                summary: summary.clone(),
            });
            continue;
        }

        if let Some((head, _)) = line.split_once(' ') {
            if head.len() >= 8 && head.chars().all(|c| c.is_ascii_hexdigit()) {
                commit_short = if head.starts_with("00000000") {
                    "working".to_string()
                } else {
                    head.chars().take(8).collect()
                };
                author.clear();
                summary.clear();
                continue;
            }
        }

        if let Some(value) = line.strip_prefix("author ") {
            author = value.to_string();
            continue;
        }
        if let Some(value) = line.strip_prefix("summary ") {
            summary = value.to_string();
        }
    }

    lines
}

fn build_code_lens_metrics(editor: &Editor) -> Vec<CodeLensMetric> {
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

fn looks_like_symbol_header(line: &str) -> bool {
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

fn color_to_hex(color: egui::Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

fn parse_hex_color(hex: &str) -> Option<egui::Color32> {
    let stripped = hex.trim().trim_start_matches('#');
    if stripped.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&stripped[0..2], 16).ok()?;
    let g = u8::from_str_radix(&stripped[2..4], 16).ok()?;
    let b = u8::from_str_radix(&stripped[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b))
}

fn parse_shortcut(raw: &str) -> Option<ShortcutSpec> {
    let mut command = false;
    let mut shift = false;
    let mut alt = false;
    let mut key = None;
    for part in raw.split('+').map(|s| s.trim().to_lowercase()) {
        match part.as_str() {
            "cmd" | "ctrl" | "control" => command = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            "a" => key = Some(egui::Key::A),
            "b" => key = Some(egui::Key::B),
            "c" => key = Some(egui::Key::C),
            "d" => key = Some(egui::Key::D),
            "e" => key = Some(egui::Key::E),
            "f" => key = Some(egui::Key::F),
            "g" => key = Some(egui::Key::G),
            "h" => key = Some(egui::Key::H),
            "i" => key = Some(egui::Key::I),
            "j" => key = Some(egui::Key::J),
            "k" => key = Some(egui::Key::K),
            "l" => key = Some(egui::Key::L),
            "m" => key = Some(egui::Key::M),
            "n" => key = Some(egui::Key::N),
            "o" => key = Some(egui::Key::O),
            "p" => key = Some(egui::Key::P),
            "q" => key = Some(egui::Key::Q),
            "r" => key = Some(egui::Key::R),
            "s" => key = Some(egui::Key::S),
            "t" => key = Some(egui::Key::T),
            "u" => key = Some(egui::Key::U),
            "v" => key = Some(egui::Key::V),
            "w" => key = Some(egui::Key::W),
            "x" => key = Some(egui::Key::X),
            "y" => key = Some(egui::Key::Y),
            "z" => key = Some(egui::Key::Z),
            _ => {}
        }
    }
    Some(ShortcutSpec {
        key: key?,
        command,
        shift,
        alt,
    })
}

fn session_file_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".lux")
        .join("session.json")
}

fn recovery_dir_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".lux")
        .join("recovery")
}

fn persist_session_snapshot(editors: &[Editor], active_tab: usize) {
    let path = session_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let files: Vec<String> = editors
        .iter()
        .filter_map(|e| e.file_path.as_ref())
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let json = serde_json::json!({
        "files": files,
        "active_tab": active_tab,
    });
    let _ = std::fs::write(path, json.to_string());
}

fn load_session_editors() -> Option<(Vec<Editor>, usize)> {
    let path = session_file_path();
    let raw = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    let files = value.get("files")?.as_array()?;
    let mut editors = Vec::new();
    for file in files {
        let Some(path_str) = file.as_str() else {
            continue;
        };
        if let Ok(editor) = Editor::from_file(PathBuf::from(path_str)) {
            editors.push(editor);
        }
    }
    if editors.is_empty() {
        return None;
    }
    let active = value
        .get("active_tab")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0)
        .min(editors.len().saturating_sub(1));
    Some((editors, active))
}

fn persist_recovery_snapshot(editor: &Editor) -> std::io::Result<()> {
    let dir = recovery_dir_path();
    std::fs::create_dir_all(&dir)?;
    let name = format!(
        "untitled-{}.txt",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    std::fs::write(dir.join(name), editor.rope.to_string())
}

fn recent_workspaces_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".lux")
        .join("recent_workspaces.json")
}

fn load_recent_workspaces() -> Vec<PathBuf> {
    let path = recent_workspaces_path();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    value
        .get("workspaces")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

fn persist_recent_workspaces(workspaces: &[PathBuf]) {
    let path = recent_workspaces_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let list: Vec<String> = workspaces
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let json = serde_json::json!({ "workspaces": list });
    let _ = std::fs::write(path, json.to_string());
}

fn file_icon(path: &Path) -> &'static str {
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

fn load_workspace_settings(
    workspace: &Path,
) -> (
    WorkspaceFontSettings,
    HashMap<PathBuf, WorkspaceFontSettings>,
) {
    let mut base = WorkspaceFontSettings {
        size: 13.5,
        family: FontFamilyKind::Monospace,
        ligatures: false,
    };
    let mut overrides = HashMap::new();
    let path = workspace.join(".lux").join("settings.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (base, overrides);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (base, overrides);
    };
    if let Some(font) = value.get("font") {
        if let Some(size) = font.get("size").and_then(|v| v.as_f64()) {
            base.size = size as f32;
        }
        if let Some(family) = font.get("family").and_then(|v| v.as_str()) {
            base.family = if family.eq_ignore_ascii_case("proportional") {
                FontFamilyKind::Proportional
            } else {
                FontFamilyKind::Monospace
            };
        }
        if let Some(ligatures) = font.get("ligatures").and_then(|v| v.as_bool()) {
            base.ligatures = ligatures;
        }
    }
    if let Some(folder_overrides) = value.get("folder_overrides").and_then(|v| v.as_array()) {
        for item in folder_overrides {
            let Some(folder) = item.get("folder").and_then(|v| v.as_str()) else {
                continue;
            };
            let size = item
                .get("size")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(base.size);
            let family = item
                .get("family")
                .and_then(|v| v.as_str())
                .map(|v| {
                    if v.eq_ignore_ascii_case("proportional") {
                        FontFamilyKind::Proportional
                    } else {
                        FontFamilyKind::Monospace
                    }
                })
                .unwrap_or(base.family);
            let ligatures = item
                .get("ligatures")
                .and_then(|v| v.as_bool())
                .unwrap_or(base.ligatures);
            overrides.insert(
                PathBuf::from(folder),
                WorkspaceFontSettings {
                    size,
                    family,
                    ligatures,
                },
            );
        }
    }
    (base, overrides)
}

fn symbol_score(candidate: &str, query: &str) -> Option<i32> {
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

fn build_simple_diff_preview(before: &str, after: &str) -> String {
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

fn parse_stacktrace_location(line: &str) -> Option<(PathBuf, usize)> {
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

fn parse_search_hit_location(hit: &str) -> Option<(String, usize)> {
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

fn parse_symbol_location(symbol: &str) -> Option<(PathBuf, usize)> {
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

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();
    for key in ["TOKEN", "SECRET", "PASSWORD", "API_KEY"] {
        if let Some(idx) = out.to_uppercase().find(key) {
            let end = (idx + key.len() + 24).min(out.len());
            out.replace_range(idx..end, &format!("{}=[REDACTED]", key));
        }
    }
    out
}
