#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status_text)]
        #[qproperty(QString, status_detail)]
        #[qproperty(QString, device_name)]
        #[qproperty(QString, transcription)]
        #[qproperty(QString, status_icon_name)]
        #[qproperty(bool, show_spinner)]
        #[qproperty(bool, show_fix_button)]
        #[qproperty(bool, show_paste_fix_button)]
        #[qproperty(bool, is_recording)]
        #[qproperty(bool, is_stopped)]
        #[qproperty(bool, is_ready)]
        type EscuchaBackend = super::EscuchaBackendRust;

        #[qinvokable]
        fn fix_permissions(self: Pin<&mut EscuchaBackend>);

        #[qinvokable]
        fn fix_paste_setup(self: Pin<&mut EscuchaBackend>);

        #[qinvokable]
        fn request_shutdown(self: Pin<&mut EscuchaBackend>);

        #[qsignal]
        fn error_occurred(self: Pin<&mut EscuchaBackend>, message: QString);
    }

    impl cxx_qt::Threading for EscuchaBackend {}
    impl cxx_qt::Initialize for EscuchaBackend {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config;
use crate::service::{ServiceCallbacks, ServiceStatus};

/// Strip "/dev/input/eventN - " prefix, show only the human-readable name.
pub fn strip_device_prefix(label: &str) -> &str {
    if let Some(pos) = label.find(" - ") {
        &label[pos + 3..]
    } else {
        label
    }
}

const SG_REEXEC_ENV: &str = "ESCUCHA_SG_REEXECED";

fn shell_quote(arg: &str) -> String {
    let escaped = arg.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

/// Restart the application by re-executing itself with the new group membership active.
fn restart_app() {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("escucha"));
    let args: Vec<String> = std::env::args().collect();

    let mut cmd_parts = vec![
        format!("{SG_REEXEC_ENV}=1"),
        shell_quote(&exe.to_string_lossy()),
    ];
    cmd_parts.extend(args[1..].iter().map(|s| shell_quote(s)));
    let full_cmd = cmd_parts.join(" ");

    let success = std::process::Command::new("sg")
        .args(["input", "-c", &full_cmd])
        .spawn()
        .is_ok();

    if !success && std::env::var(SG_REEXEC_ENV).unwrap_or_default() != "1" {
        let _ = std::process::Command::new(&exe).args(&args[1..]).spawn();
    }

    std::process::exit(0);
}

const FIRST_RUN_MARKER: &str = "first-run-onboarding-v1.done";

fn escucha_state_dir() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/state"))
        .join("escucha")
}

fn first_run_marker_path() -> PathBuf {
    escucha_state_dir().join(FIRST_RUN_MARKER)
}

fn is_first_launch() -> bool {
    !first_run_marker_path().exists()
}

fn mark_first_launch_complete() {
    let marker = first_run_marker_path();
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(marker, b"ok\n");
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

fn attempt_input_permission_fix(qt_thread: cxx_qt::CxxQtThread<qobject::EscuchaBackend>) -> bool {
    let user = std::env::var("USER").unwrap_or_default();
    if user.is_empty() {
        let _ = qt_thread.queue(move |mut qobject| {
            qobject
                .as_mut()
                .error_occurred(QString::from("Could not determine current username"));
        });
        return false;
    }
    if !user
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        let _ = qt_thread.queue(move |mut qobject| {
            qobject.as_mut().error_occurred(QString::from(
                "Unsafe username; cannot run privileged setup",
            ));
        });
        return false;
    }

    // If /etc/group already has the user but this session still lacks access,
    // first try a self re-exec under `sg input` before asking for logout.
    if user_listed_in_input_group(&user) {
        let has_sg = which::which("sg").is_ok();
        let already_reexeced = std::env::var(SG_REEXEC_ENV).unwrap_or_default() == "1";
        if has_sg && !already_reexeced {
            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().set_show_fix_button(false);
                qobject.as_mut().set_status_detail(QString::from(
                    "Input group already granted. Restarting under input group...",
                ));
            });
            std::thread::sleep(std::time::Duration::from_secs(1));
            restart_app();
            return true;
        }

        let _ = qt_thread.queue(move |mut qobject| {
            qobject.as_mut().set_show_fix_button(true);
            qobject.as_mut().set_status_detail(QString::from(
                "Input group already granted. Log out and back in if access is still denied.",
            ));
            qobject.as_mut().error_occurred(QString::from(
                "Input permission still pending session refresh.",
            ));
        });
        return false;
    }

    let script = format!(
        "set -e; \
         usermod -aG input {user}; \
         if command -v setfacl >/dev/null 2>&1; then setfacl -m u:{user}:rw /dev/input/event* || true; fi"
    );
    let ok = std::process::Command::new("pkexec")
        .args(["/bin/sh", "-c", &script])
        .status()
        .is_ok_and(|s| s.success());

    if ok {
        let has_sg = which::which("sg").is_ok();
        let _ = qt_thread.queue(move |mut qobject| {
            qobject.as_mut().set_show_fix_button(false);
            if has_sg {
                qobject.as_mut().set_status_detail(QString::from(
                    "Input permission granted; restarting with new group...",
                ));
            } else {
                qobject.as_mut().set_status_detail(QString::from(
                    "Input permission granted. Log out and back in to apply.",
                ));
            }
        });

        if has_sg {
            std::thread::sleep(std::time::Duration::from_secs(2));
            restart_app();
            return true;
        }
    } else {
        let _ = qt_thread.queue(move |mut qobject| {
            qobject.as_mut().set_show_fix_button(true);
            qobject
                .as_mut()
                .error_occurred(QString::from("Input permission request was denied"));
        });
    }

    false
}

fn first_launch_onboarding(qt_thread: &cxx_qt::CxxQtThread<qobject::EscuchaBackend>) {
    if !is_first_launch() {
        return;
    }

    let _ = qt_thread.queue(move |mut qobject| {
        qobject
            .as_mut()
            .set_status_detail(QString::from("Running first-launch setup checks..."));
    });

    // Best effort: make sure paste service is enabled/running up front.
    let paste_ready = crate::paste::ensure_ydotoold_running();
    if !paste_ready && which::which("ydotool").is_ok() {
        let _ = qt_thread.queue(move |mut qobject| {
            qobject.as_mut().set_show_paste_fix_button(true);
            qobject.as_mut().set_status_detail(QString::from(
                "Paste service not running. Click 'Fix Paste Setup' or run: systemctl --user enable --now ydotoold.service",
            ));
            qobject.as_mut().error_occurred(QString::from(
                "Automatic paste is not fully configured yet. Using clipboard fallback.",
            ));
        });
    }

    let report = crate::preflight::check_environment();
    let input_failed = report
        .checks
        .iter()
        .any(|c| c.name == "input devices" && !c.passed);

    if input_failed {
        let _ = qt_thread.queue(move |mut qobject| {
            qobject.as_mut().set_show_fix_button(true);
            qobject.as_mut().set_status_detail(QString::from(
                "Escucha needs input permissions. Requesting them now...",
            ));
        });
        let _ = attempt_input_permission_fix(qt_thread.clone());
    }

    mark_first_launch_complete();
}

#[derive(Default)]
pub struct EscuchaBackendRust {
    status_text: QString,
    status_detail: QString,
    device_name: QString,
    transcription: QString,
    status_icon_name: QString,
    show_spinner: bool,
    show_fix_button: bool,
    show_paste_fix_button: bool,
    is_recording: bool,
    is_stopped: bool,
    is_ready: bool,
    shutdown_flag: Option<Arc<AtomicBool>>,
}

impl qobject::EscuchaBackend {
    pub fn fix_permissions(self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let _ = attempt_input_permission_fix(qt_thread);
        });
    }

    pub fn request_shutdown(self: Pin<&mut Self>) {
        if let Some(flag) = &self.rust().shutdown_flag {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn fix_paste_setup(self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let ok = crate::paste::repair_paste_setup().is_ok();
            let _ = qt_thread.queue(move |mut qobject| {
                if ok {
                    qobject.as_mut().set_show_paste_fix_button(false);
                    qobject.as_mut().set_status_detail(QString::from(
                        "Paste setup fixed. Restarting...",
                    ));
                    let _ = std::process::Command::new(
                        std::env::current_exe()
                            .unwrap_or_else(|_| std::path::PathBuf::from("escucha")),
                    )
                    .args(std::env::args().skip(1))
                    .spawn();
                    std::process::exit(0);
                } else {
                    qobject.as_mut().error_occurred(QString::from(
                        "Could not fix paste setup automatically. Please verify /dev/uinput access and run: systemctl --user enable --now ydotoold.service",
                    ));
                }
            });
        });
    }
}

impl cxx_qt::Initialize for qobject::EscuchaBackend {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().set_status_text(QString::from("Starting..."));
        self.as_mut()
            .set_status_icon_name(QString::from("audio-input-microphone-symbolic"));
        self.as_mut().set_show_spinner(true);
        self.as_mut()
            .set_transcription(QString::from("Hold Right Ctrl and speak..."));

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            run_service_thread(qt_thread);
        });
    }
}

fn run_service_thread(qt_thread: cxx_qt::CxxQtThread<qobject::EscuchaBackend>) {
    first_launch_onboarding(&qt_thread);

    // Run preflight checks
    let report = crate::preflight::check_environment();
    if report.has_critical_failures() {
        let error_msg = report.critical_failure_summary();
        let input_failed = report
            .checks
            .iter()
            .any(|c| c.name == "input devices" && !c.passed);
        let paste_failed = report
            .checks
            .iter()
            .any(|c| c.name == "paste tool" && !c.passed);
        let detail_msg = report
            .checks
            .iter()
            .find(|c| !c.passed)
            .map(|c| match &c.hint {
                Some(h) => format!("{}: {}", c.message, h),
                None => c.message.clone(),
            })
            .unwrap_or_default();

        let _ = qt_thread.queue(move |mut qobject| {
            qobject.as_mut().set_status_text(QString::from("Stopped"));
            qobject.as_mut().set_show_spinner(false);
            qobject.as_mut().set_is_stopped(true);
            qobject
                .as_mut()
                .set_status_icon_name(QString::from("microphone-disabled-symbolic"));
            if input_failed {
                qobject.as_mut().set_show_fix_button(true);
            }
            if paste_failed {
                qobject.as_mut().set_show_paste_fix_button(true);
            }
            if !detail_msg.is_empty() {
                qobject
                    .as_mut()
                    .set_status_detail(QString::from(detail_msg.as_str()));
            }
            qobject
                .as_mut()
                .error_occurred(QString::from(error_msg.as_str()));
        });
        return;
    }

    if report.has_warnings() {
        for check in &report.checks {
            if !check.passed {
                let msg = match &check.hint {
                    Some(hint) => format!("{}: {} ({})", check.name, check.message, hint),
                    None => format!("{}: {}", check.name, check.message),
                };
                let _ = qt_thread.queue(move |mut qobject| {
                    qobject.as_mut().error_occurred(QString::from(msg.as_str()));
                });
            }
        }
    }

    let settings = match config::load_settings() {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Config error: {e}");
            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().set_status_text(QString::from("Stopped"));
                qobject.as_mut().set_show_spinner(false);
                qobject.as_mut().set_is_stopped(true);
                qobject
                    .as_mut()
                    .set_status_icon_name(QString::from("microphone-disabled-symbolic"));
                qobject.as_mut().error_occurred(QString::from(msg.as_str()));
            });
            return;
        }
    };

    match crate::service::DictationService::new(settings) {
        Ok(service) => {
            let device_label = service.device_label();
            let display_name = strip_device_prefix(&device_label).to_string();
            let _ = qt_thread.queue(move |mut qobject| {
                qobject
                    .as_mut()
                    .set_device_name(QString::from(display_name.as_str()));
            });

            // Set up shutdown bridge: store the service's shutdown handle into the QObject
            let svc_shutdown = service.shutdown_handle();
            let gui_shutdown = svc_shutdown.clone();
            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().rust_mut().shutdown_flag = Some(gui_shutdown);
            });

            let mut callbacks = BridgeCallbacks {
                qt_thread: qt_thread.clone(),
            };
            if let Err(e) = service.run_loop(&mut callbacks) {
                log::error!("Service error: {e}");
            }
        }
        Err(e) => {
            let msg = format!("{e}");
            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().set_status_text(QString::from("Stopped"));
                qobject.as_mut().set_show_spinner(false);
                qobject.as_mut().set_is_stopped(true);
                qobject
                    .as_mut()
                    .set_status_icon_name(QString::from("microphone-disabled-symbolic"));
                qobject.as_mut().error_occurred(QString::from(msg.as_str()));
            });
        }
    }
}

struct BridgeCallbacks {
    qt_thread: cxx_qt::CxxQtThread<qobject::EscuchaBackend>,
}

impl ServiceCallbacks for BridgeCallbacks {
    fn on_status(&mut self, status: ServiceStatus) {
        let _ = self.qt_thread.queue(move |mut qobject| {
            // Reset state booleans
            qobject.as_mut().set_is_recording(false);
            qobject.as_mut().set_is_stopped(false);
            qobject.as_mut().set_is_ready(false);

            match status {
                ServiceStatus::Stopped => {
                    qobject.as_mut().set_status_text(QString::from("Stopped"));
                    qobject.as_mut().set_show_spinner(false);
                    qobject.as_mut().set_is_stopped(true);
                    qobject
                        .as_mut()
                        .set_status_icon_name(QString::from("microphone-disabled-symbolic"));
                    qobject.as_mut().set_status_detail(QString::from(""));
                }
                ServiceStatus::Starting => {
                    qobject
                        .as_mut()
                        .set_status_text(QString::from("Starting..."));
                    qobject.as_mut().set_show_spinner(true);
                    qobject
                        .as_mut()
                        .set_status_icon_name(QString::from("audio-input-microphone-symbolic"));
                }
                ServiceStatus::Ready => {
                    qobject.as_mut().set_status_text(QString::from("Ready"));
                    qobject.as_mut().set_show_spinner(false);
                    qobject.as_mut().set_is_ready(true);
                    qobject
                        .as_mut()
                        .set_status_icon_name(QString::from("audio-input-microphone-symbolic"));
                    qobject
                        .as_mut()
                        .set_status_detail(QString::from("Hold Right Ctrl to speak"));
                    qobject.as_mut().set_show_fix_button(false);
                    qobject.as_mut().set_show_paste_fix_button(false);
                }
                ServiceStatus::Recording => {
                    qobject
                        .as_mut()
                        .set_status_text(QString::from("Recording..."));
                    qobject.as_mut().set_show_spinner(false);
                    qobject.as_mut().set_is_recording(true);
                    qobject.as_mut().set_status_icon_name(QString::from(
                        "microphone-sensitivity-high-symbolic",
                    ));
                    qobject
                        .as_mut()
                        .set_status_detail(QString::from("Release to transcribe"));
                }
                ServiceStatus::Transcribing => {
                    qobject
                        .as_mut()
                        .set_status_text(QString::from("Transcribing..."));
                    qobject.as_mut().set_show_spinner(true);
                    qobject
                        .as_mut()
                        .set_status_icon_name(QString::from("audio-input-microphone-symbolic"));
                    qobject.as_mut().set_status_detail(QString::from(""));
                }
                ServiceStatus::Stopping => {
                    qobject
                        .as_mut()
                        .set_status_text(QString::from("Stopping..."));
                    qobject.as_mut().set_show_spinner(true);
                    qobject.as_mut().set_is_stopped(true);
                    qobject
                        .as_mut()
                        .set_status_icon_name(QString::from("microphone-disabled-symbolic"));
                    qobject.as_mut().set_status_detail(QString::from(""));
                }
            }
        });
    }

    fn on_status_msg(&mut self, msg: &str) {
        let msg = msg.to_string();
        let _ = self.qt_thread.queue(move |mut qobject| {
            qobject
                .as_mut()
                .set_status_detail(QString::from(msg.as_str()));
        });
    }

    fn on_text(&mut self, text: &str) {
        let text = text.to_string();
        let _ = self.qt_thread.queue(move |mut qobject| {
            if text.is_empty() {
                qobject
                    .as_mut()
                    .set_transcription(QString::from("Hold Right Ctrl and speak..."));
            } else {
                qobject
                    .as_mut()
                    .set_transcription(QString::from(text.as_str()));
            }
        });
    }

    fn on_error(&mut self, error: &str) {
        let error = error.to_string();
        let _ = self.qt_thread.queue(move |mut qobject| {
            qobject
                .as_mut()
                .error_occurred(QString::from(error.as_str()));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_device_prefix() {
        assert_eq!(
            strip_device_prefix("/dev/input/event5 - AT Translated Set 2 keyboard"),
            "AT Translated Set 2 keyboard"
        );
        assert_eq!(
            strip_device_prefix("Some Device Without Prefix"),
            "Some Device Without Prefix"
        );
        assert_eq!(strip_device_prefix(""), "");
    }
}
