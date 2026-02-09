use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Setup stages for LFM environment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SetupStage {
    NotStarted,
    CheckingPython,
    CreatingVenv,
    InstallingDependencies,
    DownloadingModel,
    Verifying,
    Complete,
    Failed(String),
}

/// Progress update during setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupProgress {
    pub stage: SetupStage,
    pub progress: f32,  // 0.0 - 1.0
    pub message: String,
    pub details: Option<String>,
}

impl SetupProgress {
    pub fn new(stage: SetupStage, progress: f32, message: impl Into<String>) -> Self {
        Self {
            stage,
            progress,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// LFM Setup Manager - handles venv creation and dependency installation
pub struct LfmSetupManager {
    pub backend_dir: PathBuf,
    pub venv_dir: PathBuf,
    pub sidecar_path: PathBuf,
}

impl LfmSetupManager {
    pub fn new(_app_data_dir: PathBuf) -> Self {
        // Always use backend directory for LFM
        let backend_dir = Self::get_backend_dir();
        let venv_dir = backend_dir.join("lfm_engine").join("venv");
        let sidecar_path = backend_dir.join("lfm_engine").join("sidecar.py");

        Self {
            backend_dir,
            venv_dir,
            sidecar_path,
        }
    }

    /// Get the backend directory path
    fn get_backend_dir() -> PathBuf {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(manifest_dir)
            .parent()
            .unwrap_or(&std::path::Path::new("."))
            .parent()
            .unwrap_or(&std::path::Path::new("."))
            .join("backend")
    }

    /// Get the path to Python in our venv
    pub fn get_python_path(&self) -> PathBuf {
        if cfg!(windows) {
            self.venv_dir.join("Scripts").join("python.exe")
        } else {
            self.venv_dir.join("bin").join("python")
        }
    }

    /// Check if setup is complete
    pub fn is_setup_complete(&self) -> bool {
        let python_path = self.get_python_path();
        python_path.exists()
    }

    /// Check if model is downloaded (check HuggingFace cache)
    pub fn is_model_downloaded(&self) -> bool {
        // Check common HuggingFace cache locations
        let hf_cache = if cfg!(windows) {
            dirs::home_dir().map(|h| h.join(".cache").join("huggingface").join("hub"))
        } else {
            dirs::home_dir().map(|h| h.join(".cache").join("huggingface").join("hub"))
        };

        if let Some(cache_dir) = hf_cache {
            // Look for LFM model in cache
            let model_marker = cache_dir.join("models--LiquidAI--LFM2.5-Audio-1.5B");
            model_marker.exists()
        } else {
            false
        }
    }

    /// Find system Python
    async fn find_python(&self) -> Result<String> {
        // Try python3 first, then python
        for cmd in &["python3", "python"] {
            let output = Command::new(cmd)
                .args(["-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"])
                .output()
                .await;

            if let Ok(output) = output {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let parts: Vec<&str> = version.split('.').collect();
                    if parts.len() >= 2 {
                        let major: u32 = parts[0].parse().unwrap_or(0);
                        let minor: u32 = parts[1].parse().unwrap_or(0);
                        if major >= 3 && minor >= 9 {
                            log::info!("Found Python {}.{} at {}", major, minor, cmd);
                            return Ok(cmd.to_string());
                        }
                    }
                }
            }
        }

        Err(anyhow!("Python 3.9+ not found. Please install Python from python.org"))
    }

    /// Run the full setup process with progress updates
    pub async fn run_setup(&self, progress_tx: mpsc::Sender<SetupProgress>) -> Result<()> {
        // Stage 1: Check Python
        progress_tx.send(SetupProgress::new(
            SetupStage::CheckingPython,
            0.05,
            "Checking Python installation..."
        )).await.ok();

        let python_cmd = self.find_python().await.map_err(|e| {
            let _ = progress_tx.try_send(SetupProgress::new(
                SetupStage::Failed(e.to_string()),
                0.0,
                "Python not found"
            ).with_details("Please install Python 3.9+ from python.org"));
            e
        })?;

        // Stage 2: Create venv
        progress_tx.send(SetupProgress::new(
            SetupStage::CreatingVenv,
            0.1,
            "Creating Python environment..."
        ).with_details(format!("Location: {}", self.venv_dir.display()))).await.ok();

        // Remove existing venv if corrupted
        if self.venv_dir.exists() && !self.get_python_path().exists() {
            log::info!("Removing corrupted venv");
            let _ = tokio::fs::remove_dir_all(&self.venv_dir).await;
        }

        if !self.venv_dir.exists() {
            let output = Command::new(&python_cmd)
                .args(["-m", "venv", self.venv_dir.to_str().unwrap()])
                .output()
                .await?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("Failed to create venv: {}", err));
            }
        }

        // Stage 3: Install dependencies
        progress_tx.send(SetupProgress::new(
            SetupStage::InstallingDependencies,
            0.15,
            "Installing dependencies..."
        ).with_details("This may take a few minutes")).await.ok();

        let pip_path = if cfg!(windows) {
            self.venv_dir.join("Scripts").join("pip.exe")
        } else {
            self.venv_dir.join("bin").join("pip")
        };

        // Upgrade pip first
        let _ = Command::new(&pip_path)
            .args(["install", "--upgrade", "pip"])
            .output()
            .await;

        // Install liquid-audio with progress tracking
        let mut child = Command::new(&pip_path)
            .args(["install", "liquid-audio", "--progress-bar", "on"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stderr = child.stderr.take().unwrap();
        let mut reader = BufReader::new(stderr).lines();

        let progress_tx_clone = progress_tx.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                // Parse pip progress if available
                if line.contains("Downloading") || line.contains("Installing") {
                    let _ = progress_tx_clone.send(SetupProgress::new(
                        SetupStage::InstallingDependencies,
                        0.3,
                        "Installing dependencies..."
                    ).with_details(line)).await;
                }
            }
        });

        let status = child.wait().await?;
        if !status.success() {
            return Err(anyhow!("Failed to install liquid-audio package"));
        }

        progress_tx.send(SetupProgress::new(
            SetupStage::InstallingDependencies,
            0.5,
            "Dependencies installed"
        )).await.ok();

        // Stage 4: Download model
        progress_tx.send(SetupProgress::new(
            SetupStage::DownloadingModel,
            0.55,
            "Downloading LFM2.5-Audio model..."
        ).with_details("~3GB download - this may take several minutes")).await.ok();

        // Run a Python script to download the model with progress
        let download_script = r#"
import sys
import os

# Suppress warnings
os.environ['TOKENIZERS_PARALLELISM'] = 'false'

try:
    from huggingface_hub import snapshot_download
    from huggingface_hub.utils import tqdm as hf_tqdm

    # Download model
    print("PROGRESS:0.6:Downloading model files...", flush=True)

    path = snapshot_download(
        "LiquidAI/LFM2.5-Audio-1.5B",
        local_files_only=False,
    )

    print("PROGRESS:0.9:Model downloaded successfully", flush=True)
    print(f"MODEL_PATH:{path}", flush=True)

except Exception as e:
    print(f"ERROR:{e}", flush=True)
    sys.exit(1)
"#;

        let python_path = self.get_python_path();
        let mut child = Command::new(&python_path)
            .args(["-c", download_script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            if line.starts_with("PROGRESS:") {
                let parts: Vec<&str> = line.splitn(3, ':').collect();
                if parts.len() >= 3 {
                    if let Ok(prog) = parts[1].parse::<f32>() {
                        progress_tx.send(SetupProgress::new(
                            SetupStage::DownloadingModel,
                            prog,
                            parts[2]
                        )).await.ok();
                    }
                }
            } else if line.starts_with("ERROR:") {
                let err = line.strip_prefix("ERROR:").unwrap_or(&line);
                return Err(anyhow!("Model download failed: {}", err));
            }
        }

        let status = child.wait().await?;
        if !status.success() {
            return Err(anyhow!("Model download failed"));
        }

        // Stage 5: Verify installation
        progress_tx.send(SetupProgress::new(
            SetupStage::Verifying,
            0.95,
            "Verifying installation..."
        )).await.ok();

        let verify_script = r#"
from liquid_audio import ChatState, LFM2AudioModel, LFM2AudioProcessor
print("OK")
"#;

        let output = Command::new(&python_path)
            .args(["-c", verify_script])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Verification failed: {}", err));
        }

        // Complete!
        progress_tx.send(SetupProgress::new(
            SetupStage::Complete,
            1.0,
            "Setup complete!"
        ).with_details("LFM2.5-Audio is ready to use")).await.ok();

        Ok(())
    }

    /// Get setup status
    pub fn get_setup_status(&self) -> SetupStage {
        if !self.is_setup_complete() {
            SetupStage::NotStarted
        } else if !self.is_model_downloaded() {
            SetupStage::DownloadingModel
        } else {
            SetupStage::Complete
        }
    }
}
