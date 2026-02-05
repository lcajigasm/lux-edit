# Roadmap / TODO

## Tabs & Title Bar

## Editor UX
- Implement inline blame annotations and code-lens metrics.
- Add indent guides, column rulers, multi-caret column selection, and macro recording.
- Integrate snippets, auto-completion, diagnostics, and formatting via LSP.
- Support rich multi-language syntax highlighting and coloring for major ecosystems (C/C++, Rust, Go, Python, JS/TS, HTML/CSS, Java, C#, SQL, shell, etc.).

## Git Support
- Add embedded Git panel with staging, commit history, diff view, and blame.
- Show real-time SCM indicators (modified, added, removed lines) in the gutter and status bar.
- Allow checkout, merge, rebase, stash operations directly from the UI.

## Command Palette & Menus
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

## File Management
- Add project explorer with file icons, quick open, and recent workspaces list.
- Implement file operations: rename/move/copy/delete, new file/folder, duplicate.
- Support file watchers with external change detection and merge/overwrite prompts.
- Add workspace-level settings and per-folder overrides.

## Search & Navigation
- Global search with regex, case sensitivity, and file include/exclude filters.
- Search results panel with previews and multi-file replace.
- Symbol search (document/workspace) with fuzzy ranking.
- Jump to definition/references/implementations (LSP).

## Refactoring
- Rename symbol, extract method/variable, inline variable, organize imports.
- Safe refactor previews with diff/rollback per file.
- Code actions with quick fixes and auto-import suggestions.

## Debugging & Tasks
- Integrated debugger UI with breakpoints, watch, and call stack.
- Task runner panel to configure build/test/lint tasks.
- Run configurations per workspace with environment overrides.

## Terminal & Output
- Built-in terminal with multiple profiles, split panes, and theming.
- Output/Problems panels with filtering and copyable logs.
- Linkable stack traces to jump to files/lines.

## Diagnostics & Formatting
- Diagnostics list with severity filters and per-language toggles.
- On-save and on-type formatting with per-language formatter selection.
- Lint rules overrides at workspace and folder scope.

## Collaboration
- Live share / pair programming sessions with cursor tracking.
- Comments/annotations and review mode (inline notes).
- Session snapshots for handoffs.

## Accessibility
- High-contrast themes, scalable UI, and font size presets.
- Full keyboard navigation with visible focus indicators.
- Screen reader labels for key UI components.

## Workspace & Projects
- Multi-root workspaces with per-root settings.
- Project-specific gitignore patterns and search scope controls.
- Workspace trust model for running scripts/extensions.

## Security & Privacy
- Safe mode for untrusted projects (restricted scripts/extensions).
- Secrets scanning and redaction in logs.
- Optional telemetry configuration with explicit consent.

## Documentation & Help
- Built-in help for shortcuts and features (cheat sheet).
- Interactive onboarding/tutorial mode.
- Troubleshooting panel for diagnostics and logs.

## Packaging & Distribution
- Cross-platform installers and auto-updater.
- Portable mode and CLI entry point for opening files/projects.
- App configuration export/import for team setups.

## Testing & QA
- End-to-end UI test harness with snapshot-driven assertions.
- Golden-file tests for rendering and syntax highlighting.
- Performance benchmarks for startup time, typing latency, and large file load.
- Crash/telemetry triage workflow with regression tracking.

## CI/CD
- CI pipeline for lint, tests, and release artifacts on major platforms.
- Signed binaries and notarization (macOS) + code-signing (Windows).
- Release channels (stable/beta/nightly) with automated changelogs.

## Internationalization
- Locale-aware UI strings with extraction and translation tooling.
- RTL layout support and bidirectional text handling.
- Per-language input/IME testing and accessibility checks.

## Settings & Sync
- Settings sync across devices with conflict resolution.
- Profiles for per-project and per-role settings (dev/design/ops).
- Secure storage for tokens, SSH keys, and secrets.

## Onboarding & Templates
- Starter workspace templates (web app, CLI, library, doc site).
- Command palette guided tours and first-run setup flow.
- Quick actions for opening recent folders and cloning repos.

## Observability
- Structured logging with log levels and per-module filtering.
- User-facing diagnostics bundle export for support tickets.
- Health checks for extensions and background services.

## Compatibility
- Import settings from VSCode, Sublime, and JetBrains formats.
- Read and apply `.editorconfig` and `.gitattributes`.
- Respect per-language formatting conventions and toolchains.
