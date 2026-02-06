# Features

## Core Editing
- **High Performance**: Built on Rust+egui for steady 60fps rendering.
- **Syntax Highlighting**: Supports extensive language grammars via `syntect`.
- **Undo/Redo**: Robust history stack for all text operations.
- **Multi-Cursor**: (Implied) Support for multiple cursors and selections.
- **Search & Replace**: Fast find/replace within the active buffer.

## Workbench
- **Command Palette** (`Ctrl+Shift+P`): Central hub for all editor actions.
- **Sidebars**:
  - **Explorer**: File tree navigation.
  - **Search**: Workspace-wide text search.
  - **Git**: Source control status and operations.
  - **Debug**: Variable watch and call stack (experimental).
- **Dock Panels**:
  - **Terminal**: Integrated shell emulator.
  - **Output**: Logs from build tools and plugins.
  - **Problems**: Diagnostics and linter errors.

## Developer Tools
- **LSP Support**:
  - Auto-completion
  - Go to Definition
  - Formatting
  - Diagnostics
- **Git Integration**:
  - Viewing changed files.
  - Diff views.
  - Committing changes directly from the editor.
- **Tasks**: Configurable workspace tasks (Build, Test, Lint).

## Experimental / Advanced
- **Collaboration**: Real-time peer editing session support (`collab_enabled`).
- **Settings Sync**: Sync preferences across machines.
- **Plugins**: Extension system for custom commands and behavior.
