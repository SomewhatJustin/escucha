use anyhow::{Context, Result};
use evdev::{EventType, InputEventKind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use crate::audio::{self, Recording};
use crate::config::Settings;
use crate::input;
use crate::paste::{self, PasteConfig};
use crate::transcribe::Transcriber;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Ready,
    Recording,
    Transcribing,
    Stopping,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceStatus::Stopped => write!(f, "stopped"),
            ServiceStatus::Starting => write!(f, "starting"),
            ServiceStatus::Ready => write!(f, "ready"),
            ServiceStatus::Recording => write!(f, "recording"),
            ServiceStatus::Transcribing => write!(f, "transcribing"),
            ServiceStatus::Stopping => write!(f, "stopping"),
        }
    }
}

/// Callbacks for the dictation service to report status changes.
pub trait ServiceCallbacks: Send {
    fn on_status(&mut self, status: ServiceStatus);
    fn on_status_msg(&mut self, msg: &str);
    fn on_text(&mut self, text: &str);
    fn on_error(&mut self, error: &str);
}

/// No-op callbacks for daemon mode (just logs).
struct LogCallbacks;

impl ServiceCallbacks for LogCallbacks {
    fn on_status(&mut self, status: ServiceStatus) {
        log::info!("Status: {status}");
    }
    fn on_status_msg(&mut self, msg: &str) {
        log::info!("{msg}");
    }
    fn on_text(&mut self, text: &str) {
        log::info!("Transcribed: {text}");
    }
    fn on_error(&mut self, error: &str) {
        log::error!("Error: {error}");
    }
}

/// Key events sent from the reader thread.
#[derive(Debug)]
enum KeyEvent {
    Press,
    Release,
    Error(String),
}

pub struct DictationService {
    settings: Settings,
    device_path: PathBuf,
    key: evdev::Key,
    paste_config: PasteConfig,
    shutdown: Arc<AtomicBool>,
}

impl DictationService {
    pub fn new(settings: Settings) -> Result<Self> {
        let key = input::resolve_key(&settings.key)?;
        let device_path = input::pick_keyboard_device(&settings.keyboard_device, key)?;
        let paste_method = paste::pick_paste_method(&settings.paste_method)?;

        let paste_config = PasteConfig {
            method: paste_method,
            hotkey: settings.paste_hotkey.clone(),
            clipboard_paste: settings.clipboard_paste.clone(),
            clipboard_paste_delay_ms: settings.clipboard_paste_delay_ms,
        };

        log::info!("Key: {} ({:?})", settings.key, key);
        log::info!("Device: {}", device_path.display());
        log::info!("Paste method: {paste_method}");
        log::info!("Model: {}", settings.model);

        Ok(Self {
            settings,
            device_path,
            key,
            paste_config,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Get a handle to request shutdown.
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    /// Human-readable label for the active input device.
    pub fn device_label(&self) -> String {
        // Include the device name if we can open it
        if let Ok(dev) = evdev::Device::open(&self.device_path) {
            let name = dev.name().unwrap_or("Unknown");
            format!("{} - {}", self.device_path.display(), name)
        } else {
            self.device_path.display().to_string()
        }
    }

    /// Run the main event loop.
    pub fn run_loop(&self, callbacks: &mut dyn ServiceCallbacks) -> Result<()> {
        callbacks.on_status(ServiceStatus::Starting);

        // Download model if missing
        let model_path =
            crate::transcribe::ensure_model_with_status(&self.settings.model, &mut |status| {
                callbacks.on_status_msg(status)
            })?;

        callbacks.on_status_msg("Loading model...");
        let transcriber = Transcriber::new(&model_path, &self.settings.language)
            .context("Failed to load Whisper model")?;

        // Spawn a dedicated thread to read evdev events.
        // This avoids issues with poll + fetch_events interaction.
        let (key_tx, key_rx) = mpsc::channel();
        let device_path = self.device_path.clone();
        let target_key = self.key;
        let shutdown_reader = self.shutdown.clone();

        std::thread::spawn(move || {
            let mut device = match evdev::Device::open(&device_path) {
                Ok(d) => d,
                Err(e) => {
                    let _ = key_tx.send(KeyEvent::Error(format!(
                        "Failed to open {}: {e}",
                        device_path.display()
                    )));
                    return;
                }
            };

            log::info!(
                "Opened device: {} ({})",
                device_path.display(),
                device.name().unwrap_or("Unknown")
            );

            while !shutdown_reader.load(Ordering::Relaxed) {
                // fetch_events blocks until events are available
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() != EventType::KEY {
                                continue;
                            }
                            if let InputEventKind::Key(key) = event.kind() {
                                if key != target_key {
                                    continue;
                                }
                                let ke = match event.value() {
                                    1 => KeyEvent::Press,
                                    0 => KeyEvent::Release,
                                    _ => continue, // repeat, ignore
                                };
                                if key_tx.send(ke).is_err() {
                                    return; // main thread gone
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if shutdown_reader.load(Ordering::Relaxed) {
                            return;
                        }
                        let _ = key_tx.send(KeyEvent::Error(format!("Event read error: {e}")));
                        return;
                    }
                }
            }
        });

        callbacks.on_status(ServiceStatus::Ready);
        log::info!("Ready. Hold {:?} to dictate.", self.key);

        let mut recording: Option<Recording> = None;

        loop {
            // Wait for key events with timeout so we can check shutdown
            match key_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(KeyEvent::Press) => {
                    if recording.is_some() {
                        continue;
                    }
                    callbacks.on_status(ServiceStatus::Recording);
                    match audio::temp_wav_path() {
                        Ok(wav_path) => match Recording::start(&wav_path) {
                            Ok(rec) => {
                                log::info!("Recording started");
                                recording = Some(rec);
                            }
                            Err(e) => {
                                callbacks.on_error(&format!("Failed to start recording: {e}"));
                                callbacks.on_status(ServiceStatus::Ready);
                            }
                        },
                        Err(e) => {
                            callbacks.on_error(&format!("Failed to create temp file: {e}"));
                            callbacks.on_status(ServiceStatus::Ready);
                        }
                    }
                }
                Ok(KeyEvent::Release) => {
                    if let Some(rec) = recording.take() {
                        callbacks.on_status(ServiceStatus::Transcribing);
                        match rec.stop() {
                            Ok(wav_path) => {
                                match transcriber.transcribe(&wav_path) {
                                    Ok(text) => {
                                        if !text.is_empty() {
                                            callbacks.on_text(&text);
                                            if let Err(e) =
                                                paste::paste_text(&text, &self.paste_config)
                                            {
                                                callbacks.on_error(&format!("Paste failed: {e}"));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        callbacks.on_error(&format!("Transcription failed: {e}"));
                                    }
                                }
                                audio::cleanup_recording(&wav_path);
                            }
                            Err(e) => {
                                callbacks.on_error(&format!("Failed to stop recording: {e}"));
                            }
                        }
                        callbacks.on_status(ServiceStatus::Ready);
                    }
                }
                Ok(KeyEvent::Error(e)) => {
                    callbacks.on_error(&e);
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    callbacks.on_error("Event reader thread exited");
                    break;
                }
            }

            if self.shutdown.load(Ordering::Relaxed) {
                callbacks.on_status(ServiceStatus::Stopping);
                break;
            }
        }

        // Cleanup any in-progress recording
        if let Some(rec) = recording
            && let Ok(path) = rec.stop()
        {
            audio::cleanup_recording(&path);
        }

        callbacks.on_status(ServiceStatus::Stopped);
        Ok(())
    }
}

/// Global shutdown flag for signal handler.
static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);

/// Run as a daemon (default mode).
pub fn run_daemon() -> Result<()> {
    let settings = crate::config::load_settings()?;

    let report = crate::preflight::check_environment();
    if report.has_critical_failures() {
        anyhow::bail!("{}", report.critical_failure_summary());
    }

    let service = DictationService::new(settings)?;

    let shutdown = service.shutdown_handle();
    SHUTDOWN_FLAG.store(false, Ordering::Relaxed);

    unsafe {
        libc::signal(
            libc::SIGTERM,
            signal_handler as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGINT,
            signal_handler as *const () as libc::sighandler_t,
        );
    }

    let shutdown_clone = shutdown.clone();
    std::thread::spawn(move || {
        loop {
            if SHUTDOWN_FLAG.load(Ordering::Relaxed) {
                shutdown_clone.store(true, Ordering::Relaxed);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    let mut callbacks = LogCallbacks;
    service.run_loop(&mut callbacks)
}

extern "C" fn signal_handler(_sig: libc::c_int) {
    SHUTDOWN_FLAG.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_status_display() {
        assert_eq!(ServiceStatus::Stopped.to_string(), "stopped");
        assert_eq!(ServiceStatus::Starting.to_string(), "starting");
        assert_eq!(ServiceStatus::Ready.to_string(), "ready");
        assert_eq!(ServiceStatus::Recording.to_string(), "recording");
        assert_eq!(ServiceStatus::Transcribing.to_string(), "transcribing");
        assert_eq!(ServiceStatus::Stopping.to_string(), "stopping");
    }

    #[test]
    fn test_service_status_equality() {
        assert_eq!(ServiceStatus::Ready, ServiceStatus::Ready);
        assert_ne!(ServiceStatus::Ready, ServiceStatus::Recording);
    }

    struct TestCallbacks {
        statuses: Vec<ServiceStatus>,
        texts: Vec<String>,
        errors: Vec<String>,
    }

    impl TestCallbacks {
        fn new() -> Self {
            Self {
                statuses: Vec::new(),
                texts: Vec::new(),
                errors: Vec::new(),
            }
        }
    }

    impl ServiceCallbacks for TestCallbacks {
        fn on_status(&mut self, status: ServiceStatus) {
            self.statuses.push(status);
        }
        fn on_status_msg(&mut self, _msg: &str) {}
        fn on_text(&mut self, text: &str) {
            self.texts.push(text.to_string());
        }
        fn on_error(&mut self, error: &str) {
            self.errors.push(error.to_string());
        }
    }

    #[test]
    fn test_callbacks() {
        let mut cb = TestCallbacks::new();
        cb.on_status(ServiceStatus::Ready);
        cb.on_text("hello world");
        cb.on_error("test error");

        assert_eq!(cb.statuses, vec![ServiceStatus::Ready]);
        assert_eq!(cb.texts, vec!["hello world"]);
        assert_eq!(cb.errors, vec!["test error"]);
    }
}
