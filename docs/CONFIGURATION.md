# Configuration

## Workspace Settings

Lux Editor supports per-workspace preferences such as:

- Font size and font family.
- Ligatures on/off.
- UI density.
- Keymap presets and custom bindings.

## Project Conventions

When opening files, Lux Editor can read:

- `.editorconfig` for indentation and line endings.
- `.gitattributes` for line-ending defaults.

## Formatter Defaults

Default formatters are selected per detected language (for example `rustfmt`, `black`, `gofmt`, `prettier`) and can be overridden.

## Files and Paths

Project/session metadata is stored under `.lux/` in the workspace.
