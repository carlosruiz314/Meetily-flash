use anyhow::Result;
use screenpipe_audio::{
    default_input_device, default_output_device, AudioStream, record_and_transcribe,
    create_whisper_channel, AudioTranscriptionEngine, VadEngineEnum,
    vad_engine::VadSensitivity, AudioInput, pyannote::{
        embedding::EmbeddingExtractor,
        identify::EmbeddingManager,
    },
};
use screenpipe_core::Language;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::Duration;
use std::path::PathBuf;
use tokio;
use tracing::{info, warn, Level};
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Local;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with more explicit configuration
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting audio recorder with live transcription...");

    // Initialize speaker identification
    let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let embedding_model_path = project_dir
        .join("models")
        .join("pyannote")
        .join("wespeaker_en_voxceleb_CAM++.onnx");
    
    let embedding_extractor = Arc::new(Mutex::new(
        EmbeddingExtractor::new(
            embedding_model_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid embedding model path"))?
        )?
    ));
    
    let embedding_manager = Arc::new(Mutex::new(EmbeddingManager::new(10))); // Max 10 speakers

    // Create transcript file with timestamp
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let transcript_path = format!("transcript_{}.txt", timestamp);
    let transcript_file = Arc::new(Mutex::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&transcript_path)?
    ));
    
    info!("Writing transcript to: {}", transcript_path);

    // Get both input (microphone) and output (system audio) devices
    let mic_device = Arc::new(default_input_device()?);
    let system_device = Arc::new(default_output_device()?);
    info!("Using default input device (microphone)");
    info!("Using default output device (system audio)");
    
    // Create a flag to control recording
    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = is_running.clone();
    
    // Create audio streams for both devices
    info!("Initializing audio streams...");
    let mic_stream = Arc::new(AudioStream::from_device(mic_device.clone(), is_running.clone()).await?);
    let system_stream = Arc::new(AudioStream::from_device(system_device.clone(), is_running.clone()).await?);
    
    // Get audio format details from the device configs
    let mic_config = &mic_stream.device_config;
    let system_config = &system_stream.device_config;
    info!("Microphone configuration: {} Hz, {} channels", 
        mic_config.sample_rate().0, 
        mic_config.channels());
    info!("System audio configuration: {} Hz, {} channels", 
        system_config.sample_rate().0, 
        system_config.channels());
    
    // Setup transcription with smaller chunk duration for more frequent updates
    info!("Setting up transcription pipeline...");
    let chunk_duration = Duration::from_millis(3000); // Process in 2000ms chunks for faster response
    let (whisper_sender, whisper_receiver, _) = create_whisper_channel(
        Arc::new(AudioTranscriptionEngine::WhisperTiny), // Use tiny model for faster processing
        VadEngineEnum::Silero,
        None, // No Deepgram API key
        &PathBuf::from("."), // Current directory
        VadSensitivity::High, // More sensitive to catch shorter utterances
        vec![Language::English],
    ).await?;
    info!("Transcription pipeline ready");
    
    // Spawn transcription receiver task with file writing
    let recv_task = tokio::spawn({
        let is_running = is_running.clone();
        let transcript_file = transcript_file.clone();
        let embedding_manager = embedding_manager.clone();
        let embedding_extractor = embedding_extractor.clone();
        async move {
            while is_running.load(Ordering::SeqCst) {
                if let Ok(result) = whisper_receiver.try_recv() {
                    if let Some(text) = result.transcription {
                        if !text.trim().is_empty() {
                            // Get current timestamp
                            let timestamp = Local::now().format("%H:%M:%S");
                            
                            // Try to identify speaker
                            let speaker = {
                                // Get audio data from the input
                                let audio_data = &result.input.data;
                                
                                // Extract embedding and get speaker ID
                                match embedding_extractor.lock().unwrap().compute(&audio_data) {
                                    Ok(embedding_iter) => {
                                        // Convert iterator to Vec
                                        let embedding: Vec<f32> = embedding_iter.collect();
                                        match embedding_manager.lock().unwrap().search_speaker(embedding, 0.75) {
                                            Some(speaker_id) => format!("Speaker {}", speaker_id),
                                            None => "Unknown Speaker".to_string(),
                                        }
                                    },
                                    Err(e) => {
                                        warn!("Failed to extract speaker embedding: {}", e);
                                        "Unknown Speaker".to_string()
                                    }
                                }
                            };
                            
                            // Format transcript line with source info
                            let source = if result.input.device.to_string().contains("input") {
                                "Microphone"
                            } else {
                                "System Audio"
                            };
                            
                            let transcript_line = format!(
                                "[{}] {} ({}): {}\n",
                                timestamp,
                                speaker,
                                source,
                                text
                            );
                            
                            // Write to console
                            info!("{}", transcript_line.trim());
                            
                            // Write to file
                            if let Ok(mut file) = transcript_file.lock() {
                                if let Err(e) = file.write_all(transcript_line.as_bytes()) {
                                    warn!("Failed to write to transcript file: {}", e);
                                }
                            }
                        }
                    }
                    if let Some(error) = result.error {
                        warn!("Transcription error: {}", error);
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    });
    
    // Handle Ctrl+C
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        info!("Received Ctrl+C signal");
        is_running_clone.store(false, Ordering::SeqCst);
    });
    
    info!("Recording started... Press Ctrl+C to stop");
    
    // Start recording and transcribing for both streams
    let mic_handle = tokio::spawn(record_and_transcribe(
        mic_stream.clone(),
        chunk_duration,
        whisper_sender.clone(),
        is_running.clone(),
    ));
    
    let system_handle = tokio::spawn(record_and_transcribe(
        system_stream.clone(),
        chunk_duration,
        whisper_sender,
        is_running.clone(),
    ));
    
    // Wait for both recording tasks to complete
    let mic_result = mic_handle.await??;
    let system_result = system_handle.await??;
    
    info!("Recording stopped");
    Ok(())
}
