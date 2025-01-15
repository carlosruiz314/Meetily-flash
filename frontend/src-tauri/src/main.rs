// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log;

#[derive(serde::Serialize)]
struct RecordingResult {
    path: String,
    transcript: String,
}

#[tauri::command]
async fn stop_recording(save_path: String) -> Result<RecordingResult, String> {
    // Get transcript from whisper server
    let transcript = match invoke_whisper_server(&save_path).await {
        Ok(text) => text,
        Err(e) => {
            log::error!("Failed to get transcript: {}", e);
            String::new()
        }
    };
    
    Ok(RecordingResult {
        path: save_path,
        transcript,
    })
}

async fn invoke_whisper_server(audio_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    // TODO: Implement actual whisper server call
    // For now, returning placeholder
    Ok("Transcript placeholder".to_string())
}

fn main() {
    log::info!("Starting application...");
    app_lib::run();
}
