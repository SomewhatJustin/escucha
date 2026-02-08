use crate::{audio, config, input, paste, preflight, transcribe};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
pub struct DiagnoseReport {
    schema_version: u32,
    app_version: String,
    command: String,
    unix_timestamp_ms: u128,
    ok: bool,
    environment: EnvironmentInfo,
    permissions: PermissionInfo,
    preflight: PreflightInfo,
    logs: LogInfo,
    smoke_test: Option<SmokeTestInfo>,
}

#[derive(Serialize)]
struct EnvironmentInfo {
    wayland_display: Option<String>,
    x11_display: Option<String>,
    xdg_session_type: Option<String>,
    xdg_current_desktop: Option<String>,
    command_available: BTreeMap<String, bool>,
    user_service_state: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct PermissionInfo {
    user: String,
    input_group_configured: bool,
    input_group_active_in_process: bool,
    readable_input_devices: usize,
    total_input_devices: usize,
    ydotool_socket_available: bool,
}

#[derive(Serialize)]
struct PreflightInfo {
    critical_failures: usize,
    warnings: usize,
    checks: Vec<PreflightCheckInfo>,
}

#[derive(Serialize)]
struct PreflightCheckInfo {
    name: String,
    passed: bool,
    severity: String,
    message: String,
    hint: Option<String>,
}

#[derive(Serialize)]
struct LogInfo {
    configured_log_file: Option<String>,
    log_file_exists: bool,
    tail_lines: Vec<String>,
}

#[derive(Serialize)]
struct SmokeTestInfo {
    duration_ms: u128,
    passed: bool,
    steps: Vec<SmokeStepInfo>,
}

#[derive(Serialize)]
struct SmokeStepInfo {
    name: String,
    required: bool,
    status: String,
    detail: String,
    duration_ms: u128,
}

pub fn run_and_print(command: &str, with_smoke_test: bool) -> Result<bool> {
    let report = run(command, with_smoke_test);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(report.ok)
}

pub fn run(command: &str, with_smoke_test: bool) -> DiagnoseReport {
    let settings = config::load_settings();
    let preflight_report = preflight::check_environment();

    let env = collect_environment();
    let perms = collect_permissions();
    let preflight = collect_preflight(&preflight_report);
    let logs = collect_logs(settings.as_ref().ok());

    let smoke_test = if with_smoke_test {
        Some(run_smoke_test(settings.as_ref().ok()))
    } else {
        None
    };

    let smoke_ok = smoke_test.as_ref().is_none_or(|s| s.passed);
    let ok = !preflight_report.has_critical_failures() && smoke_ok;

    DiagnoseReport {
        schema_version: 1,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        command: command.to_string(),
        unix_timestamp_ms: now_unix_ms(),
        ok,
        environment: env,
        permissions: perms,
        preflight,
        logs,
        smoke_test,
    }
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn collect_environment() -> EnvironmentInfo {
    let mut command_available = BTreeMap::new();
    for cmd in [
        "arecord",
        "ydotool",
        "ydotoold",
        "wl-copy",
        "wtype",
        "xdotool",
        "xclip",
        "pw-cat",
        "pactl",
        "xdg-desktop-portal",
    ] {
        command_available.insert(cmd.to_string(), command_exists(cmd));
    }

    let mut user_service_state = BTreeMap::new();
    for unit in ["escucha.service", "ydotoold.service"] {
        user_service_state.insert(unit.to_string(), user_unit_state(unit));
    }

    EnvironmentInfo {
        wayland_display: std::env::var("WAYLAND_DISPLAY").ok(),
        x11_display: std::env::var("DISPLAY").ok(),
        xdg_session_type: std::env::var("XDG_SESSION_TYPE").ok(),
        xdg_current_desktop: std::env::var("XDG_CURRENT_DESKTOP").ok(),
        command_available,
        user_service_state,
    }
}

fn collect_permissions() -> PermissionInfo {
    let user = std::env::var("USER").unwrap_or_default();
    let input_gid = input_group_gid();
    let active_group_ids = process_group_ids();
    let input_group_active_in_process = input_gid
        .map(|gid| active_group_ids.contains(&gid))
        .unwrap_or(false);

    let (readable_input_devices, total_input_devices) = input_device_readability();

    PermissionInfo {
        user: user.clone(),
        input_group_configured: user_listed_in_input_group(&user),
        input_group_active_in_process,
        readable_input_devices,
        total_input_devices,
        ydotool_socket_available: paste::ydotool_socket_available(),
    }
}

fn collect_preflight(report: &preflight::PreflightReport) -> PreflightInfo {
    let critical_failures = report
        .checks
        .iter()
        .filter(|c| !c.passed && c.severity == preflight::CheckSeverity::Critical)
        .count();

    let warnings = report
        .checks
        .iter()
        .filter(|c| !c.passed && c.severity == preflight::CheckSeverity::Warning)
        .count();

    let checks = report
        .checks
        .iter()
        .map(|c| PreflightCheckInfo {
            name: c.name.to_string(),
            passed: c.passed,
            severity: match c.severity {
                preflight::CheckSeverity::Critical => "critical".to_string(),
                preflight::CheckSeverity::Warning => "warning".to_string(),
            },
            message: c.message.clone(),
            hint: c.hint.clone(),
        })
        .collect();

    PreflightInfo {
        critical_failures,
        warnings,
        checks,
    }
}

fn collect_logs(settings: Option<&config::Settings>) -> LogInfo {
    let configured_log_file = settings.map(|s| s.log_file.clone());
    let (log_file_exists, tail_lines) = configured_log_file
        .as_ref()
        .map(|s| {
            let path = PathBuf::from(s);
            let exists = path.exists();
            let lines = if exists {
                read_tail_lines(&path, 80)
            } else {
                Vec::new()
            };
            (exists, lines)
        })
        .unwrap_or((false, Vec::new()));

    LogInfo {
        configured_log_file,
        log_file_exists,
        tail_lines,
    }
}

fn run_smoke_test(settings: Option<&config::Settings>) -> SmokeTestInfo {
    let overall_start = Instant::now();
    let mut steps = Vec::new();

    let settings = match settings {
        Some(s) => {
            steps.push(step_pass(
                "load_settings",
                true,
                "Config loaded successfully",
                Duration::from_millis(0),
            ));
            s.clone()
        }
        None => {
            steps.push(step_fail(
                "load_settings",
                true,
                "Failed to load config",
                Duration::from_millis(0),
            ));
            return SmokeTestInfo {
                duration_ms: overall_start.elapsed().as_millis(),
                passed: false,
                steps,
            };
        }
    };

    let key = {
        let start = Instant::now();
        match input::resolve_key(&settings.key) {
            Ok(key) => {
                steps.push(step_pass(
                    "resolve_trigger_key",
                    true,
                    format!("Resolved {} to {:?}", settings.key, key),
                    start.elapsed(),
                ));
                key
            }
            Err(e) => {
                steps.push(step_fail(
                    "resolve_trigger_key",
                    true,
                    format!("Failed to resolve key: {e}"),
                    start.elapsed(),
                ));
                return SmokeTestInfo {
                    duration_ms: overall_start.elapsed().as_millis(),
                    passed: false,
                    steps,
                };
            }
        }
    };

    {
        let start = Instant::now();
        match input::pick_keyboard_device(&settings.keyboard_device, key) {
            Ok(path) => steps.push(step_pass(
                "select_input_device",
                true,
                format!("Using {}", path.display()),
                start.elapsed(),
            )),
            Err(e) => steps.push(step_fail(
                "select_input_device",
                true,
                format!("Input device selection failed: {e}"),
                start.elapsed(),
            )),
        }
    }

    {
        let start = Instant::now();
        match paste::pick_paste_method(&settings.paste_method) {
            Ok(method) => steps.push(step_pass(
                "select_paste_method",
                true,
                format!("Using {}", method.as_str()),
                start.elapsed(),
            )),
            Err(e) => steps.push(step_fail(
                "select_paste_method",
                true,
                format!("Paste method selection failed: {e}"),
                start.elapsed(),
            )),
        }
    }

    let mut wav_path: Option<PathBuf> = None;
    {
        let start = Instant::now();
        if !audio::check_arecord() {
            steps.push(step_fail(
                "audio_capture_roundtrip",
                true,
                "arecord not available",
                start.elapsed(),
            ));
        } else {
            match audio::temp_wav_path() {
                Ok(path) => match audio::Recording::start(&path) {
                    Ok(rec) => {
                        std::thread::sleep(Duration::from_millis(350));
                        match rec.stop() {
                            Ok(recorded) => {
                                let size = std::fs::metadata(&recorded)
                                    .map(|m| m.len())
                                    .unwrap_or_default();
                                if size > 44 {
                                    steps.push(step_pass(
                                        "audio_capture_roundtrip",
                                        true,
                                        format!(
                                            "Captured WAV at {} ({} bytes)",
                                            recorded.display(),
                                            size
                                        ),
                                        start.elapsed(),
                                    ));
                                } else {
                                    steps.push(step_fail(
                                        "audio_capture_roundtrip",
                                        true,
                                        format!(
                                            "Recorded WAV too small at {} ({} bytes)",
                                            recorded.display(),
                                            size
                                        ),
                                        start.elapsed(),
                                    ));
                                }
                                wav_path = Some(recorded);
                            }
                            Err(e) => steps.push(step_fail(
                                "audio_capture_roundtrip",
                                true,
                                format!("Failed to stop recording: {e}"),
                                start.elapsed(),
                            )),
                        }
                    }
                    Err(e) => steps.push(step_fail(
                        "audio_capture_roundtrip",
                        true,
                        format!("Failed to start recording: {e}"),
                        start.elapsed(),
                    )),
                },
                Err(e) => steps.push(step_fail(
                    "audio_capture_roundtrip",
                    true,
                    format!("Could not create temp WAV path: {e}"),
                    start.elapsed(),
                )),
            }
        }
    }

    {
        let start = Instant::now();
        let model_path = transcribe::model_path(&settings.model);
        match (&wav_path, model_path.exists()) {
            (Some(wav), true) => {
                match transcribe::Transcriber::new(&model_path, &settings.language) {
                    Ok(transcriber) => match transcriber.transcribe(wav) {
                        Ok(text) => steps.push(step_pass(
                            "transcription_probe",
                            true,
                            format!("Transcription completed ({} chars)", text.len()),
                            start.elapsed(),
                        )),
                        Err(e) => steps.push(step_fail(
                            "transcription_probe",
                            true,
                            format!("Transcription failed: {e}"),
                            start.elapsed(),
                        )),
                    },
                    Err(e) => steps.push(step_fail(
                        "transcription_probe",
                        true,
                        format!("Model load failed: {e}"),
                        start.elapsed(),
                    )),
                }
            }
            (Some(_), false) => steps.push(step_skip(
                "transcription_probe",
                false,
                format!(
                    "Model {} not present at {} (download on first run)",
                    settings.model,
                    model_path.display()
                ),
                start.elapsed(),
            )),
            (None, _) => steps.push(step_skip(
                "transcription_probe",
                false,
                "Skipped because audio capture step failed",
                start.elapsed(),
            )),
        }
    }

    if let Some(path) = wav_path {
        audio::cleanup_recording(&path);
    }

    let passed = steps
        .iter()
        .filter(|s| s.required)
        .all(|s| s.status == "pass");

    SmokeTestInfo {
        duration_ms: overall_start.elapsed().as_millis(),
        passed,
        steps,
    }
}

fn read_tail_lines(path: &Path, line_count: usize) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    if lines.len() > line_count {
        lines = lines.split_off(lines.len() - line_count);
    }
    lines
}

fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

fn user_unit_state(unit: &str) -> String {
    match Command::new("systemctl")
        .args(["--user", "is-active", unit])
        .output()
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !stdout.is_empty() {
                stdout
            } else if !stderr.is_empty() {
                format!("unknown ({stderr})")
            } else {
                "inactive".to_string()
            }
        }
        Err(_) => "unavailable".to_string(),
    }
}

fn input_group_gid() -> Option<u32> {
    let groups = std::fs::read_to_string("/etc/group").ok()?;
    groups.lines().find_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?;
        let _password = parts.next()?;
        let gid = parts.next()?;
        if name == "input" {
            gid.parse::<u32>().ok()
        } else {
            None
        }
    })
}

fn process_group_ids() -> Vec<u32> {
    let status = match std::fs::read_to_string("/proc/self/status") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    status
        .lines()
        .find_map(|line| line.strip_prefix("Groups:"))
        .map(|rest| {
            rest.split_whitespace()
                .filter_map(|gid| gid.parse::<u32>().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn user_listed_in_input_group(user: &str) -> bool {
    if user.is_empty() {
        return false;
    }

    let Ok(groups) = std::fs::read_to_string("/etc/group") else {
        return false;
    };

    groups.lines().any(|line| {
        if !line.starts_with("input:") {
            return false;
        }
        let members = line.split(':').nth(3).unwrap_or_default();
        members.split(',').any(|m| m.trim() == user)
    })
}

fn input_device_readability() -> (usize, usize) {
    let mut readable = 0usize;
    let mut total = 0usize;

    let entries = match std::fs::read_dir("/dev/input") {
        Ok(e) => e,
        Err(_) => return (0, 0),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !name.starts_with("event") {
            continue;
        }

        total += 1;
        if evdev::Device::open(&path).is_ok() {
            readable += 1;
        }
    }

    (readable, total)
}

fn step_pass(
    name: &str,
    required: bool,
    detail: impl Into<String>,
    elapsed: Duration,
) -> SmokeStepInfo {
    SmokeStepInfo {
        name: name.to_string(),
        required,
        status: "pass".to_string(),
        detail: detail.into(),
        duration_ms: elapsed.as_millis(),
    }
}

fn step_fail(
    name: &str,
    required: bool,
    detail: impl Into<String>,
    elapsed: Duration,
) -> SmokeStepInfo {
    SmokeStepInfo {
        name: name.to_string(),
        required,
        status: "fail".to_string(),
        detail: detail.into(),
        duration_ms: elapsed.as_millis(),
    }
}

fn step_skip(
    name: &str,
    required: bool,
    detail: impl Into<String>,
    elapsed: Duration,
) -> SmokeStepInfo {
    SmokeStepInfo {
        name: name.to_string(),
        required,
        status: "skip".to_string(),
        detail: detail.into(),
        duration_ms: elapsed.as_millis(),
    }
}
