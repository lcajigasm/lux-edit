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

mod dock_panel;
mod git_panel;
mod menu;
mod render;
mod search_bars;
mod sidebar;
mod tabs;

// --- Lux Dark palette ---
// Base: deep indigo-black  Accent: warm amber gold (Lux = light)
pub(super) const WINDOW_BG: egui::Color32 = egui::Color32::from_rgb(18, 18, 27);
pub(super) const MENU_BG: egui::Color32 = egui::Color32::from_rgb(22, 22, 33);
pub(super) const MENU_STROKE: egui::Stroke = egui::Stroke {
    width: 1.0,
    color: egui::Color32::from_rgb(40, 40, 58),
};
pub(super) const TAB_BAR_BG: egui::Color32 = egui::Color32::from_rgb(13, 13, 20);
pub(super) const TAB_ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(18, 18, 27);
pub(super) const TAB_INACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(22, 22, 33);
pub(super) const TAB_HOVER_BG: egui::Color32 = egui::Color32::from_rgb(28, 28, 42);
pub(super) const TAB_HEIGHT: f32 = 33.0;
pub(super) const TAB_MIN_WIDTH: f32 = 120.0;
pub(super) const TAB_MAX_WIDTH: f32 = 220.0;
pub(super) const TAB_PADDING_X: f32 = 12.0;
pub(super) const TAB_CLOSE_SIZE: f32 = 14.0;
pub(super) const ACCENT_COLOR: egui::Color32 = egui::Color32::from_rgb(240, 180, 66); // amber gold
pub(super) const ACTIVITY_BAR_BG: egui::Color32 = egui::Color32::from_rgb(13, 13, 20);
pub(super) const SIDEBAR_BG: egui::Color32 = egui::Color32::from_rgb(16, 16, 24);

// GitInfo moved to ui::status_bar

#[derive(Clone, Copy)]
pub(super) enum TabAction {
    Activate(usize),
    Close(usize),
    CloseOthers(usize),
    ReopenClosed,
    TogglePin(usize, bool),
    Reorder(usize, usize),
    MoveToSplit(usize, bool),
    NewTab,
}

pub(super) struct LspWorkerOutput {
    path: PathBuf,
    snapshot: LspSnapshot,
    request: RequestKind,
}

#[derive(Clone, Debug)]
pub(super) enum AsyncCommandTarget {
    TerminalPrimary,
    TerminalSecondary,
    Task,
    RunConfig,
}

#[derive(Clone, Debug)]
pub(super) struct AsyncCommandResult {
    target: AsyncCommandTarget,
    label: String,
    stdout: String,
    stderr: String,
    success: bool,
    error: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ExternalChangePrompt {
    tab_idx: usize,
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct DeletePrompt {
    tab_idx: usize,
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct RefactorPreview {
    tab_idx: usize,
    title: String,
    diff_preview: String,
    original_text: String,
}

#[derive(Clone, Debug)]
pub(super) struct WorkspaceTask {
    name: String,
    command: String,
}

#[derive(Clone, Debug)]
pub(super) struct RunConfiguration {
    name: String,
    command: String,
    env_overrides: String,
}

#[derive(Clone, Debug)]
pub(super) struct TerminalProfile {
    name: String,
    shell: String,
    theme_hint: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum LogLevel {
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
pub(super) struct WorkspaceFontSettings {
    size: f32,
    family: FontFamilyKind,
    ligatures: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarTab {
    Explorer,
    Search,
    Git,
    Debug,
    Collab,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SplitMode {
    None,
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DockPanelTab {
    Terminal,
    Output,
    Problems,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DockSide {
    Bottom,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeymapPreset {
    Vscode,
    Sublime,
    JetBrains,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UpdateChannel {
    Stable,
    Beta,
    Nightly,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ShortcutSpec {
    key: egui::Key,
    command: bool,
    shift: bool,
    alt: bool,
}

#[derive(Default)]
pub(super) struct DebugState {
    breakpoints: HashMap<PathBuf, HashSet<usize>>,
    watch_input: String,
    watches: Vec<String>,
    call_stack: Vec<String>,
}

#[derive(Default)]
pub(super) struct CollabState {
    enabled: bool,
    session_id: String,
    review_mode: bool,
    note_input: String,
    notes: HashMap<PathBuf, Vec<(usize, String)>>,
    peer_cursors: Vec<String>,
}

#[derive(Default)]
pub(super) struct RunnerState {
    tasks: Vec<WorkspaceTask>,
    new_task_name: String,
    new_task_command: String,
    run_configs: Vec<RunConfiguration>,
    new_run_config_name: String,
    new_run_config_command: String,
    new_run_config_env: String,
}

pub(super) struct TerminalState {
    profiles: Vec<TerminalProfile>,
    profile_idx: usize,
    split_panes: bool,
    input_secondary: String,
    log_secondary: Vec<String>,
    input: String,
    log: Vec<String>,
    output_log: Vec<String>,
    output_filter: String,
    problems_filter: String,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self {
            profiles: vec![
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
            profile_idx: 0,
            split_panes: false,
            input_secondary: String::new(),
            log_secondary: Vec::new(),
            input: String::new(),
            log: Vec::new(),
            output_log: Vec::new(),
            output_filter: String::new(),
            problems_filter: String::new(),
        }
    }
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
    pending_delete_confirm: Option<DeletePrompt>,
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
    sidebar_expanded_dirs: HashSet<PathBuf>,
    quick_open_query: String,
    recent_workspaces: Vec<PathBuf>,
    file_ops_target: String,
    file_ops_message: String,
    deleted_file_backup: Option<(PathBuf, PathBuf)>,
    debug: DebugState,
    runner: RunnerState,
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
    collab: CollabState,
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
    terminal: TerminalState,
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
            pending_delete_confirm: None,
            closed_tabs: Vec::new(),
            dragging_tab: None,
            editor_theme: EditorThemeKind::LuxDark,
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
            sidebar_expanded_dirs: HashSet::new(),
            quick_open_query: String::new(),
            recent_workspaces,
            file_ops_target: String::new(),
            file_ops_message: String::new(),
            deleted_file_backup: None,
            debug: DebugState {
                call_stack: vec!["main()".to_string()],
                ..Default::default()
            },
            runner: RunnerState {
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
                ..Default::default()
            },
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
            collab: CollabState::default(),
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
            terminal: TerminalState::default(),
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
}

// → show_git_panel_ui in git_panel.rs
// → show_activity_bar + show_left_sidebar in sidebar.rs
// → render_dock_panel_contents + show_dock_panel in dock_panel.rs
// → show_menu_bar in menu.rs
// → show_tab_bar in tabs.rs
// → show_search_bar + show_goto_line_bar + show_breadcrumbs in search_bars.rs

// → render_main_ui in render.rs

impl eframe::App for LuxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Lux Dark visuals — amber accent, no cyan focus rings
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = None;
        visuals.selection.bg_fill = egui::Color32::from_rgba_premultiplied(240, 180, 66, 55);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(240, 180, 66));
        visuals.widgets.active.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(240, 180, 66));
        visuals.widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 155));
        visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 58));
        visuals.widgets.open.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(240, 180, 66));
        visuals.window_fill = egui::Color32::from_rgb(18, 18, 27);
        visuals.panel_fill = egui::Color32::from_rgb(18, 18, 27);
        ctx.set_visuals(visuals);
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

        if let Some(prompt) = self.pending_delete_confirm.clone() {
            let mut confirm_delete = false;
            let mut cancel_delete = false;
            egui::Window::new("Confirm Safe Delete")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Move this file to trash?\n{}",
                        prompt.path.to_string_lossy()
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Move To Trash").clicked() {
                            confirm_delete = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel_delete = true;
                        }
                    });
                });
            if confirm_delete {
                self.delete_active_file(prompt.tab_idx, prompt.path.as_path());
                self.pending_delete_confirm = None;
            } else if cancel_delete {
                self.pending_delete_confirm = None;
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
                        for line in self.terminal.output_log.iter().rev().take(200) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_search_hit_location_handles_windows_style_paths() {
        let hit = r"C:\repo\src\main.rs:42:let x = 1;";
        let parsed = parse_search_hit_location(hit).unwrap();
        assert_eq!(parsed.0, r"C:\repo\src\main.rs");
        assert_eq!(parsed.1, 42);
    }

    #[test]
    fn parse_symbol_location_handles_windows_style_paths() {
        let symbol = r"C:\repo\src\lib.rs:12: fn run()";
        let parsed = parse_symbol_location(symbol).unwrap();
        assert_eq!(parsed.0, PathBuf::from(r"C:\repo\src\lib.rs"));
        assert_eq!(parsed.1, 12);
    }

    #[test]
    fn parse_stacktrace_location_supports_line_col_format() {
        let base = std::env::temp_dir().join(format!("lux-edit-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        let file = base.join("stack.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();

        let line = format!("{}:23:9", file.display());
        let parsed = parse_stacktrace_location(&line).unwrap();
        assert_eq!(parsed.0, file);
        assert_eq!(parsed.1, 23);
    }
}
