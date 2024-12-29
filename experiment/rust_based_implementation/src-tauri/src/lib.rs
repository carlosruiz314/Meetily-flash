use screenpipe_audio::{
    default_input_device, AudioStream, encode_single_audio,
    create_whisper_channel, AudioTranscriptionEngine, VadEngineEnum,
    vad_engine::VadSensitivity, record_and_transcribe,
};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::Duration;
use std::path::PathBuf;
use std::fs;
use tokio;
use serde::{Deserialize, Serialize};
use log::{info, error};
use tauri::{Window, Emitter, Manager};
use time::{OffsetDateTime, format_description};

static RECORDING_FLAG: AtomicBool = AtomicBool::new(false);
static mut AUDIO_BUFFER: Option<Mutex<Vec<f32>>> = None;
static mut AUDIO_STREAM: Option<Arc<AudioStream>> = None;
static mut IS_RUNNING: Option<Arc<AtomicBool>> = None;

#[derive(Debug, Deserialize)]
struct RecordingArgs {
    save_path: String,
}

#[derive(Debug, Serialize)]
struct TranscriptUpdate {
    text: String,
    timestamp: String,
}

#[tauri::command]
async fn start_recording(window: Window) -> Result<(), String> {
    info!("Starting recording...");
    if RECORDING_FLAG.load(Ordering::SeqCst) {
        error!("Recording is already in progress");
        return Err("Recording is already in progress".to_string());
    }

    // Initialize audio recording
    let device = Arc::new(default_input_device().map_err(|e| {
        error!("Failed to get default input device: {}", e);
        e.to_string()
    })?);
    
    let is_running = Arc::new(AtomicBool::new(true));
    let audio_stream = Arc::new(AudioStream::from_device(device.clone(), is_running.clone())
        .await
        .map_err(|e| {
            error!("Failed to create audio stream: {}", e);
            e.to_string()
        })?);
    
    info!("Audio stream created successfully");
    
    // Setup transcription
    info!("Setting up transcription pipeline...");
    let chunk_duration = Duration::from_millis(3000);
    let (whisper_sender, whisper_receiver, _) = create_whisper_channel(
        Arc::new(AudioTranscriptionEngine::WhisperTiny),
        VadEngineEnum::Silero,
        None,
        &PathBuf::from("."),
        VadSensitivity::High,
        vec![],  // Use default language
    ).await.map_err(|e| {
        error!("Failed to create whisper channel: {}", e);
        e.to_string()
    })?;
    
    info!("Transcription pipeline ready");
    
    unsafe {
        AUDIO_BUFFER = Some(Mutex::new(Vec::new()));
        AUDIO_STREAM = Some(audio_stream.clone());
        IS_RUNNING = Some(is_running.clone());
    }
    
    // Start transcription receiver task
    let window_clone = window.clone();
    let is_running_recv = is_running.clone();
    tokio::spawn(async move {
        while is_running_recv.load(Ordering::SeqCst) {
            if let Ok(result) = whisper_receiver.try_recv() {
                if let Some(text) = result.transcription {
                    if !text.trim().is_empty() {
                        let format = format_description::parse("[hour]:[minute]:[second]")
                            .expect("Invalid time format");
                        let timestamp = OffsetDateTime::now_utc()
                            .format(&format)
                            .unwrap_or_default();
                        
                        let update = TranscriptUpdate {
                            text: text.trim().to_string(),
                            timestamp,
                        };
                        
                        // Emit event to frontend
                        if let Err(e) = window_clone.emit("transcript-update", &update) {
                            error!("Failed to send transcript update: {}", e);
                        }
                    }
                }
                if let Some(error) = result.error {
                    error!("Transcription error: {}", error);
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    
    // Start recording and transcribing
    let is_running_record = is_running.clone();
    let _record_handle = tokio::spawn(record_and_transcribe(
        audio_stream.clone(),
        chunk_duration,
        whisper_sender,
        is_running_record,
    ));
    
    // Start audio buffer collection (for saving to file later)
    let mut file_receiver = audio_stream.subscribe().await;
    RECORDING_FLAG.store(true, Ordering::SeqCst);
    
    tokio::spawn(async move {
        info!("Started recording loop");
        while RECORDING_FLAG.load(Ordering::SeqCst) {
            if let Ok(chunk) = file_receiver.try_recv() {
                unsafe {
                    if let Some(buffer) = &AUDIO_BUFFER {
                        if let Ok(mut guard) = buffer.lock() {
                            guard.extend_from_slice(&chunk);
                            if guard.len() % 44100 == 0 {  // Log every second of audio
                                info!("Recorded {} samples", guard.len());
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        info!("Recording loop ended");
    });
    
    Ok(())
}

#[tauri::command]
async fn stop_recording(save_path: String) -> Result<(), String> {
    info!("Stopping recording...");
    if !RECORDING_FLAG.load(Ordering::SeqCst) {
        error!("No recording in progress");
        return Err("No recording in progress".to_string());
    }

    // Stop the recording loop first
    RECORDING_FLAG.store(false, Ordering::SeqCst);
    
    // Set is_running to false to stop the audio stream and transcription
    unsafe {
        if let Some(is_running) = &IS_RUNNING {
            info!("Stopping audio stream...");
            is_running.store(false, Ordering::SeqCst);
        }
    }
    
    // Small delay to ensure the recording loop has stopped
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let result = unsafe {
        let mut audio_bytes = Vec::new();
        let mut sample_rate = 44100;
        let mut channels = 2;
        
        // Get the audio stream config if available
        if let Some(stream) = &AUDIO_STREAM {
            let config = &stream.device_config;
            sample_rate = config.sample_rate().0;
            channels = config.channels();
            info!("Using device config: {} Hz, {} channels", sample_rate, channels);
        }
        
        // Get the recorded audio data
        if let Some(buffer) = &AUDIO_BUFFER {
            if let Ok(guard) = buffer.lock() {
                info!("Converting {} samples to bytes", guard.len());
                
                // Convert f32 samples to bytes in little-endian format
                audio_bytes = Vec::with_capacity(guard.len() * 4);
                for &sample in guard.iter() {
                    // Ensure samples are in -1.0 to 1.0 range
                    let clamped = sample.max(-1.0).min(1.0);
                    audio_bytes.extend_from_slice(&clamped.to_le_bytes());
                }
                
                info!("Converted {} samples to {} bytes", guard.len(), audio_bytes.len());
            }
        }
        
        // Clean up resources
        AUDIO_BUFFER = None;
        AUDIO_STREAM = None;
        IS_RUNNING = None;
        
        if audio_bytes.is_empty() {
            Err("No audio data recorded".to_string())
        } else {
            // Create output directory if it doesn't exist
            let output_path = PathBuf::from(&save_path);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    error!("Failed to create output directory: {}", e);
                    format!("Failed to create output directory: {}", e)
                })?;
            }
            
            // Change extension to .wav
            let wav_path = output_path.with_extension("wav");
            info!("Encoding audio to {}", wav_path.display());
            
            encode_single_audio(
                &audio_bytes,
                sample_rate,
                channels,
                &wav_path,
            )
            .map_err(|e| {
                error!("Failed to encode audio: {}", e);
                format!("Failed to encode audio: {}", e)
            })
        }
    };
    
    // Always reset the recording flag, even if there was an error
    RECORDING_FLAG.store(false, Ordering::SeqCst);
    
    result
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_recording, stop_recording])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
