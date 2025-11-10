# Keep - Terminal Task Manager

A lightweight, keyboard-driven task manager for the terminal, built with Rust.

![Keep Task Manager](https://img.shields.io/badge/rust-1.70%2B-orange.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

<img width="1345" height="591" alt="Screenshot_20251110_160358" src="https://github.com/user-attachments/assets/585d6478-e628-468f-b00c-c2235c5b390d" />

## Features

- **📅 Daily Task Scheduling** - Organize tasks by date with start and end times
- **⚠️ Overdue Tracking** - Automatically detects and highlights overdue tasks
- **📝 Notes View** - Quick-access notepad for capturing ideas and thoughts
- **⌨️ Keyboard-Driven** - Vim-style navigation (hjkl supported)
- **💾 Local Storage** - All data stored locally in JSON format
- **🎨 Clean TUI** - Beautiful terminal interface powered by Ratatui

## Installation

### From Source

```bash
git clone https://github.com/sudo-rsingh/keep.git
cd keep
cargo build --release
```

The binary will be available at `target/release/keep`

### Run Directly

```bash
cargo run
```

## Usage

### Basic Controls

**Task View:**
- `n` - Create new task
- `e` - Edit selected task
- `Space` - Toggle task completion
- `d` - Delete task
- `↑/↓` or `j/k` - Navigate tasks
- `←/→` or `h/l` - Previous/Next day
- `Tab` - Switch to Notes view
- `q` - Quit

**Notes View:**
- Type freely to edit notes
- `Arrow keys` - Navigate cursor
- `Home/End` - Jump to line start/end
- `Ctrl+S` - Save notes
- `Tab` - Switch to Task view
- `q` - Quit

**Add/Edit Mode:**
- `Tab` - Switch between Task/Start Time/End Time fields
- `Enter` - Save task
- `Esc` - Cancel

### Time Format

Enter times in 24-hour format: `HH:MM` (e.g., `09:30`, `14:00`, `23:45`)

## Data Storage

Keep stores all data in `~/.keep_tasks.json`. The file contains:
- All tasks with their dates, times, and completion status
- Your notes

You can back up this file to preserve your data.

## Tech Stack

- **[Rust](https://www.rust-lang.org/)** - Systems programming language
- **[Ratatui](https://github.com/ratatui-org/ratatui)** - Terminal UI library
- **[Crossterm](https://github.com/crossterm-rs/crossterm)** - Cross-platform terminal manipulation
- **[Serde](https://serde.rs/)** - Serialization framework
- **[Chrono](https://github.com/chronotope/chrono)** - Date and time library

## Planned Features

See [FEATURE_IDEAS.md](FEATURE_IDEAS.md) for a list of potential enhancements including:
- Priority levels
- Task duration display
- Weekly view
- Tags and categories
- Recurring tasks

## Contributing

Contributions are welcome! Feel free to:
- Report bugs
- Suggest features
- Submit pull requests

## License

MIT License - feel free to use this project however you'd like.

## Author

Built by Rakshit while learning Rust and terminal UI development.

---

**Why Keep?**

Keep is designed for developers and terminal enthusiasts who want a simple, distraction-free way to manage daily tasks without leaving the command line. No cloud sync, no bloat—just a fast, local task manager that respects your workflow.
