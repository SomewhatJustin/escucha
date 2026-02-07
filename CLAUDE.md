# Claude Development Guide

## Project Overview

**escucha** is a hold-to-talk speech-to-text app for Linux, written in Rust. It watches for a configurable key press, records audio with `arecord`, transcribes locally using Whisper.cpp, and pastes the result into the active window.

## Architecture

```
src/
├── main.rs          CLI entry point (--gui, --check, --list-devices)
├── lib.rs           Module exports
├── audio.rs         arecord wrapper + WAV file management
├── config.rs        INI config loading (rust-ini)
├── gui.rs           GTK4/libadwaita troubleshooting UI
├── input.rs         evdev keyboard device management + key resolution
├── paste.rs         Multi-method text pasting (xdotool/wtype/wl-copy)
├── preflight.rs     Environment validation (permissions, tools, dirs)
├── service.rs       Main dictation service + daemon mode
└── transcribe.rs    Whisper.cpp model loading + transcription
```

## Key Components

### Service Loop (`service.rs`)

- Creates `DictationService` with config, device path, key, and paste config
- Spawns evdev reader thread that filters KEY events for target key
- Main loop receives Press/Release events via mpsc channel
- Press: starts arecord to temp WAV file
- Release: stops recording, transcribes, pastes, cleans up
- Supports graceful shutdown via AtomicBool flag

### GUI (`gui.rs`)

- GTK4 + libadwaita with async message passing
- Service runs in background thread, sends status/text/error messages
- Shows visual status (spinner/icon), transcription results, and error toasts
- Preflight checks run on startup - critical failures show "Fix Input Permissions" button
- Button click runs `pkexec usermod -aG input $USER`, then restarts app with `sg input -c ...`

### Preflight (`preflight.rs`)

- Validates: input device access, arecord, paste tool, curl, writable directories
- Returns structured report with pass/fail, severity (Critical/Warning), message, and hints
- Used by daemon (bail on critical failures), GUI (show fix button), and CLI (--check)

### Paste Methods (`paste.rs`)

- **xdotool**: X11 direct typing or clipboard paste with xclip
- **wtype**: Wayland direct typing or clipboard paste with wl-copy
- **wl-copy**: Clipboard-only (no auto-paste) for GNOME Wayland
- Auto-detects display server and available tools

## Config File

Location: `~/.config/escucha/config.ini`

```ini
[escucha]
key = KEY_RIGHTCTRL
keyboard_device = auto
model = base.en
language = en
paste_method = auto
paste_hotkey = ctrl+v
clipboard_paste = auto
clipboard_paste_delay_ms = 75
log_file = ~/.local/state/escucha/escucha.log
log_level = info
```

## Testing

All modules have unit tests. Run with:
```bash
cargo test
cargo clippy -- -D warnings
```

Test coverage:
- 53 unit tests across all modules
- Preflight checks (critical/warning detection, formatting)
- Service callbacks and status transitions
- Input device filtering and key resolution
- Paste hotkey parsing (wtype argument generation)
- WAV loading (int/float samples, stereo→mono conversion)
- Config loading (defaults, partial configs, type conversion)

## Common Tasks

### Adding a new key

1. Add to `parse_key_name()` in `input.rs`
2. Update README with key in "Available keys" section

### Adding a new paste method

1. Add variant to `PasteMethod` enum in `paste.rs`
2. Implement paste function (signature: `fn paste_X(text: &str, config: &PasteConfig) -> Result<()>`)
3. Add to `pick_paste_method()` auto-detection
4. Add to `paste_text()` match statement

### Adding a new preflight check

1. Write check function in `preflight.rs` (signature: `fn check_X() -> CheckResult`)
2. Add to `check_environment()` checks vec
3. If critical failure needs special handling (like input fix button), update GUI service thread

### Modifying GUI layout

- Widget tree starts at `build_ui()` in `gui.rs`
- Status area: icon stack (icon/spinner), status label, status detail, fix button
- Transcription area: scrollable label with word-wrap
- Toast overlay handles error messages
- CSS in const `CSS` at top of file

## Error Handling

- Use `anyhow::Result<T>` for fallible functions
- Use `anyhow::bail!()` for early returns with error messages
- Wrap errors with context: `.context("Failed to do X")?`
- GUI errors sent as `ServiceMessage::Error` → shown as toasts
- Daemon errors logged to stderr via `log::error!()`

## Dependencies

**Core:**
- `whisper-rs`: Whisper.cpp Rust bindings
- `evdev`: Linux input device access
- `hound`: WAV file reading
- `gtk4` + `libadwaita`: GUI framework
- `clap`: CLI argument parsing
- `rust-ini`: Config file parsing

**System tools:**
- `arecord`: Audio recording (from alsa-utils)
- `xdotool` + `xclip`: X11 text pasting
- `wtype` + `wl-copy`: Wayland text pasting
- `curl`: Model downloads from Hugging Face
- `pkexec`: Permission elevation for group changes
- `sg`: Group activation without logout

## Conventions

- Use `log::info!()` / `log::warn!()` / `log::error!()` for logging
- Keep functions focused and under 100 lines when possible
- Document public functions with `///` doc comments
- Add unit tests for pure functions (parsing, validation, transformations)
- Use `#[cfg(test)]` modules at bottom of each file
- Follow Rust 2024 edition idioms (let-chains, etc.)

## Debugging

```bash
# Enable debug logging
RUST_LOG=debug cargo run -- --gui

# Check environment
cargo run -- --check

# List input devices
cargo run -- --list-devices

# Test specific module
cargo test audio::tests::

# Watch clippy during development
cargo watch -x "clippy -- -D warnings"
```

## Release Checklist

- [ ] All tests pass: `cargo test`
- [ ] Clippy clean: `cargo clippy -- -D warnings`
- [ ] `--check` shows expected results
- [ ] GUI launches and shows status correctly
- [ ] Permission fix button works (adds to input group + restarts)
- [ ] Transcription works with default model
- [ ] Update README if adding features/config options
- [ ] Update CLAUDE.md with architectural changes

## Known Issues & Quirks

1. **Input group restart**: Using `sg input` to avoid logout requirement. Falls back to "log out and back in" message if `sg` not available.

2. **GNOME Wayland**: Doesn't support virtual keyboard protocol, so `wtype` can't auto-paste. Falls back to clipboard-only (`wl-copy`).

3. **Model downloads**: First run downloads ~142MB model. No progress bar in daemon mode (progress shown in GUI via status messages).

4. **Temp files**: WAV files written to system temp dir and explicitly cleaned up after transcription. If app crashes during recording, temp files may remain.

5. **Signal handling**: Daemon mode sets up SIGTERM/SIGINT handlers. GUI uses window close event for cleanup.

6. **Group membership caching**: Linux caches group membership at login. `sg` command activates group for child process tree without full logout.
