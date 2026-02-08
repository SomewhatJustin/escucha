#!/bin/bash
set -e

INSTALL_DIR="${HOME}/.local/bin"
SYSTEMD_DIR="${HOME}/.config/systemd/user"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"

echo "==> escucha installer"
echo ""

# Detect distro
if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO=$ID
else
    echo "Warning: Cannot detect distro, skipping dependency check"
    DISTRO="unknown"
fi

# Check/install dependencies based on distro
echo "==> Checking dependencies..."
MISSING_DEPS=()

check_command() {
    if ! command -v "$1" &> /dev/null; then
        MISSING_DEPS+=("$2")
    fi
}

check_command "cargo" "rust cargo"
check_command "arecord" "alsa-utils"
check_command "wl-copy" "wl-clipboard"

# Check for paste tool (prefer ydotool for KDE compatibility)
if [ "$XDG_CURRENT_DESKTOP" = "KDE" ]; then
    # KDE requires ydotool (wtype doesn't work due to lack of virtual keyboard support)
    if ! command -v ydotool &> /dev/null; then
        MISSING_DEPS+=("ydotool")
    fi
else
    # Other compositors: prefer wtype, fall back to ydotool
    if ! command -v wtype &> /dev/null && ! command -v ydotool &> /dev/null; then
        MISSING_DEPS+=("wtype")
    fi
fi

if [ ${#MISSING_DEPS[@]} -gt 0 ]; then
    echo "Missing dependencies: ${MISSING_DEPS[*]}"
    echo ""

    case "$DISTRO" in
        fedora)
            echo "Installing dependencies with dnf..."
            sudo dnf install -y "${MISSING_DEPS[@]}"
            ;;
        ubuntu|debian)
            echo "Installing dependencies with apt..."
            sudo apt update
            sudo apt install -y "${MISSING_DEPS[@]}"
            ;;
        arch|manjaro)
            echo "Installing dependencies with pacman..."
            sudo pacman -S --needed "${MISSING_DEPS[@]}"
            ;;
        *)
            echo "Please install manually:"
            for dep in "${MISSING_DEPS[@]}"; do
                echo "  - $dep"
            done
            echo ""
            read -p "Continue anyway? (y/N) " -n 1 -r
            echo
            if [[ ! $REPLY =~ ^[Yy]$ ]]; then
                exit 1
            fi
            ;;
    esac
fi

echo "==> Building escucha..."
cargo build --release

echo "==> Installing binary to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
install -m755 target/release/escucha "$INSTALL_DIR/escucha"

echo "==> Installing desktop entry and icon..."
mkdir -p "$APP_DIR" "$ICON_DIR"
install -m644 io.github.escucha.desktop "$APP_DIR/io.github.escucha.desktop"
install -m644 assets/io.github.escucha.svg "$ICON_DIR/io.github.escucha.svg"

echo "==> Installing systemd service to $SYSTEMD_DIR..."
mkdir -p "$SYSTEMD_DIR"
cat > "$SYSTEMD_DIR/escucha.service" <<EOF
[Unit]
Description=Escucha speech-to-text service
After=graphical-session.target ydotoold.service

[Service]
Type=simple
ExecStart=$INSTALL_DIR/escucha
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

# Set up ydotoold service if ydotool is installed
if command -v ydotool &> /dev/null; then
    echo "==> Setting up ydotoold daemon service..."
    cat > "$SYSTEMD_DIR/ydotoold.service" <<EOF
[Unit]
Description=ydotool daemon for input automation
After=graphical-session.target

[Service]
Type=simple
ExecStart=$(which ydotoold)
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

    systemctl --user daemon-reload
    systemctl --user enable ydotoold.service
    systemctl --user start ydotoold.service

    echo "ydotoold service installed and started"
fi

echo "==> Checking input group permissions..."
if ! groups | grep -q "\binput\b"; then
    echo ""
    echo "Warning: You are not in the 'input' group."
    echo "This is required to access /dev/input devices."
    echo ""
    read -p "Add yourself to the input group now? (Y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        sudo usermod -aG input "$USER"
        echo "Added to input group. You'll need to log out and back in."
        echo "(Or use the tray app's 'Fix Input Permissions' action to restart with the group active)"
    fi
fi

echo ""
echo "==> Installation complete!"
echo ""
echo "Test the environment:"
echo "  escucha --check"
echo ""
echo "Run the tray app:"
echo "  escucha --gui"
echo ""
echo "Enable as a service:"
echo "  systemctl --user enable --now escucha.service"
echo ""
