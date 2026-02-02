# Lux Editor

A fast, free text editor inspired by Sublime Text, built with Rust and egui.

## Features

- **Fast and Lightweight**: Built with Rust for maximum performance
- **Syntax Highlighting**: Supports multiple programming languages with syntax highlighting powered by syntect
- **Multiple Tabs**: Work with multiple files simultaneously using tabs
- **Command Palette**: Quick access to all commands with `Ctrl+Shift+P` (or `Cmd+Shift+P` on macOS)
- **Find and Replace**: Search text and replace with `Ctrl+F` and `Ctrl+H`
- **Go to Line**: Jump to any line with `Ctrl+G`
- **Clipboard Integration**: Full copy, paste, and cut support
- **File Management**: Open, save, and save as functionality
- **Modified File Detection**: Visual indicators for unsaved changes
- **Undo/Redo**: Full undo and redo support

## Installation

### From Source

To build Lux Editor from source, you'll need to have Rust installed. If you don't have Rust, you can install it from [rustup.rs](https://rustup.rs/).

1. Clone the repository:
   ```bash
   git clone https://github.com/lcajigasm/lux-edit.git
   cd lux-edit
   ```

2. Build and run:
   ```bash
   cargo run --release
   ```

## Usage

### Keyboard Shortcuts

#### File Operations
- `Ctrl+N` (or `Cmd+N` on macOS) - New tab
- `Ctrl+O` (or `Cmd+O` on macOS) - Open file
- `Ctrl+S` (or `Cmd+S` on macOS) - Save file
- `Ctrl+Shift+S` (or `Cmd+Shift+S` on macOS) - Save as
- `Ctrl+W` (or `Cmd+W` on macOS) - Close tab
- Middle-click on tab - Close tab

#### Editing
- `Ctrl+Z` (or `Cmd+Z` on macOS) - Undo
- `Ctrl+Y` or `Ctrl+Shift+Z` (or `Cmd+Y` or `Cmd+Shift+Z` on macOS) - Redo
- `Ctrl+A` (or `Cmd+A` on macOS) - Select all
- `Ctrl+C` (or `Cmd+C` on macOS) - Copy
- `Ctrl+X` (or `Cmd+X` on macOS) - Cut
- `Ctrl+V` (or `Cmd+V` on macOS) - Paste

#### Navigation
- `Ctrl+F` (or `Cmd+F` on macOS) - Find
- `Ctrl+H` (or `Cmd+H` on macOS) - Find and replace (Note: On macOS, `Cmd+H` hides the window, use Command Palette instead)
- `Ctrl+G` (or `Cmd+G` on macOS) - Go to line
- `Esc` - Close search/replace/go-to-line bar

#### Commands
- `Ctrl+Shift+P` (or `Cmd+Shift+P` on macOS) - Open command palette

## Dependencies

Lux Editor is built with the following key dependencies:

- **[eframe](https://github.com/emilk/egui)** (v0.29) - Cross-platform GUI framework
- **[ropey](https://github.com/cessen/ropey)** (v1.6) - Fast text buffer library
- **[syntect](https://github.com/trishume/syntect)** (v5.2) - Syntax highlighting engine
- **[rfd](https://github.com/PolyMeilex/rfd)** (v0.15) - Native file dialogs
- **[arboard](https://github.com/1Password/arboard)** (v3.4) - Clipboard support

## Contributing

Contributions are welcome! If you'd like to contribute to Lux Editor:

1. Fork the repository
2. Create a new branch for your feature or bug fix
3. Make your changes
4. Test your changes thoroughly
5. Submit a pull request

## License

This project is licensed under the MIT License.

## Acknowledgments

- Inspired by [Sublime Text](https://www.sublimetext.com/)
- Built with [egui](https://github.com/emilk/egui), an immediate mode GUI library
- Syntax highlighting powered by [syntect](https://github.com/trishume/syntect)
