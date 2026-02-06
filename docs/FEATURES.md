# Feature Overview

Lux Editor includes:

- Multi-tab editing with split layouts and sidebar panels.
- Syntax highlighting and syntax detection by extension or shebang.
- Search and replace in file and workspace.
- Command palette and menu actions for common workflows.
- Integrated terminal/output/problems views.
- Git panel support for status, diff, and history actions.
- LSP-assisted completion, formatting, diagnostics, and code navigation.
- Plugin and scripting hooks for custom commands.
- Workspace-level settings for fonts, theme density, and keymaps.

For implementation references, see:
- `src/app.rs`
- `src/editor.rs`
- `src/ui/editor_view.rs`
- `src/ui/command_palette.rs`
- `src/ui/status_bar.rs`
