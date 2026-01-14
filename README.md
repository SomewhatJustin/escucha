# escucha

Hold a key, speak, release. The app transcribes locally with Whisper and pastes into the focused field.

What it does:
- Watches a hold-to-talk key (default `KEY_FN`).
- Records from the default mic while the key is held.
- Transcribes locally with Whisper (`faster-whisper`).
- Pastes into the active app (`ydotool`, `xdotool`, or `wtype`).

## Requirements

Must have:
- Python 3.10+ (recommended: 3.11 or 3.12 for prebuilt wheels)
- `arecord` (ALSA utils)
- Access to `/dev/input/event*` (usually add user to `input` group or a udev rule)

Paste tool (pick one):
- X11: `xdotool`
- Wayland (GNOME): `ydotool` + `ydotoold` (recommended)
- Wayland (non-GNOME): `wtype`

Recommended on Wayland:
- `wl-copy` (clipboard; used for fast pasting with `ydotool`)

Optional:
- GUI: `tkinter` (`python3-tkinter` / `python3-tk`)

### System packages

Fedora:

```bash
sudo dnf install -y python3.12 python3.12-devel python3.12-tkinter alsa-utils wl-clipboard ydotool xdotool wtype
```

Ubuntu/Debian:

```bash
sudo apt install -y python3 python3-venv python3-tk alsa-utils wl-clipboard ydotool xdotool wtype
```

Arch:

```bash
sudo pacman -S --needed python alsa-utils wl-clipboard ydotool xdotool wtype tk
```

## Install

```bash
git clone https://github.com/somewhatjustin/escucha.git
cd escucha
./install.sh
```

The installer creates a virtualenv, installs Python deps, and installs a user systemd service.

To skip the systemd service:

```bash
./install.sh --no-service
```

## Background service

Check status:

```bash
systemctl --user status escucha.service
```

Start on boot without login (optional):

```bash
sudo loginctl enable-linger "$USER"
```

## Configuration

Config file: `~/.config/escucha/config.ini`

```ini
[dictate]
key = KEY_FN
keyboard_device = auto
model = base.en
language = en
paste_method = auto
paste_hotkey = auto
clipboard_paste = auto
ydotool_key_delay_ms = 1
log_file = default
log_level = info
clipboard_paste_delay_ms = 75
```

Notes:
- `key`: any Linux input key name (see `/usr/include/linux/input-event-codes.h`).
- `keyboard_device`: `auto` or a specific `/dev/input/eventX` (use the GUI key test to find it).
- `paste_method`: `auto`, `xdotool`, `wtype`, or `ydotool`.
  - GNOME Wayland does not support `wtype` (virtual keyboard protocol).
  - GNOME Wayland: use `ydotool` and run `ydotoold` with uinput access.
  - `ydotool` prefers clipboard paste via `wl-copy` for instant insertion.
- `paste_hotkey`: clipboard paste hotkey (`auto`, `ctrl+v`, or `ctrl+shift+v`).
- `clipboard_paste`: `auto` or `off` to disable clipboard paste (forces typing).
- `clipboard_paste_delay_ms`: delay between `wl-copy` and paste hotkey (Wayland reliability).
- `ydotool_key_delay_ms`: delay between ydotool key events (lower is faster).
- `log_file`: `default` to log to `~/.local/state/escucha/escucha.log`, or empty to disable.
- `log_level`: `debug`, `info`, `warning`, `error`.

### Wayland (GNOME) with ydotool

Install and enable the daemon as root so it can access `/dev/uinput`:

```bash
sudo dnf install -y ydotool
sudo tee /etc/systemd/system/ydotoold.service >/dev/null <<'EOF'
[Unit]
Description=ydotool daemon for Wayland input
After=systemd-user-sessions.service

[Service]
Type=simple
ExecStart=/usr/bin/ydotoold -p /run/user/$(id -u)/.ydotool_socket -P 0660 -o $(id -u):$(id -g)
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable --now ydotoold
```

Then set:

```ini
paste_method = ydotool
```

If you use a non-default socket, set `YDOTOOL_SOCKET` in your environment to match.

### Ghostty (optional)

Ghostty defaults to `ctrl+shift+v` for paste. To make `ctrl+v` work:

```ini
keybind = ctrl+v=paste_from_clipboard
```

Add that to `~/.config/ghostty/config` and reload Ghostty.

## Run manually

```bash
./.venv/bin/python -m escucha
```

Launch GUI:

```bash
./.venv/bin/python -m escucha --gui
```

Use the GUI "Key test" section to see what key codes your keyboard emits.

List input devices:

```bash
./.venv/bin/python -m escucha --list-devices
```

## Troubleshooting

- If no events are received, you likely need input permissions.
- If `KEY_FN` is not emitted by your keyboard, set a different key (e.g., `KEY_F13`).
- If `evdev` fails to build, install build deps (e.g., `kernel-headers`, `gcc`, and `python3-devel`).
- If paste fails on Wayland, verify `ydotoold` is running and `YDOTOOL_SOCKET` points to the correct socket.
