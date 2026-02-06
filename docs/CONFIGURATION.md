# Configuration

Lux Editor is highly configurable. Settings are currently stored in the workspace metadata or applied via conventions.

## Appearance
- **Theme**: Supports presets like `Monokai` (`src/ui/editor_view.rs`).
- **UI Density**: Adjusts the scale and padding of UI elements.
- **Window Background**: Customizable background color for the application frame.

## Editor Settings
- **Font**: Custom font family (Monospace, etc.) and size.
- **Ligatures**: Enable/disable font ligatures (if supported by font).
- **Auto-Save**: Configurable interval (default: 5.0 seconds).
- **Formatting**:
  - `format_on_save`: Automatically format file when saving.
  - `format_on_type`: (Experimental) Format whilst typing.

## Terminal
- **Profiles**: Pre-configured shell profiles for `Bash`, `Zsh`, `sh`.
- **Custom Shell**: Override the default shell command and arguments.

## Project Conventions
The editor respects standard configuration files:
- **`.editorconfig`**: For indentation style (tabs vs spaces) and size.
- **`.gitattributes`**: For line ending normalization.

## System & Debug
- **Telemetry**: Opt-in telemetry for usage stats.
- **Update Channel**: Stable, Beta, or Nightly updates.
- **Safe Mode**: Disables plugins and external scripts for troubleshooting.
