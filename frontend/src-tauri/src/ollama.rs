use std::process::Command;
use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub id: String,
    pub size: String,
    pub modified: String,
}

#[command]
pub fn get_ollama_models() -> Result<Vec<OllamaModel>, String> {
    let output = Command::new("ollama")
        .arg("list")
        .output()
        .map_err(|e| format!("Failed to execute ollama command: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut models = Vec::new();
    
    // Skip the header line
    for line in output_str.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            models.push(OllamaModel {
                name: parts[0].to_string(),
                id: parts[1].to_string(),
                size: format!("{} {}", parts[2], parts[3]),
                modified: parts[4..].join(" "),
            });
        }
    }

    Ok(models)
}
