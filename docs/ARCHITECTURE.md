# Architecture

## Main Modules

- `src/main.rs`: startup and app bootstrapping.
- `src/app.rs`: application state, menus, actions, workflow orchestration.
- `src/editor.rs`: text buffer model, cursor/selection, editing operations.
- `src/syntax.rs`: syntax detection and highlighting integration.
- `src/lsp.rs`: LSP snapshot and request helpers.
- `src/plugin.rs`: plugin model and runtime hooks.
- `src/ui/*`: UI rendering for editor surface, command palette, status bar, markdown preview.

## Data Flow

1. UI events trigger actions in `App`.
2. `App` updates active `Editor` state.
3. Background tasks (for example LSP or indexing) push results back through channels.
4. UI widgets render current state each frame.
