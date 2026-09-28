use anyhow::Result;
use std::path::PathBuf;
use tauri::{Emitter, Runtime};
use tauri::AppHandle;

const SEGMENTATION_MODEL_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-reconstruction-models/pyannote-segmentation-3.0.onnx";
const NEMO_TITANET_EMBEDDING_MODEL_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/nemo_en_titanet_small.onnx";

const SEGMENTATION_FILENAME: &str = "pyannote-segmentation.onnx";
const NEMO_TITANET_EMBEDDING_FILENAME: &str = "nemo-titanet-embedding.onnx";
/// Conv-TasNet 2-source separation (overlap-separation-prepass 2.1):
/// exported from the JorisCos/ConvTasNet_Libri2Mix_sepnoisy_16k checkpoint
/// via tools/export_conv_tasnet.py; the sha256 pin is the exported
/// artifact's, not the upstream pytorch checkpoint's.
const SEPARATION_FILENAME: &str = "conv_tasnet_libri2mix_sepnoisy_16k.onnx";
pub const SEPARATION_MODEL_SHA256: &str =
    "ed1f7aeeb6c90b20ea78178468393aa7502c406d6fde893ce27145fec1cb2d29";

pub fn embedding_filename() -> &'static str {
    NEMO_TITANET_EMBEDDING_FILENAME
}

/// Local path of the separation model — the separation pre-pass degrades to
/// mixture-only when this file is absent (it is NOT part of
/// `speaker_models_exist`, which gates the whole diarization path).
///
/// Resolution order: the runtime models dir first (a newer manual export or
/// future download wins), then the copy committed under `frontend/models/`
/// — the artifact is a public model (Asteroid MIT / LibriSpeech CC-BY-4.0),
/// so it ships with the repo and a fresh clone needs no hosting or manual
/// export. Dev/cargo resolution only: production bundling would go through
/// Tauri resources.
pub fn separation_model_path() -> PathBuf {
    let runtime = models_dir().join(SEPARATION_FILENAME);
    if runtime.exists() {
        return runtime;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../models").join(SEPARATION_FILENAME)
}

fn models_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".meetily-models")
}

pub fn speaker_models_exist() -> bool {
    let dir = models_dir();
    dir.join(NEMO_TITANET_EMBEDDING_FILENAME).exists() && dir.join(SEGMENTATION_FILENAME).exists()
}

#[derive(Clone, serde::Serialize)]
struct SpeakerModelDownloadProgress {
    model: String,
    progress: u32,
    downloaded_mb: f64,
    total_mb: f64,
    status: String,
}

#[derive(Clone, serde::Serialize)]
struct SpeakerModelDownloadError {
    model: String,
    error: String,
}

async fn download_file<R: Runtime>(
    app: &AppHandle<R>,
    url: &str,
    filename: &str,
    model_name: &str,
) -> Result<()> {
    let dir = models_dir();
    if !dir.exists() {
        tokio::fs::create_dir_all(&dir).await?;
    }
    let dest = dir.join(filename);

    if dest.exists() {
        let _ = app.emit(
            "speaker-model-download-progress",
            SpeakerModelDownloadProgress {
                model: model_name.to_string(),
                progress: 100,
                downloaded_mb: 0.0,
                total_mb: 0.0,
                status: "completed".to_string(),
            },
        );
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let response = client.get(url).send().await?;
    let total_size = response.content_length().unwrap_or(0) as f64 / (1024.0 * 1024.0);

    let _ = app.emit(
        "speaker-model-download-progress",
        SpeakerModelDownloadProgress {
            model: model_name.to_string(),
            progress: 0,
            downloaded_mb: 0.0,
            total_mb: total_size,
            status: "downloading".to_string(),
        },
    );

    let temp_path = dir.join(format!("{filename}.tmp"));
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_reported_pct: u32 = 0;

    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        let pct = if total_size > 0.0 {
            ((downloaded as f64 / (1024.0 * 1024.0)) / total_size * 100.0) as u32
        } else {
            0
        };

        if pct > last_reported_pct + 5 || pct == 100 {
            last_reported_pct = pct;
            let _ = app.emit(
                "speaker-model-download-progress",
                SpeakerModelDownloadProgress {
                    model: model_name.to_string(),
                    progress: pct.min(100),
                    downloaded_mb: downloaded as f64 / (1024.0 * 1024.0),
                    total_mb: total_size,
                    status: "downloading".to_string(),
                },
            );
        }
    }

    file.flush().await?;
    drop(file);
    tokio::fs::rename(&temp_path, &dest).await?;

    let _ = app.emit(
        "speaker-model-download-progress",
        SpeakerModelDownloadProgress {
            model: model_name.to_string(),
            progress: 100,
            downloaded_mb: downloaded as f64 / (1024.0 * 1024.0),
            total_mb: total_size,
            status: "completed".to_string(),
        },
    );

    Ok(())
}

#[tauri::command]
pub async fn download_speaker_models<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    if let Err(e) = download_file(&app, SEGMENTATION_MODEL_URL, SEGMENTATION_FILENAME, "pyannote-segmentation").await {
        let _ = app.emit(
            "speaker-model-download-error",
            SpeakerModelDownloadError {
                model: "pyannote-segmentation".to_string(),
                error: e.to_string(),
            },
        );
        log::warn!("Failed to download segmentation model: {}", e);
    }

    if let Err(e) = download_file(&app, NEMO_TITANET_EMBEDDING_MODEL_URL, NEMO_TITANET_EMBEDDING_FILENAME, "nemo-titanet-embedding").await {
        let _ = app.emit(
            "speaker-model-download-error",
            SpeakerModelDownloadError {
                model: "nemo-titanet-embedding".to_string(),
                error: e.to_string(),
            },
        );
        log::warn!("Failed to download embedding model: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub async fn check_speaker_models_available() -> bool {
    speaker_models_exist()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_dir_is_under_home() {
        let dir = models_dir();
        assert!(dir.to_string_lossy().contains(".meetily-models"));
    }

    #[test]
    fn urls_are_valid_https() {
        assert!(SEGMENTATION_MODEL_URL.starts_with("https://"));
        assert!(NEMO_TITANET_EMBEDDING_MODEL_URL.starts_with("https://"));
    }

    #[test]
    fn filenames_match_convention() {
        assert!(SEGMENTATION_FILENAME.ends_with(".onnx"));
        assert!(NEMO_TITANET_EMBEDDING_FILENAME.ends_with(".onnx"));
    }

    #[test]
    fn committed_separation_model_resolves_and_matches_pin() {
        // The committed copy under frontend/models/ is the provisioning
        // source for a fresh clone; the suite enforces the sha256 pin on it
        // directly (not via separation_model_path, whose runtime copy may be
        // newer) so the repo can never drift from the hash the adapter
        // trusts.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../models")
            .join("conv_tasnet_libri2mix_sepnoisy_16k.onnx");
        assert!(path.exists(), "committed separation model missing: {}", path.display());
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        let mut file = std::fs::File::open(&path).expect("open separation model");
        std::io::copy(&mut file, &mut h).expect("hash separation model");
        let got = format!("{:x}", h.finalize());
        assert_eq!(
            got, SEPARATION_MODEL_SHA256,
            "committed separation model drifted from the pinned sha256"
        );
    }
}
