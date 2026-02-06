# Architecture

## Core Structures

The application state is centralized in the `LuxApp` struct (`src/app.rs`), which orchestrates the following subsystems:

- **Editor Engine**: Managing text buffers, cursors, and selections via `Editor` (`src/editor.rs`) and `ropey`.
- **UI Layer**: Built with `egui` / `eframe`. Rendering logic is split into `src/ui/` modules.
- **Command System**: Centralized dispatch via `CommandPalette` (`src/ui/command_palette.rs`).
- **Plugins**: Extension host capability in `src/plugin.rs`.

### `LuxApp` (State Container)
Holds the global state including:
- `editors`: Vector of open `Editor` tabs.
- `highlighter`: Syntax highlighting engine (`syntect`).
- `git_panel`: State for the Git integration sidebar.
- `command_palette`: State for the overlay command menu.
- Configuration fields (fonts, themes, terminal profiles).

### `Editor` (Buffer Management)
Handles:
- Text storage using `Rope` for efficient editing of large files.
- Cursor positions and multiple selections.
- Undo/Redo history stacks.
- Syntax highlighting caching.

## Data Flow

1. **Event Loop**: `eframe` calls `LuxApp::update` every frame.
2. **Input Handling**: Keyboard shortcuts are intercepted and mapped to `CommandId`s.
3. **Command Dispatch**: Commands are executed, modifying `LuxApp` or the active `Editor`.
4. **Async Tasks**:
   - **LSP**: Runs in a background thread, communicating via crossbeam channels (`lsp_rx`).
   - **Git**: File status and diffs are polled periodically to avoid blocking the UI.
5. **Rendering**: The UI draws the current state. `egui` is immediate mode, so the entire UI is reconstructed each frame based on the data.

## Subsystems

### Syntax Highlighting
Powered by `syntect`. It uses a shared `SyntaxSet` and `ThemeSet` to map text scopes to colors.

### LSP Integration
- Located in `src/lsp.rs`.
- Maintains a snapshot of the document to sync with language servers.
- Supports diagnostics, formatting, and definition lookup.

### Git Integration
- Located in `src/ui/status_bar.rs` (status) and `src/app.rs` (panel logic).
- Executes `git` CLI commands (`status`, `diff`, `commit`) and parses output.
