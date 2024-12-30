use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use screenpipe_audio::{
    default_input_device, default_output_device, AudioStream,
    encode_single_audio,
};
use tauri::{Runtime, AppHandle, Manager, Emitter};
use log::{info as log_info, error as log_error, debug as log_debug};
use reqwest::multipart::{Form, Part};

static RECORDING_FLAG: AtomicBool = AtomicBool::new(false);
static mut MIC_BUFFER: Option<Mutex<Vec<f32>>> = None;
static mut SYSTEM_BUFFER: Option<Mutex<Vec<f32>>> = None;
static mut MIC_STREAM: Option<Arc<AudioStream>> = None;
static mut SYSTEM_STREAM: Option<Arc<AudioStream>> = None;
static mut IS_RUNNING: Option<Arc<AtomicBool>> = None;

// Audio configuration constants
const CHUNK_DURATION_MS: u32 = 3000; // 3 seconds per chunk
const WHISPER_SAMPLE_RATE: u32 = 16000; // Whisper's required sample rate
const WAV_SAMPLE_RATE: u32 = 44100; // WAV file sample rate
const WAV_CHANNELS: u16 = 2; // Stereo for WAV files
const WHISPER_CHANNELS: u16 = 1; // Mono for Whisper API

#[derive(Debug, Deserialize)]
struct RecordingArgs {
    save_path: String,
}

#[derive(Debug, Serialize, Clone)]
struct TranscriptUpdate {
    text: String,
    timestamp: String,
    source: String,
}

#[derive(Debug, Deserialize)]
struct TranscriptSegment {
    text: String,
    t0: f32,
    t1: f32,
}

#[derive(Debug, Deserialize)]
struct TranscriptResponse {
    segments: Vec<TranscriptSegment>,
    buffer_size_ms: i32,
}

const WHISPER_API_URL: &str = "http://127.0.0.1:8080/stream";

async fn send_audio_chunk(chunk: Vec<f32>, client: &reqwest::Client) -> Result<TranscriptResponse, String> {
    // Convert f32 samples to bytes
    let mut audio_bytes = Vec::with_capacity(chunk.len() * 4);
    for &sample in chunk.iter() {
        let clamped = sample.max(-1.0).min(1.0);
        audio_bytes.extend_from_slice(&clamped.to_le_bytes());
    }
    
    // Create multipart form
    let part = Part::bytes(audio_bytes)
        .file_name("audio")
        .mime_str("application/octet-stream")
        .map_err(|e| format!("Failed to create form part: {}", e))?;
    
    let form = Form::new().part("audio", part);
    
    // Send request
    client.post(WHISPER_API_URL)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?
        .json::<TranscriptResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[tauri::command]
async fn start_recording<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    log_info!("Starting recording...");
    if RECORDING_FLAG.load(Ordering::SeqCst) {
        log_error!("Recording is already in progress");
        return Err("Recording is already in progress".to_string());
    }

    // Initialize audio recording devices
    let mic_device = Arc::new(default_input_device().map_err(|e| {
        log_error!("Failed to get default input device: {}", e);
        e.to_string()
    })?);
    
    let system_device = Arc::new(default_output_device().map_err(|e| {
        log_error!("Failed to get default output device: {}", e);
        e.to_string()
    })?);
    
    log_info!("Using default input device (microphone)");
    log_info!("Using default output device (system audio)");

    let is_running = Arc::new(AtomicBool::new(true));
    
    // Create audio streams for both devices
    let mic_stream = Arc::new(AudioStream::from_device(mic_device.clone(), is_running.clone())
        .await
        .map_err(|e| {
            log_error!("Failed to create microphone stream: {}", e);
            e.to_string()
        })?);
    
    let system_stream = Arc::new(AudioStream::from_device(system_device.clone(), is_running.clone())
        .await
        .map_err(|e| {
            log_error!("Failed to create system audio stream: {}", e);
            e.to_string()
        })?);

    // Log device configurations
    let mic_config = &mic_stream.device_config;
    let system_config = &system_stream.device_config;
    log_info!("Microphone configuration: {} Hz, {} channels", 
        mic_config.sample_rate().0, 
        mic_config.channels());
    log_info!("System audio configuration: {} Hz, {} channels", 
        system_config.sample_rate().0, 
        system_config.channels());
    
    // Initialize shared state
    unsafe {
        MIC_BUFFER = Some(Mutex::new(Vec::new()));
        SYSTEM_BUFFER = Some(Mutex::new(Vec::new()));
        MIC_STREAM = Some(mic_stream.clone());
        SYSTEM_STREAM = Some(system_stream.clone());
        IS_RUNNING = Some(is_running.clone());
    }
    
    // Start audio buffer collection for both streams
    let mut mic_receiver_buffer = mic_stream.subscribe().await;
    let mut system_receiver_buffer = system_stream.subscribe().await;
    RECORDING_FLAG.store(true, Ordering::SeqCst);
    
    // Create HTTP client for transcription
    let client = reqwest::Client::new();
    
    // Start transcription task
    let app_handle = app.clone();
    let is_running_clone = is_running.clone();
    
    // Create separate receivers for transcription task
    let mut mic_receiver_transcription = mic_stream.subscribe().await;
    let mut system_receiver_transcription = system_stream.subscribe().await;
    
    // Store is_running in the static variable
    unsafe {
        IS_RUNNING = Some(is_running.clone());
    }
    
    // Create debug directory for chunks
    let debug_dir = PathBuf::from("debug_chunks");
    fs::create_dir_all(&debug_dir).map_err(|e| {
        log_error!("Failed to create debug directory: {}", e);
        e.to_string()
    })?;
    
    let chunk_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let chunk_counter_clone = chunk_counter.clone();
    
    tokio::spawn(async move {
        let chunk_samples = (WHISPER_SAMPLE_RATE as f32 * (CHUNK_DURATION_MS as f32 / 1000.0)) as usize;
        let mut current_chunk: Vec<f32> = Vec::with_capacity(chunk_samples);
        
        // Get device configs
        let mic_config = mic_stream.device_config.clone();
        let system_config = system_stream.device_config.clone();
        
        log_info!("Mic config: {} Hz, {} channels", mic_config.sample_rate().0, mic_config.channels());
        log_info!("System config: {} Hz, {} channels", system_config.sample_rate().0, system_config.channels());
        
        while is_running_clone.load(Ordering::SeqCst) {
            // Collect audio samples
            let mut new_samples = Vec::new();
            let mut mic_samples = Vec::new();
            let mut system_samples = Vec::new();
            
            // Get microphone samples
            if let Ok(chunk) = mic_receiver_transcription.try_recv() {
                log_debug!("Received {} mic samples", chunk.len());
                mic_samples.extend(chunk);
            }
            
            // Get system audio samples
            if let Ok(chunk) = system_receiver_transcription.try_recv() {
                log_debug!("Received {} system samples", chunk.len());
                system_samples.extend(chunk);
            }
            
            // Mix samples with debug info
            let max_len = mic_samples.len().max(system_samples.len());
            for i in 0..max_len {
                let mic_sample = if i < mic_samples.len() { mic_samples[i] } else { 0.0 };
                let system_sample = if i < system_samples.len() { system_samples[i] } else { 0.0 };
                new_samples.push((mic_sample + system_sample) * 0.5);
            }
            
            log_debug!("Mixed {} samples", new_samples.len());
            
            // Add samples to current chunk
            for sample in new_samples {
                current_chunk.push(sample);
                
                // If we have enough samples, send for transcription
                if current_chunk.len() >= chunk_samples {
                    let chunk_to_send = current_chunk.clone();
                    current_chunk.clear();
                    
                    // Save original chunk for debugging (stereo WAV)
                    let chunk_num = chunk_counter_clone.fetch_add(1, Ordering::SeqCst);
                    let chunk_path = debug_dir.join(format!("chunk_{}.wav", chunk_num));
                    
                    // Convert chunk to bytes
                    let chunk_bytes: Vec<u8> = chunk_to_send.iter()
                        .flat_map(|&sample| {
                            let clamped = sample.max(-1.0).min(1.0);
                            clamped.to_le_bytes().to_vec()
                        })
                        .collect();
                    
                    // Save original chunk as WAV
                    if let Err(e) = encode_single_audio(
                        &chunk_bytes,
                        WAV_SAMPLE_RATE,
                        WAV_CHANNELS,
                        &chunk_path,
                    ) {
                        log_error!("Failed to save debug chunk {}: {}", chunk_num, e);
                    } else {
                        log_debug!("Saved chunk {} with {} samples", chunk_num, chunk_to_send.len());
                    }
                    
                    // Process chunk for Whisper API
                    let whisper_samples = if mic_config.sample_rate().0 != WHISPER_SAMPLE_RATE {
                        resample_audio(
                            &chunk_to_send,
                            mic_config.sample_rate().0,
                            WHISPER_SAMPLE_RATE,
                        )
                    } else {
                        chunk_to_send
                    };
                    
                    // Send chunk for transcription
                    match send_audio_chunk(whisper_samples, &client).await {
                        Ok(response) => {
                            for segment in response.segments {
                                let timestamp = format!("{:.1}s", segment.t0);
                                let update = TranscriptUpdate {
                                    text: segment.text,
                                    timestamp,
                                    source: "Mixed Audio".to_string(),
                                };
                                
                                if let Err(e) = app_handle.emit("transcript-update", update) {
                                    log_error!("Failed to send transcript update: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            log_error!("Transcription error: {}", e);
                        }
                    }
                }
            }
            
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        log_info!("Transcription task ended");
    });
    
    // Start audio buffer collection task
    tokio::spawn(async move {
        log_info!("Started recording loop");
        while RECORDING_FLAG.load(Ordering::SeqCst) {
            // Collect microphone audio
            if let Ok(chunk) = mic_receiver_buffer.try_recv() {
                unsafe {
                    if let Some(buffer) = &MIC_BUFFER {
                        if let Ok(mut guard) = buffer.lock() {
                            guard.extend_from_slice(&chunk);
                        }
                    }
                }
            }

            // Collect system audio
            if let Ok(chunk) = system_receiver_buffer.try_recv() {
                unsafe {
                    if let Some(buffer) = &SYSTEM_BUFFER {
                        if let Ok(mut guard) = buffer.lock() {
                            guard.extend_from_slice(&chunk);
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        log_info!("Recording loop ended");
    });
    
    Ok(())
}

#[tauri::command]
async fn stop_recording(args: RecordingArgs) -> Result<(), String> {
    log_info!("Stopping recording...");
    
    // Stop all tasks
    RECORDING_FLAG.store(false, Ordering::SeqCst);
    unsafe {
        if let Some(is_running) = &IS_RUNNING {
            is_running.store(false, Ordering::SeqCst);
        }
    }
    
    // Small delay to ensure tasks have stopped
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let result = unsafe {
        let mut mixed_samples: Vec<f32> = Vec::new();
        let mut sample_rate = 44100;
        let mut channels = 2;
        
        // Get device config from stream if available
        if let Some(stream) = &MIC_STREAM {
            let config = &stream.device_config;
            sample_rate = config.sample_rate().0;
            channels = config.channels();
            log_info!("Using device config: {} Hz, {} channels", sample_rate, channels);
        }
        
        // Get microphone samples
        let mic_samples = if let Some(buffer) = &MIC_BUFFER {
            if let Ok(guard) = buffer.lock() {
                guard.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        
        // Get system audio samples
        let system_samples = if let Some(buffer) = &SYSTEM_BUFFER {
            if let Ok(guard) = buffer.lock() {
                guard.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        
        // Mix samples with equal weight (0.5 for each source)
        let max_len = mic_samples.len().max(system_samples.len());
        mixed_samples.reserve(max_len);
        
        for i in 0..max_len {
            let mic_sample = if i < mic_samples.len() { mic_samples[i] } else { 0.0 };
            let system_sample = if i < system_samples.len() { system_samples[i] } else { 0.0 };
            mixed_samples.push((mic_sample + system_sample) * 0.5);
        }
        
        log_info!("Converting {} samples to bytes", mixed_samples.len());
        
        // Convert mixed samples to bytes
        let audio_bytes: Vec<u8> = mixed_samples.iter().flat_map(|&sample| {
            let clamped = sample.max(-1.0).min(1.0);
            clamped.to_le_bytes().to_vec()
        }).collect();
        
        log_info!("Converted {} samples to {} bytes", mixed_samples.len(), audio_bytes.len());
        
        // Create output directory if needed
        let save_path = PathBuf::from(&args.save_path);
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                log_error!("Failed to create output directory: {}", e);
                format!("Failed to create output directory: {}", e)
            })?;
        }
        
        // Save as WAV file
        encode_single_audio(
            &audio_bytes,
            sample_rate,
            channels,
            &save_path,
        ).map_err(|e| {
            log_error!("Failed to encode audio: {}", e);
            e.to_string()
        })?;
        
        // Clean up resources
        MIC_BUFFER = None;
        SYSTEM_BUFFER = None;
        MIC_STREAM = None;
        SYSTEM_STREAM = None;
        IS_RUNNING = None;
        
        Ok(())
    };
    
    result
}

#[tauri::command]
fn is_recording() -> bool {
    RECORDING_FLAG.load(Ordering::SeqCst)
}

// Helper function to resample audio
fn resample_audio(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    
    let ratio = to_rate as f32 / from_rate as f32;
    let new_len = (samples.len() as f32 * ratio) as usize;
    let mut resampled = Vec::with_capacity(new_len);
    
    for i in 0..new_len {
        let src_idx = (i as f32 / ratio) as usize;
        if src_idx < samples.len() {
            resampled.push(samples[src_idx]);
        }
    }
    
    resampled
}

// Helper function to convert stereo to mono
fn stereo_to_mono(samples: &[f32]) -> Vec<f32> {
    let mut mono = Vec::with_capacity(samples.len() / 2);
    for chunk in samples.chunks(2) {
        if chunk.len() == 2 {
            mono.push((chunk[0] + chunk[1]) * 0.5);
        }
    }
    mono
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            log_info!("Application setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![start_recording, stop_recording])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
