use std::fmt;
use std::path::PathBuf;

/// Severity of a preflight check result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CheckSeverity {
    /// Must pass or the app cannot function.
    Critical,
    /// App can work but with reduced functionality.
    Warning,
}

/// Result of a single preflight check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: &'static str,
    pub passed: bool,
    pub severity: CheckSeverity,
    pub message: String,
    pub hint: Option<String>,
}

/// Collection of all preflight check results.
pub struct PreflightReport {
    pub checks: Vec<CheckResult>,
}

impl PreflightReport {
    pub fn has_critical_failures(&self) -> bool {
        self.checks
            .iter()
            .any(|c| !c.passed && c.severity == CheckSeverity::Critical)
    }

    pub fn has_warnings(&self) -> bool {
        self.checks
            .iter()
            .any(|c| !c.passed && c.severity == CheckSeverity::Warning)
    }

    /// Short summary of critical failures for GUI toasts.
    pub fn critical_failure_summary(&self) -> String {
        let failures: Vec<&str> = self
            .checks
            .iter()
            .filter(|c| !c.passed && c.severity == CheckSeverity::Critical)
            .map(|c| c.name)
            .collect();

        match failures.len() {
            0 => String::new(),
            1 => format!("Setup required: {}", failures[0]),
            _ => format!("Setup required: {}", failures.join(", ")),
        }
    }
}

impl fmt::Display for PreflightReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "escucha environment check:")?;
        for check in &self.checks {
            let tag = if check.passed {
                " PASS"
            } else {
                match check.severity {
                    CheckSeverity::Critical => " FAIL",
                    CheckSeverity::Warning => " WARN",
                }
            };
            writeln!(f, "  [{tag}] {:<14} {}", check.name, check.message)?;
            if let Some(hint) = &check.hint {
                writeln!(f, "  {:>6} {:<14} hint: {hint}", "", "")?;
            }
        }
        Ok(())
    }
}

/// Run all environment checks and return a report.
pub fn check_environment() -> PreflightReport {
    let checks = vec![
        check_input_access(),
        check_arecord(),
        check_paste_tool(),
        check_curl(),
        check_directory(
            "config dir",
            crate::config::config_dir(),
            CheckSeverity::Critical,
        ),
        check_directory(
            "data dir",
            crate::transcribe::default_model_dir(),
            CheckSeverity::Critical,
        ),
        check_directory(
            "state dir",
            dirs::state_dir()
                .unwrap_or_else(|| PathBuf::from("~/.local/state"))
                .join("escucha"),
            CheckSeverity::Warning,
        ),
    ];

    PreflightReport { checks }
}

/// Check if we can access /dev/input devices (need input group).
fn check_input_access() -> CheckResult {
    let name = "input devices";

    let entries = match std::fs::read_dir("/dev/input") {
        Ok(e) => e,
        Err(_) => {
            return CheckResult {
                name,
                passed: false,
                severity: CheckSeverity::Critical,
                message: "Cannot read /dev/input".into(),
                hint: Some(
                    "sudo usermod -aG input $USER  (then log out and back in)".into(),
                ),
            };
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if !filename.starts_with("event") {
            continue;
        }
        if evdev::Device::open(&path).is_ok() {
            return CheckResult {
                name,
                passed: true,
                severity: CheckSeverity::Critical,
                message: format!("Can access {}", path.display()),
                hint: None,
            };
        }
    }

    CheckResult {
        name,
        passed: false,
        severity: CheckSeverity::Critical,
        message: "No input devices accessible (permission denied)".into(),
        hint: Some("sudo usermod -aG input $USER  (then log out and back in)".into()),
    }
}

/// Check if arecord is installed.
fn check_arecord() -> CheckResult {
    let name = "arecord";
    match which::which("arecord") {
        Ok(path) => CheckResult {
            name,
            passed: true,
            severity: CheckSeverity::Critical,
            message: format!("Found at {}", path.display()),
            hint: None,
        },
        Err(_) => CheckResult {
            name,
            passed: false,
            severity: CheckSeverity::Critical,
            message: "arecord not found".into(),
            hint: Some("Install alsa-utils".into()),
        },
    }
}

/// Check if an appropriate paste tool is available.
fn check_paste_tool() -> CheckResult {
    let name = "paste tool";
    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let is_x11 = std::env::var("DISPLAY").is_ok();

    if is_wayland {
        if which::which("wtype").is_ok() {
            return CheckResult {
                name,
                passed: true,
                severity: CheckSeverity::Critical,
                message: "wtype available (Wayland)".into(),
                hint: None,
            };
        }
        if which::which("wl-copy").is_ok() {
            return CheckResult {
                name,
                passed: true,
                severity: CheckSeverity::Warning,
                message: "wl-copy available (clipboard only, no auto-paste)".into(),
                hint: Some("Install wtype for automatic pasting".into()),
            };
        }
    }

    if is_x11 && which::which("xdotool").is_ok() {
        return CheckResult {
            name,
            passed: true,
            severity: CheckSeverity::Critical,
            message: "xdotool available (X11)".into(),
            hint: None,
        };
    }

    if !is_wayland && !is_x11 {
        // No display server detected — likely running under systemd before session init
        return CheckResult {
            name,
            passed: true,
            severity: CheckSeverity::Warning,
            message: "No display server detected (OK if running as a service)".into(),
            hint: None,
        };
    }

    CheckResult {
        name,
        passed: false,
        severity: CheckSeverity::Critical,
        message: "No paste tool found".into(),
        hint: Some(if is_wayland {
            "Install wtype and wl-clipboard".into()
        } else {
            "Install xdotool".into()
        }),
    }
}

/// Check if curl is available (needed for model downloads).
fn check_curl() -> CheckResult {
    let name = "curl";

    // If the default model is already cached, curl isn't needed
    let settings = crate::config::Settings::default();
    let model_path = crate::transcribe::model_path(&settings.model);
    if model_path.exists() {
        return CheckResult {
            name,
            passed: true,
            severity: CheckSeverity::Warning,
            message: "Not needed (model already downloaded)".into(),
            hint: None,
        };
    }

    match which::which("curl") {
        Ok(path) => CheckResult {
            name,
            passed: true,
            severity: CheckSeverity::Warning,
            message: format!("Found at {}", path.display()),
            hint: None,
        },
        Err(_) => CheckResult {
            name,
            passed: false,
            severity: CheckSeverity::Warning,
            message: "curl not found (needed to download Whisper model)".into(),
            hint: Some("Install curl".into()),
        },
    }
}

/// Check if a directory can be created/accessed.
fn check_directory(
    name: &'static str,
    path: PathBuf,
    severity: CheckSeverity,
) -> CheckResult {
    match std::fs::create_dir_all(&path) {
        Ok(()) if path.is_dir() => CheckResult {
            name,
            passed: true,
            severity,
            message: format!("{}", path.display()),
            hint: None,
        },
        Ok(()) => CheckResult {
            name,
            passed: false,
            severity,
            message: format!("{} is not a directory", path.display()),
            hint: Some("Check file system permissions".into()),
        },
        Err(e) => CheckResult {
            name,
            passed: false,
            severity,
            message: format!("Cannot create {}: {e}", path.display()),
            hint: Some("Check file system permissions".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(name: &'static str) -> CheckResult {
        CheckResult {
            name,
            passed: true,
            severity: CheckSeverity::Critical,
            message: "ok".into(),
            hint: None,
        }
    }

    fn fail(name: &'static str, severity: CheckSeverity) -> CheckResult {
        CheckResult {
            name,
            passed: false,
            severity,
            message: "bad".into(),
            hint: Some("fix it".into()),
        }
    }

    #[test]
    fn test_no_failures() {
        let report = PreflightReport {
            checks: vec![pass("a"), pass("b")],
        };
        assert!(!report.has_critical_failures());
        assert!(!report.has_warnings());
        assert!(report.critical_failure_summary().is_empty());
    }

    #[test]
    fn test_critical_failure() {
        let report = PreflightReport {
            checks: vec![pass("a"), fail("input", CheckSeverity::Critical)],
        };
        assert!(report.has_critical_failures());
        assert!(!report.has_warnings());
        assert_eq!(report.critical_failure_summary(), "Setup required: input");
    }

    #[test]
    fn test_warning_only() {
        let report = PreflightReport {
            checks: vec![pass("a"), fail("curl", CheckSeverity::Warning)],
        };
        assert!(!report.has_critical_failures());
        assert!(report.has_warnings());
    }

    #[test]
    fn test_multiple_critical_failures() {
        let report = PreflightReport {
            checks: vec![
                fail("input", CheckSeverity::Critical),
                fail("arecord", CheckSeverity::Critical),
            ],
        };
        assert_eq!(
            report.critical_failure_summary(),
            "Setup required: input, arecord"
        );
    }

    #[test]
    fn test_format_report_pass() {
        let report = PreflightReport {
            checks: vec![pass("arecord")],
        };
        let output = report.to_string();
        assert!(output.contains("PASS"));
        assert!(output.contains("arecord"));
    }

    #[test]
    fn test_format_report_fail_with_hint() {
        let report = PreflightReport {
            checks: vec![fail("input", CheckSeverity::Critical)],
        };
        let output = report.to_string();
        assert!(output.contains("FAIL"));
        assert!(output.contains("hint:"));
    }

    #[test]
    fn test_check_directory_with_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("escucha_test");
        let result = check_directory("test dir", path.clone(), CheckSeverity::Critical);
        assert!(result.passed);
        assert!(path.exists());
    }

    #[test]
    fn test_check_arecord_does_not_panic() {
        let result = check_arecord();
        // Just verify it returns a well-formed result
        assert!(!result.name.is_empty());
    }

    #[test]
    fn test_check_curl_does_not_panic() {
        let result = check_curl();
        assert!(!result.name.is_empty());
    }

    #[test]
    fn test_check_paste_tool_does_not_panic() {
        let result = check_paste_tool();
        assert!(!result.name.is_empty());
    }
}
