#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV="$ROOT_DIR/.venv"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_NAME="escucha.service"

NO_SERVICE=0

usage() {
  echo "Usage: $0 [--no-service]"
}

for arg in "$@"; do
  case "$arg" in
    --no-service)
      NO_SERVICE=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      usage >&2
      exit 1
      ;;
  esac
done

OS_ID=""
OS_LIKE=""
if [[ -f /etc/os-release ]]; then
  . /etc/os-release
  OS_ID="${ID:-}"
  OS_LIKE="${ID_LIKE:-}"
fi

print_deps_hint() {
  case "$OS_ID" in
    fedora)
      echo "Install deps: sudo dnf install -y python3.12 python3.12-devel python3.12-tkinter alsa-utils wl-clipboard ydotool xdotool wtype"
      ;;
    debian|ubuntu)
      echo "Install deps: sudo apt install -y python3 python3-venv python3-tk alsa-utils wl-clipboard ydotool xdotool wtype"
      ;;
    arch)
      echo "Install deps: sudo pacman -S --needed python alsa-utils wl-clipboard ydotool xdotool wtype tk"
      ;;
    *)
      echo "Install deps: python3, alsa-utils (arecord), wl-clipboard, and one of ydotool/wtype/xdotool"
      ;;
  esac
}

PYTHON=""
for candidate in python3.12 python3.11 python3.10 python3; do
  if command -v "$candidate" >/dev/null 2>&1; then
    PYTHON="$candidate"
    break
  fi
done

if [[ -z "$PYTHON" ]]; then
  echo "No suitable Python found (need python3.10+)." >&2
  exit 1
fi

if [[ -x "$VENV/bin/python" ]]; then
  CURRENT_VER="$("$VENV/bin/python" -c 'import sys;print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
  TARGET_VER="$("$PYTHON" -c 'import sys;print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
  if [[ "$CURRENT_VER" != "$TARGET_VER" ]]; then
    echo "Existing venv uses Python $CURRENT_VER but installer will use $TARGET_VER." >&2
    echo "Please remove $VENV and re-run install.sh." >&2
    exit 1
  fi
fi

"$PYTHON" -m venv "$VENV"
"$VENV/bin/pip" install --upgrade pip
"$VENV/bin/pip" install -r "$ROOT_DIR/requirements.txt"

mkdir -p "$SERVICE_DIR"
cp "$ROOT_DIR/systemd/$SERVICE_NAME" "$SERVICE_DIR/$SERVICE_NAME"

mkdir -p "$HOME/.config/escucha"

missing=()
for cmd in arecord; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    missing+=("$cmd")
  fi
done

if ! command -v ydotool >/dev/null 2>&1 && ! command -v wtype >/dev/null 2>&1 && ! command -v xdotool >/dev/null 2>&1; then
  missing+=("ydotool|wtype|xdotool")
fi

if ! command -v wl-copy >/dev/null 2>&1; then
  missing+=("wl-copy")
fi

if [[ "${#missing[@]}" -gt 0 ]]; then
  echo "Missing commands: ${missing[*]}" >&2
  print_deps_hint >&2
fi

if [[ "$NO_SERVICE" -eq 0 ]] && command -v systemctl >/dev/null 2>&1; then
  systemctl --user daemon-reload
  systemctl --user enable --now "$SERVICE_NAME"
  echo "Installed and started $SERVICE_NAME"
else
  echo "Skipping systemd service install."
  echo "Run manually with: $VENV/bin/python -m escucha"
fi

echo "If key events aren't detected, add your user to the input group:"
echo "  sudo usermod -aG input $USER"
