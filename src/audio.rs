use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Handle to an in-progress audio recording via arecord.
pub struct Recording {
    child: Child,
    path: PathBuf,
}

impl Recording {
    /// Start recording audio to a WAV file using arecord.
    /// Format: 16kHz, mono, S16_LE PCM.
    pub fn start(output_path: &Path) -> Result<Self> {
        let child = Command::new("arecord")
            .args([
                "-f",
                "S16_LE",
                "-r",
                "16000",
                "-c",
                "1",
                "-t",
                "wav",
                output_path.to_str().unwrap_or("recording.wav"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start arecord. Is alsa-utils installed?")?;

        Ok(Self {
            child,
            path: output_path.to_path_buf(),
        })
    }

    /// Stop recording and return the path to the WAV file.
    pub fn stop(mut self) -> Result<PathBuf> {
        // Send SIGTERM for graceful shutdown
        let pid = self.child.id();
        if let Err(e) = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid as i32),
            nix::sys::signal::Signal::SIGTERM,
        ) {
            log::warn!("Failed to send SIGTERM to arecord (pid {pid}): {e}");
            // Try regular kill as fallback
            let _ = self.child.kill();
        }

        self.child
            .wait()
            .context("Failed to wait for arecord to stop")?;

        if !self.path.exists() {
            bail!("Recording file not found: {}", self.path.display());
        }

        Ok(self.path)
    }

    /// Get the output file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Create a temporary WAV file path for recording.
pub fn temp_wav_path() -> Result<PathBuf> {
    let dir = tempfile::tempdir().context("Failed to create temp dir")?;
    // We leak the tempdir so it doesn't get cleaned up
    // The caller is responsible for cleaning up the WAV file
    let path = dir.path().join("escucha_recording.wav");
    std::mem::forget(dir);
    Ok(path)
}

/// Clean up a recording file.
pub fn cleanup_recording(path: &Path) {
    if path.exists()
        && let Err(e) = std::fs::remove_file(path)
    {
        log::warn!("Failed to clean up {}: {e}", path.display());
    }
    // Also try to remove the parent temp directory
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

/// Check if arecord is available on the system.
pub fn check_arecord() -> bool {
    which::which("arecord").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_arecord() {
        // Just verify it doesn't panic
        let _available = check_arecord();
    }

    #[test]
    fn test_temp_wav_path() {
        let path = temp_wav_path().unwrap();
        assert!(path.to_string_lossy().contains("escucha_recording.wav"));
        // Clean up
        cleanup_recording(&path);
    }

    #[test]
    fn test_cleanup_nonexistent() {
        // Should not panic
        cleanup_recording(Path::new("/tmp/nonexistent_escucha_test.wav"));
    }

    #[test]
    fn test_cleanup_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wav");
        std::fs::write(&path, b"fake wav data").unwrap();
        assert!(path.exists());
        cleanup_recording(&path);
        assert!(!path.exists());
    }
}
