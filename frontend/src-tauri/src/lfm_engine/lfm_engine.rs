use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Model status for LFM Audio engine
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelStatus {
    NotInstalled,  // Python package not installed
    Available,     // Ready to use
    Loading,       // Model is loading
    Loaded,        // Model loaded in sidecar
    Error(String),
}

/// LFM Audio Engine Error
#[derive(Debug)]
pub enum LfmEngineError {
    SidecarNotRunning,
    PythonNotFound,
    PackageNotInstalled,
    ModelLoadFailed(String),
    TranscriptionFailed(String),
    IoError(std::io::Error),
    Other(String),
}

impl std::fmt::Display for LfmEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LfmEngineError::SidecarNotRunning => write!(f, "LFM Audio sidecar is not running"),
            LfmEngineError::PythonNotFound => write!(f, "Python not found. Please install Python 3.9+"),
            LfmEngineError::PackageNotInstalled => write!(f, "liquid-audio package not installed. Run: pip install liquid-audio"),
            LfmEngineError::ModelLoadFailed(err) => write!(f, "Failed to load model: {}", err),
            LfmEngineError::TranscriptionFailed(err) => write!(f, "Transcription failed: {}", err),
            LfmEngineError::IoError(err) => write!(f, "IO error: {}", err),
            LfmEngineError::Other(err) => write!(f, "Error: {}", err),
        }
    }
}

impl std::error::Error for LfmEngineError {}

impl From<std::io::Error> for LfmEngineError {
    fn from(err: std::io::Error) -> Self {
        LfmEngineError::IoError(err)
    }
}

/// Request to sidecar
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum SidecarRequest {
    LoadModel { model_id: String },
    Transcribe { audio_path: String },
    TranscribeSamples { samples: Vec<f32>, sample_rate: u32 },
    Unload,
    Ping,
    Shutdown,
}

/// Response from sidecar
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SidecarResponse {
    Response { text: String, error: Option<String> },
    Pong,
    Goodbye,
    Error { message: String },
}

/// LFM Audio Engine - manages Python sidecar for Liquid AI ASR
pub struct LfmEngine {
    sidecar_path: PathBuf,
    venv_python_path: PathBuf,
    sidecar_process: Arc<RwLock<Option<Child>>>,
    status: Arc<RwLock<ModelStatus>>,
    current_model: Arc<RwLock<Option<String>>>,
}

impl LfmEngine {
    /// Create a new LFM engine with the sidecar script path
    pub fn new(_app_data_dir: Option<PathBuf>) -> Result<Self> {
        // Always use backend directory for LFM engine
        let backend_dir = Self::get_backend_dir();

        let sidecar_path = backend_dir.join("lfm_engine").join("sidecar.py");

        // Venv is always in backend/lfm_engine/venv
        let venv_python_path = if cfg!(windows) {
            backend_dir.join("lfm_engine").join("venv").join("Scripts").join("python.exe")
        } else {
            backend_dir.join("lfm_engine").join("venv").join("bin").join("python")
        };

        log::info!("LfmEngine sidecar path: {}", sidecar_path.display());
        log::info!("LfmEngine venv Python: {}", venv_python_path.display());

        Ok(Self {
            sidecar_path,
            venv_python_path,
            sidecar_process: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(ModelStatus::NotInstalled)),
            current_model: Arc::new(RwLock::new(None)),
        })
    }

    /// Get the backend directory path
    fn get_backend_dir() -> PathBuf {
        // In development, resolve from CARGO_MANIFEST_DIR
        // In production, this would be bundled differently
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(manifest_dir)
            .parent()
            .unwrap_or(&PathBuf::from("."))
            .parent()
            .unwrap_or(&PathBuf::from("."))
            .join("backend")
    }

    /// Check if Python and liquid-audio are available in the venv
    pub async fn check_dependencies(&self) -> Result<bool> {
        // Use venv Python if available
        let python_cmd = if self.venv_python_path.exists() {
            self.venv_python_path.to_string_lossy().to_string()
        } else {
            // Venv not set up yet
            log::info!("LFM venv not found at: {}", self.venv_python_path.display());
            *self.status.write().await = ModelStatus::NotInstalled;
            return Ok(false);
        };

        // Check Python
        let python_check = Command::new(&python_cmd)
            .args(["-c", "import sys; print(sys.version_info[:2])"])
            .output();

        match python_check {
            Ok(output) if output.status.success() => {
                log::info!("Python found: {}", String::from_utf8_lossy(&output.stdout).trim());
            }
            _ => {
                log::warn!("Python not working in venv");
                *self.status.write().await = ModelStatus::NotInstalled;
                return Ok(false);
            }
        }

        // Check liquid-audio package
        let package_check = Command::new(&python_cmd)
            .args(["-c", "from liquid_audio import ChatState, LFM2AudioModel; print('ok')"])
            .output();

        match package_check {
            Ok(output) if output.status.success() => {
                log::info!("liquid-audio package found in venv");
                *self.status.write().await = ModelStatus::Available;
                Ok(true)
            }
            _ => {
                log::warn!("liquid-audio package not installed in venv");
                *self.status.write().await = ModelStatus::NotInstalled;
                Ok(false)
            }
        }
    }

    /// Get current status
    pub async fn get_status(&self) -> ModelStatus {
        self.status.read().await.clone()
    }

    /// Start the sidecar process
    async fn start_sidecar(&self) -> Result<()> {
        let mut process_guard = self.sidecar_process.write().await;

        // Check if already running
        if let Some(ref mut child) = *process_guard {
            match child.try_wait() {
                Ok(None) => {
                    log::debug!("Sidecar already running");
                    return Ok(());
                }
                _ => {
                    log::info!("Sidecar process ended, restarting...");
                }
            }
        }

        log::info!("Starting LFM Audio sidecar: {}", self.sidecar_path.display());
        log::info!("Using Python: {}", self.venv_python_path.display());

        // Use venv Python if available, otherwise fall back to system Python
        let python_cmd = if self.venv_python_path.exists() {
            self.venv_python_path.to_string_lossy().to_string()
        } else {
            "python3".to_string()
        };

        let child = Command::new(&python_cmd)
            .arg(&self.sidecar_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Show sidecar logs
            .spawn()
            .map_err(|e| anyhow!("Failed to start sidecar: {}", e))?;

        *process_guard = Some(child);
        log::info!("LFM Audio sidecar started");

        Ok(())
    }

    /// Send request to sidecar and get response
    async fn send_request(&self, request: &SidecarRequest) -> Result<SidecarResponse> {
        // Ensure sidecar is running
        self.start_sidecar().await?;

        let mut process_guard = self.sidecar_process.write().await;
        let child = process_guard.as_mut()
            .ok_or_else(|| anyhow!("Sidecar not running"))?;

        // Send request
        let stdin = child.stdin.as_mut()
            .ok_or_else(|| anyhow!("Failed to get stdin"))?;

        let request_json = serde_json::to_string(request)?;
        writeln!(stdin, "{}", request_json)?;
        stdin.flush()?;

        // Read response
        let stdout = child.stdout.as_mut()
            .ok_or_else(|| anyhow!("Failed to get stdout"))?;

        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        let response: SidecarResponse = serde_json::from_str(&response_line)
            .map_err(|e| anyhow!("Failed to parse response: {} - {}", e, response_line))?;

        Ok(response)
    }

    /// Load the LFM2.5-Audio model
    pub async fn load_model(&self, model_id: Option<&str>) -> Result<()> {
        let model_id = model_id.unwrap_or("LiquidAI/LFM2.5-Audio-1.5B").to_string();

        // Check if already loaded
        if let Some(ref loaded) = *self.current_model.read().await {
            if loaded == &model_id {
                log::info!("Model {} already loaded", model_id);
                return Ok(());
            }
        }

        *self.status.write().await = ModelStatus::Loading;
        log::info!("Loading LFM model: {}", model_id);

        let response = self.send_request(&SidecarRequest::LoadModel {
            model_id: model_id.clone(),
        }).await?;

        match response {
            SidecarResponse::Response { error: None, .. } => {
                *self.status.write().await = ModelStatus::Loaded;
                *self.current_model.write().await = Some(model_id);
                log::info!("LFM model loaded successfully");
                Ok(())
            }
            SidecarResponse::Response { error: Some(err), .. } => {
                *self.status.write().await = ModelStatus::Error(err.clone());
                Err(anyhow!("Failed to load model: {}", err))
            }
            SidecarResponse::Error { message } => {
                *self.status.write().await = ModelStatus::Error(message.clone());
                Err(anyhow!("Sidecar error: {}", message))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Transcribe audio from file
    pub async fn transcribe_file(&self, audio_path: &str) -> Result<String> {
        // Ensure model is loaded
        if *self.status.read().await != ModelStatus::Loaded {
            self.load_model(None).await?;
        }

        log::info!("Transcribing file: {}", audio_path);

        let response = self.send_request(&SidecarRequest::Transcribe {
            audio_path: audio_path.to_string(),
        }).await?;

        match response {
            SidecarResponse::Response { text, error: None } => {
                log::info!("Transcription result: {} chars", text.len());
                Ok(text)
            }
            SidecarResponse::Response { error: Some(err), .. } => {
                Err(anyhow!("Transcription failed: {}", err))
            }
            SidecarResponse::Error { message } => {
                Err(anyhow!("Sidecar error: {}", message))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Transcribe audio from raw samples (f32, mono)
    pub async fn transcribe_samples(&self, samples: Vec<f32>, sample_rate: u32) -> Result<String> {
        // Ensure model is loaded
        if *self.status.read().await != ModelStatus::Loaded {
            self.load_model(None).await?;
        }

        log::debug!("Transcribing {} samples at {}Hz", samples.len(), sample_rate);

        let response = self.send_request(&SidecarRequest::TranscribeSamples {
            samples,
            sample_rate,
        }).await?;

        match response {
            SidecarResponse::Response { text, error: None } => {
                log::debug!("Transcription result: {} chars", text.len());
                Ok(text)
            }
            SidecarResponse::Response { error: Some(err), .. } => {
                Err(anyhow!("Transcription failed: {}", err))
            }
            SidecarResponse::Error { message } => {
                Err(anyhow!("Sidecar error: {}", message))
            }
            _ => Err(anyhow!("Unexpected response")),
        }
    }

    /// Unload model to free memory
    pub async fn unload_model(&self) -> Result<()> {
        if *self.status.read().await != ModelStatus::Loaded {
            return Ok(());
        }

        log::info!("Unloading LFM model");

        let response = self.send_request(&SidecarRequest::Unload).await?;

        match response {
            SidecarResponse::Response { error: None, .. } => {
                *self.status.write().await = ModelStatus::Available;
                *self.current_model.write().await = None;
                log::info!("LFM model unloaded");
                Ok(())
            }
            _ => Ok(()), // Ignore errors on unload
        }
    }

    /// Shutdown the sidecar
    pub async fn shutdown(&self) -> Result<()> {
        log::info!("Shutting down LFM sidecar");

        // Try graceful shutdown
        let _ = self.send_request(&SidecarRequest::Shutdown).await;

        // Kill process if still running
        let mut process_guard = self.sidecar_process.write().await;
        if let Some(ref mut child) = *process_guard {
            let _ = child.kill();
            let _ = child.wait();
        }
        *process_guard = None;

        *self.status.write().await = ModelStatus::Available;
        *self.current_model.write().await = None;

        log::info!("LFM sidecar shutdown complete");
        Ok(())
    }

    /// Check if model is loaded
    pub async fn is_model_loaded(&self) -> bool {
        *self.status.read().await == ModelStatus::Loaded
    }

    /// Get current model name
    pub async fn get_current_model(&self) -> Option<String> {
        self.current_model.read().await.clone()
    }
}

impl Drop for LfmEngine {
    fn drop(&mut self) {
        // Synchronous cleanup - best effort
        if let Ok(mut guard) = self.sidecar_process.try_write() {
            if let Some(ref mut child) = *guard {
                let _ = child.kill();
            }
        }
    }
}
