use crate::lfm_engine::{LfmEngine, ModelStatus, LfmSetupManager, SetupProgress, SetupStage};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::{mpsc, RwLock};

/// State wrapper for LFM Engine
pub struct LfmEngineState(pub Arc<RwLock<Option<LfmEngine>>>);

/// State wrapper for LFM Setup Manager
pub struct LfmSetupState {
    pub manager: Arc<RwLock<Option<LfmSetupManager>>>,
    pub is_setting_up: Arc<RwLock<bool>>,
}

/// Initialize the LFM engine
pub fn init_lfm_engine(app_data_dir: Option<PathBuf>) -> LfmEngineState {
    match LfmEngine::new(app_data_dir) {
        Ok(engine) => {
            log::info!("LfmEngine initialized");
            LfmEngineState(Arc::new(RwLock::new(Some(engine))))
        }
        Err(e) => {
            log::error!("Failed to initialize LfmEngine: {}", e);
            LfmEngineState(Arc::new(RwLock::new(None)))
        }
    }
}

/// Initialize the LFM setup manager
pub fn init_lfm_setup(app_data_dir: PathBuf) -> LfmSetupState {
    let manager = LfmSetupManager::new(app_data_dir);
    LfmSetupState {
        manager: Arc::new(RwLock::new(Some(manager))),
        is_setting_up: Arc::new(RwLock::new(false)),
    }
}

/// Check if LFM environment is set up (venv exists with dependencies)
#[tauri::command]
pub async fn lfm_is_setup_complete(
    state: State<'_, LfmSetupState>,
) -> Result<bool, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("LFM setup manager not initialized")?;
    Ok(manager.is_setup_complete())
}

/// Check if LFM model is downloaded
#[tauri::command]
pub async fn lfm_is_model_downloaded(
    state: State<'_, LfmSetupState>,
) -> Result<bool, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("LFM setup manager not initialized")?;
    Ok(manager.is_model_downloaded())
}

/// Get current setup status
#[tauri::command]
pub async fn lfm_get_setup_status(
    state: State<'_, LfmSetupState>,
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("LFM setup manager not initialized")?;

    let status = manager.get_setup_status();
    let status_str = match status {
        SetupStage::NotStarted => "not_started",
        SetupStage::CheckingPython => "checking_python",
        SetupStage::CreatingVenv => "creating_venv",
        SetupStage::InstallingDependencies => "installing_dependencies",
        SetupStage::DownloadingModel => "downloading_model",
        SetupStage::Verifying => "verifying",
        SetupStage::Complete => "complete",
        SetupStage::Failed(_) => "failed",
    };

    Ok(status_str.to_string())
}

/// Run the full LFM setup process (creates venv, installs deps, downloads model)
/// Emits 'lfm-setup-progress' events with SetupProgress payload
#[tauri::command]
pub async fn lfm_run_setup<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LfmSetupState>,
) -> Result<(), String> {
    // Check if already setting up
    {
        let is_setting_up = state.is_setting_up.read().await;
        if *is_setting_up {
            return Err("Setup already in progress".to_string());
        }
    }

    // Mark as setting up
    {
        let mut is_setting_up = state.is_setting_up.write().await;
        *is_setting_up = true;
    }

    let manager = {
        let guard = state.manager.read().await;
        guard.as_ref().ok_or("LFM setup manager not initialized")?.clone()
    };

    // Create progress channel
    let (tx, mut rx) = mpsc::channel::<SetupProgress>(32);

    // Spawn task to emit progress events
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app_clone.emit("lfm-setup-progress", &progress);
        }
    });

    // Run setup
    let result = manager.run_setup(tx).await;

    // Mark as not setting up
    {
        let mut is_setting_up = state.is_setting_up.write().await;
        *is_setting_up = false;
    }

    result.map_err(|e| e.to_string())
}

/// Check if LFM dependencies (Python + liquid-audio) are available
#[tauri::command]
pub async fn lfm_check_dependencies(
    state: State<'_, LfmEngineState>,
) -> Result<bool, String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    engine.check_dependencies().await
        .map_err(|e| e.to_string())
}

/// Get LFM engine status
#[tauri::command]
pub async fn lfm_get_status(
    state: State<'_, LfmEngineState>,
) -> Result<String, String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    let status = engine.get_status().await;
    let status_str = match status {
        ModelStatus::NotInstalled => "not_installed",
        ModelStatus::Available => "available",
        ModelStatus::Loading => "loading",
        ModelStatus::Loaded => "loaded",
        ModelStatus::Error(_) => "error",
    };

    Ok(status_str.to_string())
}

/// Load LFM model
#[tauri::command]
pub async fn lfm_load_model(
    state: State<'_, LfmEngineState>,
    model_id: Option<String>,
) -> Result<(), String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    engine.load_model(model_id.as_deref()).await
        .map_err(|e| e.to_string())
}

/// Unload LFM model
#[tauri::command]
pub async fn lfm_unload_model(
    state: State<'_, LfmEngineState>,
) -> Result<(), String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    engine.unload_model().await
        .map_err(|e| e.to_string())
}

/// Transcribe audio file using LFM
#[tauri::command]
pub async fn lfm_transcribe_file(
    state: State<'_, LfmEngineState>,
    audio_path: String,
) -> Result<String, String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    engine.transcribe_file(&audio_path).await
        .map_err(|e| e.to_string())
}

/// Transcribe audio samples using LFM
#[tauri::command]
pub async fn lfm_transcribe_samples(
    state: State<'_, LfmEngineState>,
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<String, String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    engine.transcribe_samples(samples, sample_rate).await
        .map_err(|e| e.to_string())
}

/// Check if LFM model is loaded
#[tauri::command]
pub async fn lfm_is_model_loaded(
    state: State<'_, LfmEngineState>,
) -> Result<bool, String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    Ok(engine.is_model_loaded().await)
}

/// Get current LFM model name
#[tauri::command]
pub async fn lfm_get_current_model(
    state: State<'_, LfmEngineState>,
) -> Result<Option<String>, String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    Ok(engine.get_current_model().await)
}

/// Shutdown LFM sidecar
#[tauri::command]
pub async fn lfm_shutdown(
    state: State<'_, LfmEngineState>,
) -> Result<(), String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;

    engine.shutdown().await
        .map_err(|e| e.to_string())
}

/// Get installation instructions for liquid-audio
#[tauri::command]
pub async fn lfm_get_install_instructions() -> Result<String, String> {
    Ok(r#"LFM2.5-Audio will be set up automatically when you click "Setup LFM".

This includes:
1. Creating a Python environment (isolated from your system)
2. Installing the liquid-audio package and dependencies
3. Downloading the LFM2.5-Audio model (~3GB)

Requirements:
- Python 3.9+ must be installed on your system
- ~5GB disk space (dependencies + model)
- Internet connection for download

The setup process takes about 5-10 minutes depending on your connection speed."#.to_string())
}

// Make LfmSetupManager cloneable for use in async context
impl Clone for LfmSetupManager {
    fn clone(&self) -> Self {
        // Access fields directly since they're not private within the crate
        let backend_dir = self.backend_dir.clone();
        Self::new(backend_dir) // The arg is ignored, it uses get_backend_dir()
    }
}

// Add the field accessors we need
impl LfmSetupManager {
    pub fn backend_dir(&self) -> &PathBuf {
        &self.backend_dir
    }
}
