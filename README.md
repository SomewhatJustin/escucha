# escucha

Hold a key, speak, release. Transcribes locally with Whisper and pastes into the focused field.

What it does:
- Watches a hold-to-talk key (default: Right Ctrl)
- Records audio while the key is held
- Transcribes locally with Whisper.cpp
- Pastes into the active app

## Requirements

**Must have:**
- Rust 1.75+ (2024 edition)
- `alsa-utils` (provides `arecord`)
- Access to `/dev/input/event*` (add user to `input` group)

**Paste tool (one required):**
- X11: `xdotool` + `xclip`
- Wayland: `ydotool` + `wl-clipboard` (works on all compositors including KDE)
- Wayland (alternative): `wtype` + `wl-clipboard` (for compositors with virtual keyboard support)

**Optional:**
- Qt6 system tray integration packages (usually installed with `qt6-base` + `qt6-declarative`)

### System packages

**Fedora:**
```bash
sudo dnf install -y rust cargo alsa-utils qt6-qtbase-devel qt6-qtdeclarative-devel \
  wl-clipboard ydotool xdotool xclip curl
```

**Ubuntu/Debian:**
```bash
sudo apt install -y cargo rustc alsa-utils qt6-base-dev qt6-declarative-dev \
  wl-clipboard ydotool xdotool xclip curl
```

**Arch (manual dependencies):**
```bash
sudo pacman -S --needed base-devel git rust alsa-utils \
  qt6-base qt6-declarative qt6-tools \
  wl-clipboard ydotool wtype xdotool xclip curl
```

## Arch / AUR-Style Install

This repository includes a `PKGBUILD`, so you can emulate the AUR user flow directly:

```bash
git clone https://github.com/SomewhatJustin/escucha.git
cd escucha
makepkg -si
```

`makepkg -si` means:
- `-s`: install missing dependencies from repos
- `-i`: install the package after build

After install:
- Launch tray app: `escucha --gui`
- KRunner launcher: `Escucha`

## Build & Install

### Quick install (installs dependencies automatically)

```bash
git clone https://github.com/somewhatjustin/escucha.git
cd escucha
./install.sh
```

The installer will:
- Check for and install missing dependencies (`ydotool`, `wl-clipboard`, `alsa-utils`, etc.)
- Start the `ydotoold` daemon (required for ydotool)
- Build the release binary
- Install to `~/.local/bin/escucha`
- Install systemd services (escucha + ydotoold)
- Optionally add you to the `input` group

### Manual install

```bash
cargo build --release
make install
```

### Input permissions

The app needs access to `/dev/input/event*` devices. Add your user to the `input` group:

```bash
sudo usermod -aG input $USER
```

Then **log out and back in** (or use the tray app's "Fix Input Permissions" action to auto-restart).

## Usage

### Check environment

Before running, verify your system is configured correctly:

```bash
escucha --check
```

This validates input device access, arecord, paste tools, and directories.

### Structured diagnostics (JSON)

For automation and CI-style debugging loops:

```bash
escucha --diagnose
```

Prints a structured JSON report and exits `0`/`1`.

### Headless smoke test

Runs a non-interactive startup/audio smoke test and prints JSON:

```bash
escucha --smoke-test
```

This is intended for repeatable machine-driven iteration (run, parse, patch, rerun).

### Run as daemon (default)

```bash
escucha
```

Runs in the background. Hold Right Ctrl and speak to transcribe.

### Tray App

```bash
escucha --gui
```

Runs as a system tray app and shows status/error notifications.

### List input devices

```bash
escucha --list-devices
```

## Configuration

Config file: `~/.config/escucha/config.ini`

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

**Options:**
- `key`: Linux input key name (e.g., `KEY_RIGHTCTRL`, `KEY_FN`, `KEY_CAPSLOCK`)
- `keyboard_device`: `auto` or specific `/dev/input/eventX`
- `model`: Whisper model name (`tiny.en`, `base.en`, `small.en`, `medium.en`, `large`)
- `language`: Language code (`en`, `es`, `fr`, `de`, etc.)
- `paste_method`: `auto`, `xdotool`, `ydotool`, `wtype`, or `wl-copy`
- `paste_hotkey`: Keyboard shortcut for clipboard paste (`ctrl+v`, `ctrl+shift+v`)
- `clipboard_paste`: `auto`, `on`, or `off` (auto uses clipboard on Wayland)
- `clipboard_paste_delay_ms`: Delay between clipboard copy and paste simulation
- `log_level`: `debug`, `info`, `warn`, `error`

### Available keys

Common dictation keys:
- `KEY_RIGHTCTRL` / `KEY_LEFTCTRL`
- `KEY_RIGHTALT` / `KEY_LEFTALT`
- `KEY_CAPSLOCK`
- `KEY_FN` (if your keyboard emits it)
- `KEY_F13` through `KEY_F24`
- `KEY_PAUSE`, `KEY_SCROLLLOCK`, `KEY_INSERT`

Use `escucha --list-devices` to see your keyboard and confirm detected device names.

## Whisper models

Models are automatically downloaded on first run to `~/.local/share/escucha/models/`.

**Model sizes:**
- `tiny.en`: ~75 MB, fastest, least accurate
- `base.en`: ~142 MB, good balance (default)
- `small.en`: ~466 MB, better accuracy
- `medium.en`: ~1.5 GB, high accuracy
- `large`: ~3 GB, best accuracy, multilingual

English-only models (`*.en`) are faster and more accurate for English.

## Wayland notes

**ydotool daemon:** The `ydotool` paste method requires the `ydotoold` daemon to be running. The installer automatically sets this up as a systemd user service.

If you installed manually, start it with:
```bash
systemctl --user enable --now ydotoold.service
```

Or run it manually:
```bash
ydotoold &
```

**For compositors without virtual keyboard support (KDE, GNOME):** The app uses `ydotool` which works universally via `/dev/uinput`.

**For compositors with virtual keyboard support (Sway, Hyprland):** Both `wtype` and `ydotool` work.

## Troubleshooting

**"Setup required: input devices"**
- Add user to input group: `sudo usermod -aG input $USER`
- Log out and back in
- Or use the tray app's "Fix Input Permissions" action

**"arecord not found"**
- Install `alsa-utils`: `sudo dnf install alsa-utils`

**"No paste tool found"**
- X11: Install `xdotool` and `xclip`
- Wayland (KDE/most compositors): Install `ydotool` and `wl-clipboard`
- Wayland (Sway/Hyprland): Install `wtype` and `wl-clipboard`

**Key not detected**
- Run `escucha --list-devices` to verify input access
- Use `escucha --list-devices` and set a known key like `KEY_RIGHTCTRL` in config
- Some keyboards don't emit `KEY_FN` - use `KEY_RIGHTCTRL` or a function key

**Paste fails**
- X11: Check that `xdotool` and `xclip` work: `echo "test" | xclip -selection clipboard && xdotool key ctrl+v`
- Wayland (ydotool): Check that `ydotool` and `wl-copy` work: `echo "test" | wl-copy && ydotool key 29:47`
- Wayland (wtype): Check that `wtype` and `wl-copy` work: `echo "test" | wl-copy && wtype -M ctrl -k v -m ctrl`
- Try increasing `clipboard_paste_delay_ms` in config

**Model download fails**
- Check internet connection
- Verify `curl` is installed
- Models are fetched from huggingface.co

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy -- -D warnings

# Run with logging
RUST_LOG=debug cargo run -- --gui

# Check environment
cargo run -- --check

# Structured diagnostics JSON
cargo run -- --diagnose

# Headless smoke test JSON
cargo run -- --smoke-test

# One-command closed loop
./scripts/diagnostics-loop.sh
```

## AUR Maintainer Notes

When preparing an AUR update:

```bash
# after updating PKGBUILD values (pkgver/pkgrel/source/checksums)
makepkg --printsrcinfo > .SRCINFO
```

AUR submission itself uses a separate AUR git repo containing `PKGBUILD` and `.SRCINFO`.

## License

MIT
