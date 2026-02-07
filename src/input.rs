use anyhow::{Context, Result, bail};
use evdev::Key;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct InputDevice {
    pub path: PathBuf,
    pub name: String,
}

/// List all /dev/input/event* devices with their names.
pub fn list_input_devices() -> Result<Vec<InputDevice>> {
    let mut devices = Vec::new();

    let entries = std::fs::read_dir("/dev/input").context("Failed to read /dev/input directory")?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let filename = path.file_name().unwrap_or_default().to_string_lossy();

        if !filename.starts_with("event") {
            continue;
        }

        match evdev::Device::open(&path) {
            Ok(device) => {
                let name = device.name().unwrap_or("Unknown").to_string();
                devices.push(InputDevice {
                    path: path.clone(),
                    name,
                });
            }
            Err(_) => {
                // Skip devices we can't open (permission issues)
                continue;
            }
        }
    }

    devices.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(devices)
}

/// Filter out mice, touchpads, and virtual devices from device list.
pub fn filter_keyboards(devices: &[InputDevice]) -> Vec<&InputDevice> {
    let exclude_patterns = ["mouse", "touchpad", "trackpoint", "trackball", "virtual"];
    devices
        .iter()
        .filter(|d| {
            let lower = d.name.to_lowercase();
            !exclude_patterns.iter().any(|pat| lower.contains(pat))
        })
        .collect()
}

/// Check if a device supports a specific key in its capabilities.
fn device_supports_key(path: &std::path::Path, key: Key) -> bool {
    let Ok(device) = evdev::Device::open(path) else {
        return false;
    };
    device
        .supported_keys()
        .is_some_and(|keys| keys.contains(key))
}

/// Pick the keyboard device to use based on settings.
/// When set to "auto", finds the first non-mouse/touchpad device
/// that supports the configured key in its capabilities.
pub fn pick_keyboard_device(device_setting: &str, key: Key) -> Result<PathBuf> {
    if device_setting != "auto" {
        let path = PathBuf::from(device_setting);
        if path.exists() {
            return Ok(path);
        }
        bail!("Configured keyboard device not found: {}", device_setting);
    }

    let devices = list_input_devices()?;
    let keyboards = filter_keyboards(&devices);

    // First pass: find a keyboard that supports the key
    for dev in &keyboards {
        if device_supports_key(&dev.path, key) {
            log::info!(
                "Auto-selected device {} ({}) - supports {:?}",
                dev.path.display(),
                dev.name,
                key
            );
            return Ok(dev.path.clone());
        }
    }

    // Fallback: first keyboard device
    if let Some(dev) = keyboards.first() {
        log::warn!(
            "No device explicitly supports {:?}, falling back to {} ({})",
            key,
            dev.path.display(),
            dev.name
        );
        return Ok(dev.path.clone());
    }

    bail!("No keyboard devices found. Check /dev/input permissions.");
}

/// Resolve a key name like "KEY_FN" to an evdev Key.
pub fn resolve_key(key_name: &str) -> Result<Key> {
    parse_key_name(key_name).with_context(|| format!("Unknown key name: {key_name}"))
}

/// Parse a key name string to an evdev Key.
fn parse_key_name(name: &str) -> Option<Key> {
    let name_upper = name.to_uppercase();
    let name_upper = name_upper.strip_prefix("KEY_").unwrap_or(&name_upper);

    match name_upper {
        "FN" => Some(Key::KEY_FN),
        "CAPSLOCK" => Some(Key::KEY_CAPSLOCK),
        "RIGHTCTRL" => Some(Key::KEY_RIGHTCTRL),
        "LEFTCTRL" => Some(Key::KEY_LEFTCTRL),
        "RIGHTALT" => Some(Key::KEY_RIGHTALT),
        "LEFTALT" => Some(Key::KEY_LEFTALT),
        "RIGHTMETA" => Some(Key::KEY_RIGHTMETA),
        "LEFTMETA" => Some(Key::KEY_LEFTMETA),
        "RIGHTSHIFT" => Some(Key::KEY_RIGHTSHIFT),
        "LEFTSHIFT" => Some(Key::KEY_LEFTSHIFT),
        "SCROLLLOCK" => Some(Key::KEY_SCROLLLOCK),
        "PAUSE" => Some(Key::KEY_PAUSE),
        "INSERT" => Some(Key::KEY_INSERT),
        "F1" => Some(Key::KEY_F1),
        "F2" => Some(Key::KEY_F2),
        "F3" => Some(Key::KEY_F3),
        "F4" => Some(Key::KEY_F4),
        "F5" => Some(Key::KEY_F5),
        "F6" => Some(Key::KEY_F6),
        "F7" => Some(Key::KEY_F7),
        "F8" => Some(Key::KEY_F8),
        "F9" => Some(Key::KEY_F9),
        "F10" => Some(Key::KEY_F10),
        "F11" => Some(Key::KEY_F11),
        "F12" => Some(Key::KEY_F12),
        "SPACE" => Some(Key::KEY_SPACE),
        _ => None,
    }
}

pub fn list_devices_cli() -> Result<()> {
    let devices = list_input_devices()?;
    let keyboards = filter_keyboards(&devices);

    println!("Input devices (keyboards):");
    for dev in &keyboards {
        println!("  {} - {}", dev.path.display(), dev.name);
    }

    if keyboards.is_empty() {
        println!("  (none found - check /dev/input permissions)");
        println!("  Try: sudo usermod -aG input $USER");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_key_fn() {
        let key = resolve_key("KEY_FN").unwrap();
        assert_eq!(key, Key::KEY_FN);
    }

    #[test]
    fn test_resolve_key_capslock() {
        let key = resolve_key("KEY_CAPSLOCK").unwrap();
        assert_eq!(key, Key::KEY_CAPSLOCK);
    }

    #[test]
    fn test_resolve_key_rightctrl() {
        let key = resolve_key("KEY_RIGHTCTRL").unwrap();
        assert_eq!(key, Key::KEY_RIGHTCTRL);
    }

    #[test]
    fn test_resolve_key_without_prefix() {
        let key = resolve_key("FN").unwrap();
        assert_eq!(key, Key::KEY_FN);
    }

    #[test]
    fn test_resolve_key_case_insensitive() {
        let key = resolve_key("key_rightctrl").unwrap();
        assert_eq!(key, Key::KEY_RIGHTCTRL);
    }

    #[test]
    fn test_resolve_key_unknown() {
        assert!(resolve_key("KEY_NONEXISTENT").is_err());
    }

    #[test]
    fn test_resolve_function_keys() {
        for i in 1..=12 {
            let name = format!("KEY_F{i}");
            assert!(resolve_key(&name).is_ok(), "Failed to resolve {name}");
        }
    }

    #[test]
    fn test_filter_keyboards() {
        let devices = vec![
            InputDevice {
                path: PathBuf::from("/dev/input/event0"),
                name: "AT Translated Set 2 keyboard".into(),
            },
            InputDevice {
                path: PathBuf::from("/dev/input/event1"),
                name: "SynPS/2 Synaptics TouchPad".into(),
            },
            InputDevice {
                path: PathBuf::from("/dev/input/event2"),
                name: "TPPS/2 Elan TrackPoint".into(),
            },
            InputDevice {
                path: PathBuf::from("/dev/input/event3"),
                name: "USB Mouse".into(),
            },
            InputDevice {
                path: PathBuf::from("/dev/input/event4"),
                name: "ThinkPad Extra Buttons".into(),
            },
        ];

        let keyboards = filter_keyboards(&devices);
        assert_eq!(keyboards.len(), 2);
        assert_eq!(keyboards[0].name, "AT Translated Set 2 keyboard");
        assert_eq!(keyboards[1].name, "ThinkPad Extra Buttons");
    }

    #[test]
    fn test_filter_keyboards_empty() {
        let devices: Vec<InputDevice> = vec![];
        let keyboards = filter_keyboards(&devices);
        assert!(keyboards.is_empty());
    }

    #[test]
    fn test_pick_keyboard_device_explicit_missing() {
        let result = pick_keyboard_device("/dev/input/event9999", Key::KEY_RIGHTCTRL);
        assert!(result.is_err());
    }
}
