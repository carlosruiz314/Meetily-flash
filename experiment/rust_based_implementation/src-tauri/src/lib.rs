use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use screenpipe_audio::{
    default_input_device, default_output_device, AudioStream,
    encode_single_audio,
};
use tauri::{Runtime, AppHandle, Emitter};
use log::{info as log_info, error as log_error, debug as log_debug};
use reqwest::multipart::{Form, Part};

static RECORDING_FLAG: AtomicBool = AtomicBool::new(false);
static mut MIC_BUFFER: Option<Mutex<Vec<f32>>> = None;
static mut SYSTEM_BUFFER: Option<Mutex<Vec<f32>>> = None;
static mut MIC_STREAM: Option<Arc<AudioStream>> = None;
static mut SYSTEM_STREAM: Option<Arc<AudioStream>> = None;
static mut IS_RUNNING: Option<Arc<AtomicBool>> = None;

// Audio configuration constants
const CHUNK_DURATION_MS: u32 = 15000; // 30 seconds per chunk for better sentence processing
const WHISPER_SAMPLE_RATE: u32 = 16000; // Whisper's required sample rate
const WAV_SAMPLE_RATE: u32 = 44100; // WAV file sample rate
const WAV_CHANNELS: u16 = 2; // Stereo for WAV files
const WHISPER_CHANNELS: u16 = 1; // Mono for Whisper API
const SENTENCE_TIMEOUT_MS: u64 = 1000; // Emit incomplete sentence after 1 second of silence
const MIN_CHUNK_DURATION_MS: u32 = 1000; // Minimum duration before sending chunk

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

// Helper struct to accumulate transcript segments
#[derive(Debug)]
struct TranscriptAccumulator {
    current_sentence: String,
    sentence_start_time: f32,
    last_update_time: std::time::Instant,
    last_segment_hash: u64,
}

impl TranscriptAccumulator {
    fn new() -> Self {
        Self {
            current_sentence: String::new(),
            sentence_start_time: 0.0,
            last_update_time: std::time::Instant::now(),
            last_segment_hash: 0,
        }
    }

    fn add_segment(&mut self, segment: &TranscriptSegment) -> Option<TranscriptUpdate> {
        log_debug!("Processing new transcript segment: {:?}", segment);
        
        // Update the last update time
        self.last_update_time = std::time::Instant::now();

        // Clean up the text (remove [BLANK_AUDIO], [AUDIO OUT] and trim)
        let clean_text = segment.text
            .replace("[BLANK_AUDIO]", "")
            .replace("[AUDIO OUT]", "")
            .trim()
            .to_string();
            
        if !clean_text.is_empty() {
            log_debug!("Clean transcript text: {}", clean_text);
        }

        // Skip empty segments or very short segments (less than 1 second)
        if clean_text.is_empty() || (segment.t1 - segment.t0) < 1.0 {
            return None;
        }

        // Calculate hash of this segment to detect duplicates
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        segment.text.hash(&mut hasher);
        segment.t0.to_bits().hash(&mut hasher);
        segment.t1.to_bits().hash(&mut hasher);
        let segment_hash = hasher.finish();

        // Skip if this is a duplicate segment
        if segment_hash == self.last_segment_hash {
            return None;
        }
        self.last_segment_hash = segment_hash;

        // If this is the start of a new sentence, store the start time
        if self.current_sentence.is_empty() {
            self.sentence_start_time = segment.t0;
        }

        // Add the new text with proper spacing
        if !self.current_sentence.is_empty() && !self.current_sentence.ends_with(' ') {
            self.current_sentence.push(' ');
        }
        self.current_sentence.push_str(&clean_text);

        // Check if we have a complete sentence
        if clean_text.ends_with('.') || clean_text.ends_with('?') || clean_text.ends_with('!') {
            let sentence = std::mem::take(&mut self.current_sentence);
            let update = TranscriptUpdate {
                text: sentence.trim().to_string(),
                timestamp: format!("{:.1} - {:.1}", self.sentence_start_time, segment.t1),
                source: "Mixed Audio".to_string(),
            };
            log_info!("Generated transcript update: {:?}", update);
            Some(update)
        } else {
            None
        }
    }

    fn check_timeout(&mut self) -> Option<TranscriptUpdate> {
        if !self.current_sentence.is_empty() && 
           self.last_update_time.elapsed() > Duration::from_millis(SENTENCE_TIMEOUT_MS) {
            let sentence = std::mem::take(&mut self.current_sentence);
            let current_time = self.sentence_start_time + (SENTENCE_TIMEOUT_MS as f32 / 1000.0);
            let update = TranscriptUpdate {
                text: sentence.trim().to_string(),
                timestamp: format!("{:.1} - {:.1}", self.sentence_start_time, current_time),
                source: "Mixed Audio".to_string(),
            };
            Some(update)
        } else {
            None
        }
    }
}

async fn send_audio_chunk(chunk: Vec<f32>, client: &reqwest::Client) -> Result<TranscriptResponse, String> {
    log_debug!("Preparing to send audio chunk of size: {}", chunk.len());
    
    // Convert f32 samples to bytes
    let bytes: Vec<u8> = chunk.iter()
        .flat_map(|&sample| {
            let clamped = sample.max(-1.0).min(1.0);
            clamped.to_le_bytes().to_vec()
        })
        .collect();
    
    // Create multipart form
    let part = Part::bytes(bytes).file_name("audio.raw").mime_str("audio/x-raw").unwrap();
    let form = Form::new().part("audio", part);
    
    // Send request
    client.post("http://127.0.0.1:8080/stream")
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<TranscriptResponse>()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_recording<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    log_info!("Attempting to start recording...");
    
    if is_recording() {
        log_error!("Recording already in progress");
        return Err("Recording already in progress".to_string());
    }

    // Initialize recording flag and buffers
    RECORDING_FLAG.store(true, Ordering::SeqCst);
    log_info!("Recording flag set to true");

    unsafe {
        MIC_BUFFER = Some(Mutex::new(Vec::new()));
        SYSTEM_BUFFER = Some(Mutex::new(Vec::new()));
        log_info!("Audio buffers initialized");
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
    
    // Create audio streams
    let is_running = Arc::new(AtomicBool::new(true));
    
    // Create microphone stream
    let mic_stream = AudioStream::from_device(mic_device.clone(), is_running.clone())
        .await
        .map_err(|e| {
            log_error!("Failed to create microphone stream: {}", e);
            e.to_string()
        })?;
    let mic_stream = Arc::new(mic_stream);
    
    // Create system audio stream
    let system_stream = AudioStream::from_device(system_device.clone(), is_running.clone())
        .await
        .map_err(|e| {
            log_error!("Failed to create system audio stream: {}", e);
            e.to_string()
        })?;
    let system_stream = Arc::new(system_stream);

    // Initialize shared state
    unsafe {
        MIC_STREAM = Some(mic_stream.clone());
        SYSTEM_STREAM = Some(system_stream.clone());
        IS_RUNNING = Some(is_running.clone());
    }
    
    // Create HTTP client for transcription
    let client = reqwest::Client::new();
    
    // Start transcription task
    let app_handle = app.clone();
    
    // Create audio receivers
    let mut mic_receiver = mic_stream.subscribe().await;
    let mut system_receiver = system_stream.subscribe().await;
    
    // Create debug directory for chunks in temp
    let temp_dir = std::env::temp_dir();
    log_info!("System temp directory: {:?}", temp_dir);
    let debug_dir = temp_dir.join("meeting_minutes_debug");
    log_info!("Full debug directory path: {:?}", debug_dir);
    
    // Create directory and check if it exists
    fs::create_dir_all(&debug_dir).map_err(|e| {
        log_error!("Failed to create debug directory: {}", e);
        e.to_string()
    })?;
    
    if debug_dir.exists() {
        log_info!("Debug directory successfully created and exists");
    } else {
        log_error!("Failed to create debug directory - path does not exist after creation");
    }
    
    let chunk_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let chunk_counter_clone = chunk_counter.clone();
    
    // Create transcript accumulator
    let mut accumulator = TranscriptAccumulator::new();
    
    tokio::spawn(async move {
        let chunk_samples = (WHISPER_SAMPLE_RATE as f32 * (CHUNK_DURATION_MS as f32 / 1000.0)) as usize;
        let min_samples = (WHISPER_SAMPLE_RATE as f32 * (MIN_CHUNK_DURATION_MS as f32 / 1000.0)) as usize;
        let mut current_chunk: Vec<f32> = Vec::with_capacity(chunk_samples);
        let mut last_chunk_time = std::time::Instant::now();
        
        // Get device configs
        let mic_config = mic_stream.device_config.clone();
        let system_config = system_stream.device_config.clone();
        
        log_info!("Mic config: {} Hz, {} channels", mic_config.sample_rate().0, mic_config.channels());
        log_info!("System config: {} Hz, {} channels", system_config.sample_rate().0, system_config.channels());
        
        while is_running.load(Ordering::SeqCst) {
            // Check for timeout on current sentence
            if let Some(update) = accumulator.check_timeout() {
                if let Err(e) = app_handle.emit("transcript-update", update) {
                    log_error!("Failed to send timeout transcript update: {}", e);
                }
            }

            // Collect audio samples
            let mut new_samples = Vec::new();
            let mut mic_samples = Vec::new();
            let mut system_samples = Vec::new();
            
            // Get microphone samples
            let mut got_mic_samples = false;
            while let Ok(chunk) = mic_receiver.try_recv() {
                got_mic_samples = true;
                log_debug!("Received {} mic samples", chunk.len());
                mic_samples.extend(chunk);
            }
            // If we didn't get any samples, try to resubscribe to clear any backlog
            if !got_mic_samples {
                log_debug!("No mic samples received, resubscribing to clear channel");
                mic_receiver = mic_stream.subscribe().await;
            }
            
            // Get system audio samples
            let mut got_system_samples = false;
            while let Ok(chunk) = system_receiver.try_recv() {
                got_system_samples = true;
                log_debug!("Received {} system samples", chunk.len());
                system_samples.extend(chunk);
            }
            // If we didn't get any samples, try to resubscribe to clear any backlog
            if !got_system_samples {
                log_debug!("No system samples received, resubscribing to clear channel");
                system_receiver = system_stream.subscribe().await;
            }
            
            // Mix samples with debug info
            let max_len = mic_samples.len().max(system_samples.len());
            for i in 0..max_len {
                let mic_sample = if i < mic_samples.len() { mic_samples[i] } else { 0.0 };
                let system_sample = if i < system_samples.len() { system_samples[i] } else { 0.0 };
                // Increase mic sensitivity by giving it more weight in the mix (80% mic, 20% system)
                new_samples.push((mic_sample * 0.7) + (system_sample * 0.3));
            }
            
            log_debug!("Mixed {} samples", new_samples.len());
            
            // Add samples to current chunk
            for sample in new_samples {
                current_chunk.push(sample);
            }
            
            // Check if we should send the chunk based on size or time
            let should_send = current_chunk.len() >= chunk_samples || 
                            (current_chunk.len() >= min_samples && 
                             last_chunk_time.elapsed() >= Duration::from_millis(CHUNK_DURATION_MS as u64));
            
            if should_send {
                log_info!("Should send chunk with {} samples", current_chunk.len());
                let chunk_to_send = current_chunk.clone();
                current_chunk.clear();
                last_chunk_time = std::time::Instant::now();
                
                // Save debug chunks
                let chunk_num = chunk_counter_clone.fetch_add(1, Ordering::SeqCst);
                log_info!("Processing chunk {}", chunk_num);
                
                // Save mic chunk
                if !mic_samples.is_empty() {
                    let mic_chunk_path = debug_dir.join(format!("chunk_{}_mic.wav", chunk_num));
                    log_info!("Saving mic chunk to {:?}", mic_chunk_path);
                    let mic_bytes: Vec<u8> = mic_samples.iter()
                        .flat_map(|&sample| {
                            let clamped = sample.max(-1.0).min(1.0);
                            clamped.to_le_bytes().to_vec()
                        })
                        .collect();
                    if let Err(e) = encode_single_audio(
                        &mic_bytes,
                        WAV_SAMPLE_RATE,
                        1, // Mono for mic
                        &mic_chunk_path,
                    ) {
                        log_error!("Failed to save mic chunk {}: {}", chunk_num, e);
                    } else {
                        log_info!("Successfully saved mic chunk {} with {} samples", chunk_num, mic_samples.len());
                    }
                } else {
                    log_info!("No mic samples to save for chunk {}", chunk_num);
                }

                // Save system chunk
                if !system_samples.is_empty() {
                    let system_chunk_path = debug_dir.join(format!("chunk_{}_system.wav", chunk_num));
                    log_info!("Saving system chunk to {:?}", system_chunk_path);
                    let system_bytes: Vec<u8> = system_samples.iter()
                        .flat_map(|&sample| {
                            let clamped = sample.max(-1.0).min(1.0);
                            clamped.to_le_bytes().to_vec()
                        })
                        .collect();
                    if let Err(e) = encode_single_audio(
                        &system_bytes,
                        WAV_SAMPLE_RATE,
                        2, // Stereo for system
                        &system_chunk_path,
                    ) {
                        log_error!("Failed to save system chunk {}: {}", chunk_num, e);
                    } else {
                        log_info!("Successfully saved system chunk {} with {} samples", chunk_num, system_samples.len());
                    }
                } else {
                    log_info!("No system samples to save for chunk {}", chunk_num);
                }
                
                // Save mixed chunk
                let mixed_chunk_path = debug_dir.join(format!("chunk_{}_mixed.wav", chunk_num));
                log_info!("Saving mixed chunk to {:?}", mixed_chunk_path);
                let chunk_bytes: Vec<u8> = chunk_to_send.iter()
                    .flat_map(|&sample| {
                        let clamped = sample.max(-1.0).min(1.0);
                        clamped.to_le_bytes().to_vec()
                    })
                    .collect();
                
                if let Err(e) = encode_single_audio(
                    &chunk_bytes,
                    WAV_SAMPLE_RATE,
                    WAV_CHANNELS,
                    &mixed_chunk_path,
                ) {
                    log_error!("Failed to save mixed chunk {}: {}", chunk_num, e);
                } else {
                    log_info!("Successfully saved mixed chunk {} with {} samples", chunk_num, chunk_to_send.len());
                }
                
                // Keep only last 10 chunks
                if chunk_num > 10 {
                    if let Ok(entries) = fs::read_dir(&debug_dir) {
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.starts_with("chunk_") && 
                                   name.ends_with(".wav") && 
                                   !name.contains(&format!("chunk_{}", chunk_num)) {
                                    let _ = fs::remove_file(entry.path());
                                }
                            }
                        }
                    }
                }
                
                // Process chunk for Whisper API
                let whisper_samples = if mic_config.sample_rate().0 != WHISPER_SAMPLE_RATE {
                    log_debug!("Resampling audio from {} to {}", mic_config.sample_rate().0, WHISPER_SAMPLE_RATE);
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
                        log_info!("Received {} transcript segments", response.segments.len());
                        for segment in response.segments {
                            log_info!("Processing segment: {} ({:.1}s - {:.1}s)", 
                                     segment.text.trim(), segment.t0, segment.t1);
                            // Add segment to accumulator and check for complete sentence
                            if let Some(update) = accumulator.add_segment(&segment) {
                                // Emit the update
                                if let Err(e) = app_handle.emit("transcript-update", update) {
                                    log_error!("Failed to emit transcript update: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log_error!("Transcription error: {}", e);
                    }
                }
            }
            
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // Emit any remaining transcript when recording stops
        if let Some(update) = accumulator.check_timeout() {
            if let Err(e) = app_handle.emit("transcript-update", update) {
                log_error!("Failed to send final transcript update: {}", e);
            }
        }
        
        log_info!("Transcription task ended");
    });
    
    Ok(())
}

#[tauri::command]
async fn stop_recording(args: RecordingArgs) -> Result<(), String> {
    log_info!("Attempting to stop recording...");
    
    if !is_recording() {
        log_error!("No recording in progress");
        return Err("No recording in progress".to_string());
    }

    // First set the recording flag to false to prevent new data from being processed
    RECORDING_FLAG.store(false, Ordering::SeqCst);
    log_info!("Recording flag set to false");
    
    unsafe {
        // Stop the running flag for audio streams
        if let Some(is_running) = &IS_RUNNING {
            is_running.store(false, Ordering::SeqCst);
        }
    }
    
    // Give time for the background task to complete
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    unsafe {
        // Now try to stop the streams
        if let Some(mic_stream) = MIC_STREAM.take() {
            // Drop any remaining subscribers first
            if let Ok(stream) = Arc::try_unwrap(mic_stream) {
                if let Err(e) = stream.stop().await {
                    log_error!("Error stopping mic stream: {}", e);
                }
            } else {
                log_error!("Could not get exclusive ownership of mic stream");
            }
        }

        if let Some(system_stream) = SYSTEM_STREAM.take() {
            if let Ok(stream) = Arc::try_unwrap(system_stream) {
                if let Err(e) = stream.stop().await {
                    log_error!("Error stopping system stream: {}", e);
                }
            } else {
                log_error!("Could not get exclusive ownership of system stream");
            }
        }
    }
    
    // Get final buffers
    let mic_data = unsafe {
        if let Some(buffer) = &MIC_BUFFER {
            if let Ok(guard) = buffer.lock() {
                guard.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    };
    
    let system_data = unsafe {
        if let Some(buffer) = &SYSTEM_BUFFER {
            if let Ok(guard) = buffer.lock() {
                guard.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    };
    
    // Mix the audio
    let max_len = mic_data.len().max(system_data.len());
    let mut mixed_data = Vec::with_capacity(max_len);
    
    for i in 0..max_len {
        let mic_sample = if i < mic_data.len() { mic_data[i] } else { 0.0 };
        let system_sample = if i < system_data.len() { system_data[i] } else { 0.0 };
        mixed_data.push((mic_sample + system_sample) * 0.5);
    }
    
    // Convert to bytes
    let bytes: Vec<u8> = mixed_data.iter()
        .flat_map(|&sample| {
            let clamped = sample.max(-1.0).min(1.0);
            clamped.to_le_bytes().to_vec()
        })
        .collect();
    
    // Save the recording
    encode_single_audio(
        &bytes,
        WAV_SAMPLE_RATE,
        WAV_CHANNELS,
        &PathBuf::from(&args.save_path),
    ).map_err(|e| {
        log_error!("Failed to save recording: {}", e);
        e.to_string()
    })?;
    
    // Clean up
    unsafe {
        MIC_BUFFER = None;
        SYSTEM_BUFFER = None;
        MIC_STREAM = None;
        SYSTEM_STREAM = None;
        IS_RUNNING = None;
    }
    
    Ok(())
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
    env_logger::init();
    log::set_max_level(log::LevelFilter::Debug);
    
    tauri::Builder::default()
        .setup(|_app| {
            log_info!("Application setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            is_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
