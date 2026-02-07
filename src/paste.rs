use anyhow::{Context, Result, bail};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasteMethod {
    Xdotool,
    Wtype,
    WlCopy,
}

impl PasteMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            PasteMethod::Xdotool => "xdotool",
            PasteMethod::Wtype => "wtype",
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
        "wl-copy" => return Ok(PasteMethod::WlCopy),
        _ => {}
    }

    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let is_x11 = std::env::var("DISPLAY").is_ok();

    if is_wayland {
        if is_available("wtype") {
            return Ok(PasteMethod::Wtype);
        }
        if is_available("wl-copy") {
            log::warn!(
                "wtype not found; falling back to wl-copy (clipboard only). \
                 Install wtype for automatic pasting."
            );
            return Ok(PasteMethod::WlCopy);
        }
    }

    if is_x11 && is_available("xdotool") {
        return Ok(PasteMethod::Xdotool);
    }

    bail!("No paste tool found. Install wtype + wl-copy (Wayland) or xdotool (X11).")
}

fn is_available(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

/// Paste text using the configured method.
pub fn paste_text(text: &str, config: &PasteConfig) -> Result<()> {
    match config.method {
        PasteMethod::Xdotool => paste_xdotool(text, config),
        PasteMethod::Wtype => paste_wtype(text, config),
        PasteMethod::WlCopy => paste_wl_copy_only(text),
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
        bail!("wtype key failed");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paste_method_display() {
        assert_eq!(PasteMethod::Xdotool.to_string(), "xdotool");
        assert_eq!(PasteMethod::Wtype.to_string(), "wtype");
        assert_eq!(PasteMethod::WlCopy.to_string(), "wl-copy");
    }

    #[test]
    fn test_pick_paste_method_explicit() {
        assert_eq!(pick_paste_method("xdotool").unwrap(), PasteMethod::Xdotool);
        assert_eq!(pick_paste_method("wtype").unwrap(), PasteMethod::Wtype);
        assert_eq!(pick_paste_method("wl-copy").unwrap(), PasteMethod::WlCopy);
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
