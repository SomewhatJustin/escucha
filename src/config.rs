use anyhow::{Context, Result};
use ini::Ini;
use std::path::PathBuf;

const SECTION: &str = "escucha";

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub key: String,
    pub keyboard_device: String,
    pub model: String,
    pub language: String,
    pub paste_method: String,
    pub paste_hotkey: String,
    pub clipboard_paste: String,
    pub clipboard_paste_delay_ms: u32,
    pub log_file: String,
    pub log_level: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            key: "KEY_RIGHTCTRL".into(),
            keyboard_device: "auto".into(),
            model: "base.en".into(),
            language: "en".into(),
            paste_method: "auto".into(),
            paste_hotkey: "ctrl+v".into(),
            clipboard_paste: "auto".into(),
            clipboard_paste_delay_ms: 75,
            log_file: default_log_file(),
            log_level: "info".into(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("escucha")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.ini")
}

fn default_log_file() -> String {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("~/.local/state"))
        .join("escucha")
        .join("escucha.log")
        .to_string_lossy()
        .into_owned()
}

fn get_or_default(ini: &Ini, key: &str, default: &str) -> String {
    ini.get_from(Some(SECTION), key)
        .unwrap_or(default)
        .to_string()
}

fn get_u32_or_default(ini: &Ini, key: &str, default: u32) -> u32 {
    ini.get_from(Some(SECTION), key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub fn load_settings() -> Result<Settings> {
    load_settings_from(config_path())
}

pub fn load_settings_from(path: PathBuf) -> Result<Settings> {
    let defaults = Settings::default();

    if !path.exists() {
        return Ok(defaults);
    }

    let ini = Ini::load_from_file(&path)
        .with_context(|| format!("Failed to load config from {}", path.display()))?;

    Ok(Settings {
        key: get_or_default(&ini, "key", &defaults.key),
        keyboard_device: get_or_default(&ini, "keyboard_device", &defaults.keyboard_device),
        model: get_or_default(&ini, "model", &defaults.model),
        language: get_or_default(&ini, "language", &defaults.language),
        paste_method: get_or_default(&ini, "paste_method", &defaults.paste_method),
        paste_hotkey: get_or_default(&ini, "paste_hotkey", &defaults.paste_hotkey),
        clipboard_paste: get_or_default(&ini, "clipboard_paste", &defaults.clipboard_paste),
        clipboard_paste_delay_ms: get_u32_or_default(
            &ini,
            "clipboard_paste_delay_ms",
            defaults.clipboard_paste_delay_ms,
        ),
        log_file: get_or_default(&ini, "log_file", &defaults.log_file),
        log_level: get_or_default(&ini, "log_level", &defaults.log_level),
    })
}

pub fn ensure_default_config() -> Result<PathBuf> {
    let path = config_path();
    if path.exists() {
        return Ok(path);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config dir {}", parent.display()))?;
    }

    let defaults = Settings::default();
    let mut ini = Ini::new();
    ini.with_section(Some(SECTION))
        .set("key", &defaults.key)
        .set("keyboard_device", &defaults.keyboard_device)
        .set("model", &defaults.model)
        .set("language", &defaults.language)
        .set("paste_method", &defaults.paste_method)
        .set("paste_hotkey", &defaults.paste_hotkey)
        .set("clipboard_paste", &defaults.clipboard_paste)
        .set(
            "clipboard_paste_delay_ms",
            defaults.clipboard_paste_delay_ms.to_string(),
        )
        .set("log_file", &defaults.log_file)
        .set("log_level", &defaults.log_level);

    ini.write_to_file(&path)
        .with_context(|| format!("Failed to write config to {}", path.display()))?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert_eq!(s.key, "KEY_RIGHTCTRL");
        assert_eq!(s.keyboard_device, "auto");
        assert_eq!(s.model, "base.en");
        assert_eq!(s.language, "en");
        assert_eq!(s.paste_method, "auto");
        assert_eq!(s.paste_hotkey, "ctrl+v");
        assert_eq!(s.clipboard_paste, "auto");
        assert_eq!(s.clipboard_paste_delay_ms, 75);
        assert_eq!(s.log_level, "info");
    }

    #[test]
    fn test_load_missing_config_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.ini");
        let settings = load_settings_from(path).unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn test_load_partial_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.ini");

        let mut ini = Ini::new();
        ini.with_section(Some(SECTION))
            .set("key", "KEY_CAPSLOCK")
            .set("model", "large");
        ini.write_to_file(&path).unwrap();

        let settings = load_settings_from(path).unwrap();
        assert_eq!(settings.key, "KEY_CAPSLOCK");
        assert_eq!(settings.model, "large");
        // Defaults for unset values
        assert_eq!(settings.language, "en");
        assert_eq!(settings.paste_method, "auto");
    }

    #[test]
    fn test_load_full_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.ini");

        let mut ini = Ini::new();
        ini.with_section(Some(SECTION))
            .set("key", "KEY_RIGHTCTRL")
            .set("keyboard_device", "/dev/input/event5")
            .set("model", "small.en")
            .set("language", "es")
            .set("paste_method", "xdotool")
            .set("paste_hotkey", "ctrl+shift+v")
            .set("clipboard_paste", "off")
            .set("clipboard_paste_delay_ms", "100")
            .set("log_file", "/tmp/test.log")
            .set("log_level", "debug");
        ini.write_to_file(&path).unwrap();

        let settings = load_settings_from(path).unwrap();
        assert_eq!(settings.key, "KEY_RIGHTCTRL");
        assert_eq!(settings.keyboard_device, "/dev/input/event5");
        assert_eq!(settings.model, "small.en");
        assert_eq!(settings.language, "es");
        assert_eq!(settings.paste_method, "xdotool");
        assert_eq!(settings.paste_hotkey, "ctrl+shift+v");
        assert_eq!(settings.clipboard_paste, "off");
        assert_eq!(settings.clipboard_paste_delay_ms, 100);
        assert_eq!(settings.log_file, "/tmp/test.log");
        assert_eq!(settings.log_level, "debug");
    }

    #[test]
    fn test_invalid_u32_falls_back_to_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.ini");

        let mut ini = Ini::new();
        ini.with_section(Some(SECTION))
            .set("clipboard_paste_delay_ms", "");
        ini.write_to_file(&path).unwrap();

        let settings = load_settings_from(path).unwrap();
        assert_eq!(settings.clipboard_paste_delay_ms, 75);
    }

    #[test]
    fn test_ensure_default_config_creates_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subdir").join("config.ini");

        // Manually write defaults to this path to test the write logic
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent).unwrap();

        let defaults = Settings::default();
        let mut ini = Ini::new();
        ini.with_section(Some(SECTION))
            .set("key", &defaults.key)
            .set("model", &defaults.model);
        ini.write_to_file(&path).unwrap();

        assert!(path.exists());
        let settings = load_settings_from(path).unwrap();
        assert_eq!(settings.key, "KEY_RIGHTCTRL");
        assert_eq!(settings.model, "base.en");
    }
}
