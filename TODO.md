# Roadmap / TODO

## Minimap
- Render diff hunks, search matches, and selection overlays in the minimap.
- Provide hover preview tooltips that show surrounding code when the cursor is over the minimap.
- Support configurable width, opacity, and a toggle per tab/workspace.
- Implement click-to-fold markers and keyboard shortcuts tied to the minimap viewport.

## Tabs & Title Bar
- Add drag-and-drop reordering, pinning, and tab context menus (close others, reopen closed tab).
- Show dirty indicators via subtle colored strips and provide breadcrumb/path display above the editor.
- Integrate workspace indicators (branch name, remote status) near the tab row.

## Editor UX
- Implement folding gutters, inline blame annotations, and code-lens metrics.
- Add indent guides, column rulers, multi-caret column selection, and macro recording.
- Integrate snippets, auto-completion, diagnostics, and formatting via LSP.
- Support rich multi-language syntax highlighting and coloring for major ecosystems (C/C++, Rust, Go, Python, JS/TS, HTML/CSS, Java, C#, SQL, shell, etc.).

## Git Support
- Add embedded Git panel with staging, commit history, diff view, and blame.
- Show real-time SCM indicators (modified, added, removed lines) in the gutter and status bar.
- Allow checkout, merge, rebase, stash operations directly from the UI.

## Command Palette & Menus
- Implement fuzzy scoring with prioritization, categories, and history.
- Allow extensions to register commands and palette providers.
- Expose palette actions for Git, debugging, tasks, and custom scripts.

## Status Bar
- Make segments interactive (encoding toggle, indentation mode, CRLF/LF switch).
- Surface diagnostics count, Git branch, background tasks, and notification badges.

## Panels & Layout
- Add sidebars (Explorer, Search, Git, Debug) with resizable panes.
- Support split editors (vertical/horizontal), layout presets, zen mode, and focus mode.
- Include terminal, output, and problems panels with drag-to-dock behavior.

## Theme & Customization
- Provide full theme switcher with JSON import/export, per-color overrides, and UI density controls.
- Allow font settings (size, family, ligatures) per workspace.
- Offer keymap presets (VSCode, Sublime, JetBrains) and user-defined bindings.

## Performance & Platform
- Add file watchers for hot reload, autosave, crash recovery, and session restore.
- Provide telemetry opt-in, update channel, and plugin sandboxing.
- Optimize large file handling with asynchronous parsing and incremental rendering.

## Extensions & API
- Design plugin architecture for syntax packages, formatters, build tasks, and keymaps.
- Provide scripting API (Lua/JS/Rust) with safe sandboxing and lifecycle hooks.
- Ship a marketplace/registry integration for installing/updating extensions.
