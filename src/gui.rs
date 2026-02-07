use anyhow::Result;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config;
use crate::service::ServiceStatus;

#[derive(Debug, Clone)]
enum ServiceMessage {
    Status(ServiceStatus),
    StatusMsg(String),
    Text(String),
    Error(String),
    Device(String),
    InputFixAvailable,
}

const CSS: &str = r#"
@keyframes recording-pulse {
    0%, 100% { opacity: 1.0; }
    50% { opacity: 0.4; }
}

.recording-pulse {
    animation: recording-pulse 1.2s ease-in-out infinite;
}

.status-icon {
    -gtk-icon-size: 64px;
}
"#;

struct GuiCallbacks {
    tx: async_channel::Sender<ServiceMessage>,
}

impl crate::service::ServiceCallbacks for GuiCallbacks {
    fn on_status(&mut self, status: ServiceStatus) {
        let _ = self.tx.send_blocking(ServiceMessage::Status(status));
    }
    fn on_status_msg(&mut self, msg: &str) {
        let _ = self
            .tx
            .send_blocking(ServiceMessage::StatusMsg(msg.to_string()));
    }
    fn on_text(&mut self, text: &str) {
        let _ = self
            .tx
            .send_blocking(ServiceMessage::Text(text.to_string()));
    }
    fn on_error(&mut self, error: &str) {
        let _ = self
            .tx
            .send_blocking(ServiceMessage::Error(error.to_string()));
    }
}

fn strip_device_prefix(label: &str) -> &str {
    // Strip "/dev/input/eventN - " prefix, show only the human-readable name
    if let Some(pos) = label.find(" - ") {
        &label[pos + 3..]
    } else {
        label
    }
}

/// Restart the application by re-executing itself with the new group membership active.
fn restart_app() {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("escucha"));
    let args: Vec<String> = std::env::args().collect();

    // Try to use 'sg input' to activate the group without logout
    // Format: sg input -c "escucha --gui"
    let mut cmd_parts = vec![exe.to_string_lossy().to_string()];
    cmd_parts.extend(args[1..].iter().map(|s| s.to_string()));
    let full_cmd = cmd_parts.join(" ");

    let success = std::process::Command::new("sg")
        .args(["input", "-c", &full_cmd])
        .spawn()
        .is_ok();

    if !success {
        // Fallback: just re-exec normally (won't have group active, but better than nothing)
        let _ = std::process::Command::new(&exe)
            .args(&args[1..])
            .spawn();
    }

    // Exit current process
    std::process::exit(0);
}

fn build_ui(app: &adw::Application) {
    // Load CSS
    let css_provider = gtk4::CssProvider::new();
    css_provider.load_from_string(CSS);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not get default display"),
        &css_provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // --- Build widget tree ---

    // Status icon (for icon-based states)
    let status_icon = gtk4::Image::builder()
        .icon_name("audio-input-microphone-symbolic")
        .pixel_size(64)
        .css_classes(["status-icon"])
        .build();

    // Spinner (for loading states)
    let status_spinner = adw::Spinner::builder()
        .width_request(32)
        .height_request(32)
        .build();

    // Icon stack (crossfade between icon and spinner)
    let icon_stack = gtk4::Stack::builder()
        .transition_type(gtk4::StackTransitionType::Crossfade)
        .transition_duration(150)
        .build();
    icon_stack.add_named(&status_icon, Some("icon"));
    icon_stack.add_named(&status_spinner, Some("spinner"));
    icon_stack.set_visible_child_name("icon");

    // Status label
    let status_label = gtk4::Label::builder()
        .label("Starting...")
        .css_classes(["title-2"])
        .build();

    // Status detail
    let status_detail = gtk4::Label::builder()
        .label("")
        .css_classes(["dim-label"])
        .visible(false)
        .build();

    // Fix permissions button (hidden until needed)
    let fix_button = gtk4::Button::builder()
        .label("Fix Input Permissions")
        .css_classes(["suggested-action", "pill"])
        .visible(false)
        .build();

    // Status area box
    let status_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .spacing(12)
        .build();
    status_box.append(&icon_stack);
    status_box.append(&status_label);
    status_box.append(&status_detail);
    status_box.append(&fix_button);

    // Status area clamp
    let status_clamp = adw::Clamp::builder()
        .maximum_size(360)
        .vexpand(true)
        .build();
    status_clamp.set_child(Some(&status_box));

    // Separator
    let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);

    // Transcription section header
    let section_header = gtk4::Label::builder()
        .label("Last transcription")
        .css_classes(["heading", "dim-label"])
        .xalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(4)
        .build();

    // Transcription label
    let transcription_label = gtk4::Label::builder()
        .label("Hold Right Ctrl and speak...")
        .css_classes(["body", "dim-label"])
        .selectable(true)
        .wrap(true)
        .wrap_mode(gtk4::pango::WrapMode::WordChar)
        .xalign(0.0)
        .yalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(12)
        .build();

    // Transcription clamp
    let transcription_clamp = adw::Clamp::builder().maximum_size(600).build();
    transcription_clamp.set_child(Some(&transcription_label));

    // Scrolled window for transcription
    let scrolled_window = gtk4::ScrolledWindow::builder()
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .min_content_height(120)
        .build();
    scrolled_window.set_child(Some(&transcription_clamp));

    // Transcription area box
    let transcription_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();
    transcription_box.append(&section_header);
    transcription_box.append(&scrolled_window);

    // Main content box
    let content_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();
    content_box.append(&status_clamp);
    content_box.append(&separator);
    content_box.append(&transcription_box);

    // Toast overlay
    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&content_box));

    // Fix button click handler — runs pkexec in a background thread
    {
        let toast_overlay = toast_overlay.clone();
        let fix_button_ref = fix_button.clone();
        fix_button.connect_clicked(move |btn| {
            let user = std::env::var("USER").unwrap_or_default();
            if user.is_empty() {
                let toast = adw::Toast::new("Could not determine current username");
                toast.set_timeout(5);
                toast_overlay.add_toast(toast);
                return;
            }

            btn.set_sensitive(false);
            btn.set_label("Requesting permissions...");

            let (result_tx, result_rx) = async_channel::bounded::<bool>(1);
            std::thread::spawn(move || {
                let ok = std::process::Command::new("pkexec")
                    .args(["usermod", "-aG", "input", &user])
                    .status()
                    .is_ok_and(|s| s.success());
                let _ = result_tx.send_blocking(ok);
            });

            let toast_overlay = toast_overlay.clone();
            let button = fix_button_ref.clone();
            glib::spawn_future_local(async move {
                if let Ok(success) = result_rx.recv().await {
                    if success {
                        button.set_visible(false);

                        // Check if sg command exists to determine the restart method
                        let has_sg = which::which("sg").is_ok();

                        let msg = if has_sg {
                            "Permissions granted \u{2014} restarting with new group..."
                        } else {
                            "Permissions granted \u{2014} log out and back in to apply"
                        };

                        let toast = adw::Toast::new(msg);
                        toast.set_timeout(if has_sg { 2 } else { 0 });
                        toast.set_priority(adw::ToastPriority::High);
                        toast_overlay.add_toast(toast);

                        if has_sg {
                            // Wait 2 seconds for user to see the toast, then restart with sg
                            glib::timeout_add_seconds_local_once(2, || {
                                restart_app();
                            });
                        }
                    } else {
                        let toast = adw::Toast::new("Permission request denied or failed");
                        toast.set_timeout(5);
                        toast_overlay.add_toast(toast);
                        button.set_sensitive(true);
                        button.set_label("Fix Input Permissions");
                    }
                }
            });
        });
    }

    // Header bar
    let window_title = adw::WindowTitle::builder()
        .title("Escucha")
        .subtitle("Detecting device...")
        .build();

    let header_bar = adw::HeaderBar::new();
    header_bar.set_title_widget(Some(&window_title));

    // Toolbar view
    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.set_content(Some(&toast_overlay));

    // Window
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Escucha")
        .default_width(420)
        .default_height(520)
        .content(&toolbar_view)
        .build();
    window.set_size_request(360, 400);

    // --- Service communication ---

    let (tx, rx) = async_channel::unbounded::<ServiceMessage>();
    let shutdown_flag = Arc::new(AtomicBool::new(false));

    // Start the service thread
    let service_tx = tx.clone();
    let service_shutdown = shutdown_flag.clone();
    std::thread::spawn(move || {
        // Run preflight checks before starting the service
        let report = crate::preflight::check_environment();
        if report.has_critical_failures() {
            let _ = service_tx.send_blocking(ServiceMessage::Error(
                report.critical_failure_summary(),
            ));
            // Offer a fix button if input device access is the issue
            let input_failed = report
                .checks
                .iter()
                .any(|c| c.name == "input devices" && !c.passed);
            if input_failed {
                let _ = service_tx.send_blocking(ServiceMessage::InputFixAvailable);
            }
            let _ = service_tx.send_blocking(ServiceMessage::Status(ServiceStatus::Stopped));
            return;
        }
        if report.has_warnings() {
            for check in &report.checks {
                if !check.passed {
                    let msg = match &check.hint {
                        Some(hint) => format!("{}: {} ({})", check.name, check.message, hint),
                        None => format!("{}: {}", check.name, check.message),
                    };
                    let _ = service_tx.send_blocking(ServiceMessage::Error(msg));
                }
            }
        }

        let settings = match config::load_settings() {
            Ok(s) => s,
            Err(e) => {
                let _ =
                    service_tx.send_blocking(ServiceMessage::Error(format!("Config error: {e}")));
                let _ = service_tx.send_blocking(ServiceMessage::Status(ServiceStatus::Stopped));
                return;
            }
        };

        match crate::service::DictationService::new(settings) {
            Ok(service) => {
                let svc_shutdown = service.shutdown_handle();
                let shutdown_watcher = service_shutdown.clone();
                std::thread::spawn(move || {
                    while !shutdown_watcher.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    svc_shutdown.store(true, Ordering::Relaxed);
                });

                let _ = service_tx.send_blocking(ServiceMessage::Device(service.device_label()));

                let mut callbacks = GuiCallbacks { tx: service_tx };
                if let Err(e) = service.run_loop(&mut callbacks) {
                    log::error!("Service error: {e}");
                }
            }
            Err(e) => {
                let _ = service_tx.send_blocking(ServiceMessage::Error(format!("{e}")));
                let _ = service_tx.send_blocking(ServiceMessage::Status(ServiceStatus::Stopped));
            }
        }
    });

    // Handle shutdown on window close
    let close_shutdown = shutdown_flag.clone();
    window.connect_close_request(move |_| {
        close_shutdown.store(true, Ordering::Relaxed);
        glib::Propagation::Proceed
    });

    // --- Message receiver ---

    let status_msg_text = std::rc::Rc::new(std::cell::RefCell::new(String::new()));

    {
        let icon_stack = icon_stack.clone();
        let status_icon = status_icon.clone();
        let status_label = status_label.clone();
        let status_detail = status_detail.clone();
        let fix_button = fix_button.clone();
        let transcription_label = transcription_label.clone();
        let toast_overlay = toast_overlay.clone();
        let window_title = window_title.clone();
        let status_msg_text = status_msg_text.clone();

        glib::spawn_future_local(async move {
            while let Ok(msg) = rx.recv().await {
                match msg {
                    ServiceMessage::Status(status) => match status {
                        ServiceStatus::Stopped => {
                            icon_stack.set_visible_child_name("icon");
                            status_icon.set_icon_name(Some("microphone-disabled-symbolic"));
                            status_icon.set_css_classes(&["status-icon", "dim-label"]);
                            status_label.set_text("Stopped");
                            status_label.set_css_classes(&["title-2", "dim-label"]);
                            status_detail.set_visible(false);
                            window_title.set_subtitle("");
                        }
                        ServiceStatus::Starting => {
                            icon_stack.set_visible_child_name("spinner");
                            status_label.set_text("Starting...");
                            status_label.set_css_classes(&["title-2"]);
                            let msg = status_msg_text.borrow();
                            if msg.is_empty() {
                                status_detail.set_visible(false);
                            } else {
                                status_detail.set_text(&msg);
                                status_detail.set_visible(true);
                            }
                        }
                        ServiceStatus::Ready => {
                            icon_stack.set_visible_child_name("icon");
                            status_icon.set_icon_name(Some("audio-input-microphone-symbolic"));
                            status_icon.set_css_classes(&["status-icon", "success"]);
                            status_label.set_text("Ready");
                            status_label.set_css_classes(&["title-2", "success"]);
                            status_detail.set_text("Hold Right Ctrl to speak");
                            status_detail.set_css_classes(&["dim-label"]);
                            status_detail.set_visible(true);
                            status_msg_text.borrow_mut().clear();
                        }
                        ServiceStatus::Recording => {
                            icon_stack.set_visible_child_name("icon");
                            status_icon.set_icon_name(Some("microphone-sensitivity-high-symbolic"));
                            status_icon.set_css_classes(&[
                                "status-icon",
                                "error",
                                "recording-pulse",
                            ]);
                            status_label.set_text("Recording...");
                            status_label.set_css_classes(&["title-2", "error"]);
                            status_detail.set_text("Release to transcribe");
                            status_detail.set_css_classes(&["dim-label"]);
                            status_detail.set_visible(true);
                        }
                        ServiceStatus::Transcribing => {
                            icon_stack.set_visible_child_name("spinner");
                            status_label.set_text("Transcribing...");
                            status_label.set_css_classes(&["title-2"]);
                            status_detail.set_visible(false);
                        }
                        ServiceStatus::Stopping => {
                            icon_stack.set_visible_child_name("spinner");
                            status_label.set_text("Stopping...");
                            status_label.set_css_classes(&["title-2", "dim-label"]);
                            status_detail.set_visible(false);
                        }
                    },
                    ServiceMessage::StatusMsg(msg) => {
                        *status_msg_text.borrow_mut() = msg.clone();
                        status_detail.set_text(&msg);
                        status_detail.set_visible(!msg.is_empty());
                    }
                    ServiceMessage::Text(text) => {
                        if text.is_empty() {
                            transcription_label.set_text("Hold Right Ctrl and speak...");
                            transcription_label.add_css_class("dim-label");
                        } else {
                            transcription_label.set_text(&text);
                            transcription_label.remove_css_class("dim-label");
                        }
                    }
                    ServiceMessage::Error(error) => {
                        let toast = adw::Toast::new(&error);
                        toast.set_timeout(5);
                        toast.set_priority(adw::ToastPriority::High);
                        toast_overlay.add_toast(toast);
                    }
                    ServiceMessage::Device(label) => {
                        let display_name = strip_device_prefix(&label);
                        window_title.set_subtitle(display_name);
                    }
                    ServiceMessage::InputFixAvailable => {
                        fix_button.set_visible(true);
                    }
                }
            }
        });
    }

    window.present();
}

pub fn run_gui() -> Result<()> {
    let app = adw::Application::builder()
        .application_id("io.github.escucha")
        .build();

    app.connect_activate(build_ui);

    let exit_code = app.run_with_args::<String>(&[]);

    if exit_code != glib::ExitCode::SUCCESS {
        anyhow::bail!("GUI exited with error");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ServiceCallbacks;

    #[test]
    fn test_service_message_types() {
        let msg = ServiceMessage::Status(ServiceStatus::Ready);
        assert!(matches!(msg, ServiceMessage::Status(ServiceStatus::Ready)));

        let msg = ServiceMessage::Text("hello".to_string());
        assert!(matches!(msg, ServiceMessage::Text(_)));

        let msg = ServiceMessage::Error("oops".to_string());
        assert!(matches!(msg, ServiceMessage::Error(_)));

        let msg = ServiceMessage::Device("dev".to_string());
        assert!(matches!(msg, ServiceMessage::Device(_)));

        let msg = ServiceMessage::StatusMsg("loading".to_string());
        assert!(matches!(msg, ServiceMessage::StatusMsg(_)));

        let msg = ServiceMessage::InputFixAvailable;
        assert!(matches!(msg, ServiceMessage::InputFixAvailable));
    }

    #[test]
    fn test_gui_callbacks_send() {
        let (tx, _rx) = async_channel::unbounded();
        let mut cb = GuiCallbacks { tx };

        // These just test that send doesn't panic (receiver may be dropped)
        cb.on_status(ServiceStatus::Recording);
        cb.on_text("test text");
        cb.on_error("test error");
        cb.on_status_msg("downloading");
    }

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
