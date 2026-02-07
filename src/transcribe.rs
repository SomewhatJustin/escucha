use anyhow::{Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const HF_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

pub struct Transcriber {
    ctx: WhisperContext,
    language: String,
}

impl Transcriber {
    /// Load a Whisper model.
    pub fn new(model_path: &Path, language: &str) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap_or(""),
            WhisperContextParameters::default(),
        )
        .context("Failed to load Whisper model")?;

        Ok(Self {
            ctx,
            language: language.to_string(),
        })
    }

    /// Transcribe a WAV file and return the text.
    pub fn transcribe(&self, wav_path: &Path) -> Result<String> {
        let audio = load_wav_f32(wav_path)?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let mut state = self
            .ctx
            .create_state()
            .context("Failed to create Whisper state")?;

        state
            .full(params, &audio)
            .context("Whisper transcription failed")?;

        let num_segments = state
            .full_n_segments()
            .context("Failed to get segment count")?;

        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(&segment);
            }
        }

        Ok(normalize_whitespace(&text))
    }
}

/// Load a WAV file as f32 samples at 16kHz mono.
fn load_wav_f32(path: &Path) -> Result<Vec<f32>> {
    let reader = hound::WavReader::open(path)
        .with_context(|| format!("Failed to open WAV file: {}", path.display()))?;

    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };

    // If stereo, convert to mono by averaging channels
    if spec.channels == 2 {
        let mono: Vec<f32> = samples
            .chunks(2)
            .map(|chunk| {
                if chunk.len() == 2 {
                    (chunk[0] + chunk[1]) / 2.0
                } else {
                    chunk[0]
                }
            })
            .collect();
        Ok(mono)
    } else {
        Ok(samples)
    }
}

/// Normalize whitespace: trim and collapse multiple spaces.
pub fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Get the default model directory path.
pub fn default_model_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("escucha")
        .join("models")
}

/// Get the path for a model by name.
pub fn model_path(model_name: &str) -> PathBuf {
    default_model_dir().join(format!("ggml-{model_name}.bin"))
}

/// Download URL for a model.
fn model_url(model_name: &str) -> String {
    format!("{HF_BASE_URL}/ggml-{model_name}.bin")
}

/// Ensure the model exists locally, downloading it if needed.
/// Returns the path to the model file.
pub fn ensure_model(model_name: &str) -> Result<PathBuf> {
    let path = model_path(model_name);
    if path.exists() {
        return Ok(path);
    }

    let url = model_url(model_name);
    log::info!("Downloading Whisper model '{model_name}' from {url}");

    let dir = default_model_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create model dir {}", dir.display()))?;

    // Download with curl (available on virtually all Linux systems)
    let tmp_path = path.with_extension("bin.part");
    let status = std::process::Command::new("curl")
        .args([
            "-L",
            "--progress-bar",
            "-o",
            tmp_path.to_str().unwrap_or(""),
            &url,
        ])
        .status()
        .context("Failed to run curl. Is curl installed?")?;

    if !status.success() {
        // Clean up partial download
        let _ = std::fs::remove_file(&tmp_path);
        anyhow::bail!("Failed to download model from {url}");
    }

    // Verify we got something reasonable (> 1MB)
    let metadata = std::fs::metadata(&tmp_path).context("Downloaded file not found")?;
    if metadata.len() < 1_000_000 {
        let _ = std::fs::remove_file(&tmp_path);
        anyhow::bail!(
            "Downloaded file too small ({}B) - likely a download error",
            metadata.len()
        );
    }

    std::fs::rename(&tmp_path, &path).context("Failed to move downloaded model into place")?;

    log::info!("Model downloaded to {}", path.display());
    Ok(path)
}

/// Ensure the model exists, with a progress callback for GUI use.
pub fn ensure_model_with_status(
    model_name: &str,
    on_status: &mut dyn FnMut(&str),
) -> Result<PathBuf> {
    let path = model_path(model_name);
    if path.exists() {
        return Ok(path);
    }

    on_status(&format!("Downloading model '{model_name}'..."));

    let url = model_url(model_name);
    let dir = default_model_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create model dir {}", dir.display()))?;

    let tmp_path = path.with_extension("bin.part");

    // Use a simple HTTP download so we can report progress
    let output = std::process::Command::new("curl")
        .args([
            "-L",
            "--progress-bar",
            "-o",
            tmp_path.to_str().unwrap_or(""),
            &url,
        ])
        .stderr(std::process::Stdio::piped())
        .status()
        .context("Failed to run curl")?;

    if !output.success() {
        let _ = std::fs::remove_file(&tmp_path);
        anyhow::bail!("Download failed");
    }

    let metadata = std::fs::metadata(&tmp_path)?;
    if metadata.len() < 1_000_000 {
        let _ = std::fs::remove_file(&tmp_path);
        anyhow::bail!("Downloaded file too small - likely an error");
    }

    std::fs::rename(&tmp_path, &path)?;
    on_status("Model downloaded");

    // Flush any buffered output
    let _ = std::io::stdout().flush();

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_whitespace_basic() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
    }

    #[test]
    fn test_normalize_whitespace_empty() {
        assert_eq!(normalize_whitespace(""), "");
    }

    #[test]
    fn test_normalize_whitespace_single_word() {
        assert_eq!(normalize_whitespace("  hello  "), "hello");
    }

    #[test]
    fn test_normalize_whitespace_tabs_newlines() {
        assert_eq!(
            normalize_whitespace("hello\t\n  world\n\nfoo"),
            "hello world foo"
        );
    }

    #[test]
    fn test_normalize_whitespace_already_normal() {
        assert_eq!(normalize_whitespace("hello world"), "hello world");
    }

    #[test]
    fn test_model_path() {
        let path = model_path("base.en");
        assert!(path.to_string_lossy().contains("ggml-base.en.bin"));
    }

    #[test]
    fn test_model_url() {
        let url = model_url("base.en");
        assert!(url.contains("ggml-base.en.bin"));
        assert!(url.starts_with("https://huggingface.co/"));
    }

    #[test]
    fn test_model_path_large() {
        let path = model_path("large");
        assert!(path.to_string_lossy().contains("ggml-large.bin"));
    }

    #[test]
    fn test_load_wav_missing_file() {
        let result = load_wav_f32(Path::new("/tmp/nonexistent.wav"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_wav_16bit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wav");

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        writer.write_sample(0i16).unwrap();
        writer.write_sample(16383i16).unwrap(); // ~0.5
        writer.write_sample(-16384i16).unwrap(); // ~-0.5
        writer.finalize().unwrap();

        let samples = load_wav_f32(&path).unwrap();
        assert_eq!(samples.len(), 3);
        assert!((samples[0] - 0.0).abs() < 0.01);
        assert!((samples[1] - 0.5).abs() < 0.01);
        assert!((samples[2] + 0.5).abs() < 0.01);
    }

    #[test]
    fn test_load_wav_stereo_to_mono() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_stereo.wav");

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        writer.write_sample(16383i16).unwrap();
        writer.write_sample(16383i16).unwrap();
        writer.write_sample(32767i16).unwrap();
        writer.write_sample(0i16).unwrap();
        writer.finalize().unwrap();

        let samples = load_wav_f32(&path).unwrap();
        assert_eq!(samples.len(), 2);
        assert!((samples[0] - 0.5).abs() < 0.02);
        assert!((samples[1] - 0.5).abs() < 0.02);
    }
}
