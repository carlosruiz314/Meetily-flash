# Meeting Minutes - AI-Powered Meeting Assistant

## Release 0.0.1

A new release is available!

Please check out the release [here](https://github.com/Zackriya-Solutions/meeting-minutes/releases/tag/v0.0.1).

## Overview

An AI-powered meeting assistant that captures live meeting audio, transcribes it in real-time, and generates summaries while ensuring user privacy. Perfect for teams who want to focus on discussions while automatically capturing and organizing meeting content.

### Why?

While there are many meeting transcription tools available, this solution stands out by offering:
- **Privacy First**: All processing happens locally on your device
- **Cost Effective**: Uses open-source AI models instead of expensive APIs
- **Flexible**: Works offline, supports multiple meeting platforms
- **Customizable**: Self-host and modify for your specific needs
- **Intelligent**: Built-in knowledge graph for semantic search across meetings

> **Note**: We have an experimental Rust-based implementation that explores better performance and native integration. It currently implements:
> - ✅ Real-time audio capture from both microphone and system audio
> - ✅ Live transcription using locally-running Whisper
> - ✅ Speaker diarization
> - ✅ Rich text editor for notes
> 
> See [Rust Implementation](experiment/rust_based_implementation) for details.


## Features

✅ Modern, responsive UI with real-time updates
✅ Real-time audio capture (microphone + system audio)
✅ Live transcription using Whisper.cpp
✅ Speaker diarization
✅ Local processing for privacy
✅ Packaged the app for Mac Os
🚧 Export to Markdown/PDF

## LLM Integration

The backend supports multiple LLM providers through a unified interface. Current implementations include:

### Supported Providers
- **Anthropic** (Claude models)
- **Groq** (Llama3.2 90 B, Deepseek)
- **Ollama** (Local models)

### Configuration
Create `.env` file with your API keys:
```env
# Required for Anthropic
ANTHROPIC_API_KEY=your_key_here  

# Required for Groq 
GROQ_API_KEY=your_key_here

```

## System Architecture

![High Level Architecture](docs/HighLevel.jpg)

### Core Components

1. **Audio Capture Service**
   - Real-time microphone/system audio capture
   - Audio preprocessing pipeline
   - Built with Rust (experimental) and Python

2. **Transcription Engine**
   - Whisper.cpp for local transcription
   - Supports multiple model sizes (tiny->large)
   - GPU-accelerated processing

3. **LLM Orchestrator**
   - Unified interface for multiple providers
   - Automatic fallback handling
   - Chunk processing with overlap
   - Model configuration:

4. **Data Services**
   - **ChromaDB**: Vector store for transcript embeddings
   - **SQLite**: Process tracking and metadata storage

5. **API Layer**
   - FastAPI endpoints:
     - POST /upload
     - POST /process
     - GET /summary/{id}
     - DELETE /summary/{id}

### Deployment Architecture

- **Frontend**: Tauri app + Next.js (packaged executables)
- **Backend**: Python FastAPI:
  - Transcript workers
  - LLM inference

## Prerequisites

- Node.js 18+
- Python 3.10+
- FFmpeg
- Rust 1.65+ (for experimental features)

## Setup Instructions

### 1. Frontend Setup

#### Run packaged version

Go to the [releases page](https://github.com/Zackriya-Solutions/meeting-minutes/releases) and download the latest version.

Unzip the file and run the executable.

Provide necessary permissions for audio capture and microphone access (Only screen capture permission is required).

#### Dev run

```bash

# Navigate to frontend directory
cd frontend

# Give execute permissions to clean_build.sh
chmod +x clean_build.sh

# run clean_build.sh
./clean_build.sh
```

### 2. Backend Setup

```bash
# Create and activate virtual environment
python -m venv venv
source venv/bin/activate  # Windows: .\venv\Scripts\activate

# Navigate to backend directory
cd backend

# Install dependencies
pip install -r requirements.txt

# Start backend servers
./clean_start_backend.sh
```

## Development Guidelines

- Follow the established project structure
- Write tests for new features
- Document API changes
- Use type hints in Python code
- Follow ESLint configuration for JavaScript/TypeScript

## Contributing

1. Fork the repository
2. Create a feature branch
3. Submit a pull request

## License

MIT License - Feel free to use this project for your own purposes.

Last updated: December 26, 2024

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Zackriya-Solutions/meeting-minutes&type=Date)](https://star-history.com/#Zackriya-Solutions/meeting-minutes&Date)