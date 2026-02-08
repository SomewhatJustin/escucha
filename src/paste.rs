use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasteMethod {
    Xdotool,
    Wtype,
    Ydotool,
    WlCopy,
}

impl PasteMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            PasteMethod::Xdotool => "xdotool",
            PasteMethod::Wtype => "wtype",
            PasteMethod::Ydotool => "ydotool",
            PasteMethod::WlCopy => "wl-copy",
        }
    }
}

impl std::fmt::Display for PasteMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct PasteConfig {
    pub method: PasteMethod,
    pub hotkey: String,
    pub clipboard_paste: String,
    pub clipboard_paste_delay_ms: u32,
}

/// Auto-detect the best paste method for the current environment.
pub fn pick_paste_method(setting: &str) -> Result<PasteMethod> {
    match setting {
        "xdotool" => return Ok(PasteMethod::Xdotool),
        "wtype" => return Ok(PasteMethod::Wtype),
        "ydotool" => return Ok(PasteMethod::Ydotool),
        "wl-copy" => return Ok(PasteMethod::WlCopy),
        _ => {}
    }

    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let is_x11 = std::env::var("DISPLAY").is_ok();

    if is_wayland {
        // Prefer ydotool (works on all compositors including KDE)
        if is_available("ydotool") && (ydotool_socket_available() || ensure_ydotoold_running()) {
            return Ok(PasteMethod::Ydotool);
        }
        // wtype only works on compositors that support virtual keyboard
        if is_available("wtype") {
            return Ok(PasteMethod::Wtype);
        }
        if is_available("wl-copy") {
            log::warn!(
                "ydotool/wtype not found; falling back to wl-copy (clipboard only). \
                 Install ydotool for automatic pasting."
            );
            return Ok(PasteMethod::WlCopy);
        }
    }

    if is_x11 && is_available("xdotool") {
        return Ok(PasteMethod::Xdotool);
    }

    bail!("No paste tool found. Install ydotool + wl-copy (Wayland) or xdotool (X11).")
}

fn is_available(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

fn ydotool_socket_path_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(path) = std::env::var("YDOTOOL_SOCKET") {
        paths.push(PathBuf::from(path));
    }
    paths.push(PathBuf::from("/tmp/.ydotool_socket"));
    paths
}

pub fn ydotool_socket_available() -> bool {
    ydotool_socket_path_candidates().iter().any(|p| p.exists())
}

fn ydotoold_service_active() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", "ydotoold.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub fn ydotool_ready() -> bool {
    ydotool_socket_available() || ydotoold_service_active()
}

pub fn uinput_accessible() -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/uinput")
        .is_ok()
}

/// Best-effort startup of ydotoold for desktop sessions where the user has installed a user unit.
pub fn ensure_ydotoold_running() -> bool {
    if ydotool_ready() {
        return true;
    }

    // First-run friendly path: persistently enable and start the user service.
    let started = run_systemctl_user(["enable", "--now", "ydotoold.service"]);
    if !started {
        // Fallback to a plain start for environments where enable is restricted.
        let _ = run_systemctl_user(["start", "ydotoold.service"]);
    }
    std::thread::sleep(std::time::Duration::from_millis(200));

    ydotool_ready()
}

fn run_systemctl_user<const N: usize>(args: [&str; N]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn validated_current_user() -> Result<String> {
    let user = std::env::var("USER").unwrap_or_default();
    if user.is_empty() {
        bail!("Could not determine current username");
    }
    if !user
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("Refusing to use unsafe username in privileged setup");
    }
    Ok(user)
}

/// Privileged best-effort repair for systems where /dev/uinput is not user-accessible.
/// This installs an udev rule, ensures input-group membership, and reloads udev.
pub fn repair_uinput_permissions() -> Result<()> {
    let user = validated_current_user()?;
    let script = format!(
        "set -e; \
         install -d /etc/udev/rules.d; \
         cat > /etc/udev/rules.d/80-escucha-uinput.rules <<'EOF'\n\
KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\", OPTIONS+=\"static_node=uinput\"\n\
EOF\n\
         usermod -aG input {user}; \
         udevadm control --reload-rules || true; \
         udevadm trigger --name-match=uinput || true; \
         if [ -e /dev/uinput ]; then chgrp input /dev/uinput || true; chmod 0660 /dev/uinput || true; fi"
    );

    let status = Command::new("pkexec")
        .args(["/bin/sh", "-c", &script])
        .status()
        .context("Failed to run pkexec for /dev/uinput repair")?;

    if !status.success() {
        bail!("Privileged setup was denied or failed");
    }

    Ok(())
}

/// End-to-end paste setup repair used by the GUI action:
/// 1) try starting ydotoold as-is
/// 2) if unavailable and /dev/uinput is blocked, request privileged repair
/// 3) retry ydotoold startup
pub fn repair_paste_setup() -> Result<()> {
    if ensure_ydotoold_running() {
        return Ok(());
    }

    if !uinput_accessible() {
        repair_uinput_permissions()?;
    }

    if ensure_ydotoold_running() {
        Ok(())
    } else {
        bail!("Could not start ydotoold after repair");
    }
}

/// Paste text using the configured method.
/// Appends a trailing space so consecutive dictations don't run together.
pub fn paste_text(text: &str, config: &PasteConfig) -> Result<()> {
    let text = format!("{text} ");
    match config.method {
        PasteMethod::Xdotool => paste_xdotool(&text, config),
        PasteMethod::Wtype => paste_wtype(&text, config),
        PasteMethod::Ydotool => paste_ydotool(&text, config),
        PasteMethod::WlCopy => paste_wl_copy_only(&text),
    }
}

fn paste_xdotool(text: &str, config: &PasteConfig) -> Result<()> {
    if should_use_clipboard(&config.clipboard_paste) {
        clipboard_paste_x11(text, &config.hotkey, config.clipboard_paste_delay_ms)
    } else {
        // Direct typing with xdotool
        let status = Command::new("xdotool")
            .args(["type", "--delay", "1", text])
            .status()
            .context("Failed to run xdotool")?;

        if !status.success() {
            bail!("xdotool type failed with status {status}");
        }
        Ok(())
    }
}

fn paste_wtype(text: &str, config: &PasteConfig) -> Result<()> {
    if should_use_clipboard(&config.clipboard_paste) {
        clipboard_paste_wayland(text, &config.hotkey, config.clipboard_paste_delay_ms)
    } else {
        let status = Command::new("wtype")
            .arg(text)
            .status()
            .context("Failed to run wtype")?;

        if !status.success() {
            // Fallback to clipboard paste
            log::warn!("wtype direct typing failed, falling back to clipboard paste");
            clipboard_paste_wayland(text, &config.hotkey, config.clipboard_paste_delay_ms)
        } else {
            Ok(())
        }
    }
}

fn paste_ydotool(text: &str, config: &PasteConfig) -> Result<()> {
    if should_use_clipboard(&config.clipboard_paste) {
        clipboard_paste_ydotool(text, &config.hotkey, config.clipboard_paste_delay_ms)
    } else {
        // Direct typing with ydotool
        let status = Command::new("ydotool")
            .args(["type", text])
            .status()
            .context("Failed to run ydotool")?;

        if !status.success() {
            // Fallback to clipboard paste
            log::warn!("ydotool direct typing failed, falling back to clipboard paste");
            clipboard_paste_ydotool(text, &config.hotkey, config.clipboard_paste_delay_ms)
        } else {
            Ok(())
        }
    }
}

/// Clipboard-only paste: copies text to clipboard via wl-copy and logs a notice.
fn paste_wl_copy_only(text: &str) -> Result<()> {
    let status = Command::new("wl-copy")
        .arg(text)
        .status()
        .context("Failed to copy to clipboard with wl-copy")?;

    if !status.success() {
        bail!("wl-copy failed");
    }

    log::info!("Text copied to clipboard (paste with Ctrl+V)");
    Ok(())
}

fn should_use_clipboard(setting: &str) -> bool {
    setting == "auto" || setting == "on"
}

fn clipboard_paste_x11(text: &str, hotkey: &str, delay_ms: u32) -> Result<()> {
    // Copy to clipboard using xclip or xsel
    let status = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait()
        })
        .context("Failed to copy to clipboard with xclip")?;

    if !status.success() {
        bail!("xclip failed");
    }

    std::thread::sleep(std::time::Duration::from_millis(delay_ms as u64));

    // Simulate paste hotkey
    let status = Command::new("xdotool")
        .args(["key", hotkey])
        .status()
        .context("Failed to simulate paste with xdotool")?;

    if !status.success() {
        bail!("xdotool key failed");
    }
    Ok(())
}

fn clipboard_paste_wayland(text: &str, hotkey: &str, delay_ms: u32) -> Result<()> {
    // Copy to clipboard using wl-copy
    let status = Command::new("wl-copy")
        .arg(text)
        .status()
        .context("Failed to copy to clipboard with wl-copy")?;

    if !status.success() {
        bail!("wl-copy failed");
    }

    std::thread::sleep(std::time::Duration::from_millis(delay_ms as u64));

    // Simulate paste hotkey with wtype
    let keys = parse_hotkey_to_wtype(hotkey);
    let status = Command::new("wtype")
        .args(&keys)
        .status()
        .context("Failed to simulate paste with wtype")?;

    if !status.success() {
        log::warn!(
            "wtype key simulation failed (compositor may not support virtual keyboard). Text copied to clipboard - paste manually with Ctrl+V"
        );
        return Ok(()); // Don't fail - clipboard copy succeeded
    }
    Ok(())
}

fn clipboard_paste_ydotool(text: &str, hotkey: &str, delay_ms: u32) -> Result<()> {
    // Copy to clipboard using wl-copy
    let status = Command::new("wl-copy")
        .arg(text)
        .status()
        .context("Failed to copy to clipboard with wl-copy")?;

    if !status.success() {
        bail!("wl-copy failed");
    }

    std::thread::sleep(std::time::Duration::from_millis(delay_ms as u64));

    // Simulate paste hotkey with ydotool
    // Format: ydotool key KEYCODE:1 KEYCODE:1 KEYCODE:0 KEYCODE:0
    // where :1 = press, :0 = release
    let args = parse_hotkey_to_ydotool(hotkey);
    let status = Command::new("ydotool")
        .arg("key")
        .args(&args)
        .status()
        .context("Failed to simulate paste with ydotool")?;

    if !status.success() {
        bail!("ydotool key failed");
    }
    Ok(())
}

/// Parse a hotkey like "ctrl+v" or "ctrl+shift+v" to wtype args.
fn parse_hotkey_to_wtype(hotkey: &str) -> Vec<String> {
    let mut args = Vec::new();
    let parts: Vec<&str> = hotkey.split('+').collect();

    for (i, part) in parts.iter().enumerate() {
        let lowered = part.to_lowercase();
        let key = match lowered.as_str() {
            "ctrl" => "ctrl",
            "shift" => "shift",
            "alt" => "alt",
            "super" | "meta" => "super",
            _ => &lowered,
        };

        if i < parts.len() - 1 {
            args.push("-M".to_string());
            args.push(key.to_string());
        } else {
            args.push("-k".to_string());
            args.push(key.to_string());
        }
    }

    // Release modifiers in reverse
    for part in parts[..parts.len().saturating_sub(1)].iter().rev() {
        let lowered = part.to_lowercase();
        let key = match lowered.as_str() {
            "ctrl" => "ctrl",
            "shift" => "shift",
            "alt" => "alt",
            "super" | "meta" => "super",
            _ => &lowered,
        };
        args.push("-m".to_string());
        args.push(key.to_string());
    }

    args
}

/// Map a key name to a Linux evdev key code for ydotool.
fn key_name_to_code(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "ctrl" => Some("29"),            // KEY_LEFTCTRL
        "shift" => Some("42"),           // KEY_LEFTSHIFT
        "alt" => Some("56"),             // KEY_LEFTALT
        "super" | "meta" => Some("125"), // KEY_LEFTMETA
        "v" => Some("47"),               // KEY_V
        "c" => Some("46"),               // KEY_C
        "a" => Some("30"),               // KEY_A
        "z" => Some("44"),               // KEY_Z
        _ => None,
    }
}

/// Parse a hotkey like "ctrl+v" to ydotool key arguments.
/// ydotool format: each arg is KEYCODE:STATE where 1=press, 0=release.
/// For ctrl+v: "29:1" "47:1" "47:0" "29:0"
fn parse_hotkey_to_ydotool(hotkey: &str) -> Vec<String> {
    let parts: Vec<&str> = hotkey.split('+').collect();
    let mut codes: Vec<&str> = Vec::new();

    for part in &parts {
        if let Some(code) = key_name_to_code(part) {
            codes.push(code);
        } else {
            log::warn!("Unknown key in hotkey: {}", part);
        }
    }

    let mut args = Vec::new();

    // Press all keys in order
    for code in &codes {
        args.push(format!("{code}:1"));
    }

    // Release all keys in reverse order
    for code in codes.iter().rev() {
        args.push(format!("{code}:0"));
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paste_method_display() {
        assert_eq!(PasteMethod::Xdotool.to_string(), "xdotool");
        assert_eq!(PasteMethod::Wtype.to_string(), "wtype");
        assert_eq!(PasteMethod::Ydotool.to_string(), "ydotool");
        assert_eq!(PasteMethod::WlCopy.to_string(), "wl-copy");
    }

    #[test]
    fn test_pick_paste_method_explicit() {
        assert_eq!(pick_paste_method("xdotool").unwrap(), PasteMethod::Xdotool);
        assert_eq!(pick_paste_method("wtype").unwrap(), PasteMethod::Wtype);
        assert_eq!(pick_paste_method("ydotool").unwrap(), PasteMethod::Ydotool);
        assert_eq!(pick_paste_method("wl-copy").unwrap(), PasteMethod::WlCopy);
    }

    #[test]
    fn test_parse_hotkey_to_ydotool_ctrl_v() {
        let args = parse_hotkey_to_ydotool("ctrl+v");
        // Press Ctrl, press V, release V, release Ctrl
        assert_eq!(args, vec!["29:1", "47:1", "47:0", "29:0"]);
    }

    #[test]
    fn test_parse_hotkey_to_ydotool_ctrl_shift_v() {
        let args = parse_hotkey_to_ydotool("ctrl+shift+v");
        assert_eq!(args, vec!["29:1", "42:1", "47:1", "47:0", "42:0", "29:0"]);
    }

    #[test]
    fn test_parse_hotkey_ctrl_v_wtype() {
        let args = parse_hotkey_to_wtype("ctrl+v");
        assert_eq!(args, vec!["-M", "ctrl", "-k", "v", "-m", "ctrl"]);
    }

    #[test]
    fn test_parse_hotkey_ctrl_shift_v_wtype() {
        let args = parse_hotkey_to_wtype("ctrl+shift+v");
        assert_eq!(
            args,
            vec![
                "-M", "ctrl", "-M", "shift", "-k", "v", "-m", "shift", "-m", "ctrl"
            ]
        );
    }

    #[test]
    fn test_should_use_clipboard() {
        assert!(should_use_clipboard("auto"));
        assert!(should_use_clipboard("on"));
        assert!(!should_use_clipboard("off"));
    }

    #[test]
    fn test_paste_config_clone() {
        let config = PasteConfig {
            method: PasteMethod::Xdotool,
            hotkey: "ctrl+v".into(),
            clipboard_paste: "auto".into(),
            clipboard_paste_delay_ms: 75,
        };
        let cloned = config.clone();
        assert_eq!(cloned.method, PasteMethod::Xdotool);
        assert_eq!(cloned.hotkey, "ctrl+v");
    }
}
