# Meeting Minutes Backend

FastAPI backend for meeting transcription and analysis

## Features
- Audio file upload and storage
- Real-time Whisper-based transcription with streaming support
- Meeting analysis with LLMs (supports Claude, Groq, and Ollama)
- REST API endpoints

## Requirements
- Python 3.9+
- FFmpeg
- C++ compiler (for Whisper.cpp)
- CMake
- Git (for submodules)
- Ollama running
- API Keys (for Claude or Groq) if planning to use APIS
- ChromaDB

## Installation

### 1. Environment Setup
Create `.env` file in the backend directory:
```bash
ANTHROPIC_API_KEY=your_key_here  # Optional, for Claude
GROQ_API_KEY=your_key_here      # Optional, for Groq
```

### 2. Build Whisper Server
Run the build script which will:
- Initialize and update git submodules
- Build Whisper.cpp with custom server modifications
- Set up the server package with required files
- Download the selected Whisper model

```bash
./build_whisper.sh
```

If no model is specified, the script will prompt you to choose one interactively.

### 3. Running the Server
The clean start script provides an interactive way to start the backend services:

```bash
./clean_start_backend.sh
```

The script will:
1. Check and clean up any existing processes
2. Verify environment setup and required directories
3. Check for existing Whisper models
4. Download the selected model if not present
5. Start the Whisper server
6. Start the FastAPI backend in a Python virtual environment

To stop all services, press Ctrl+C. The script will automatically clean up all processes.

## API Documentation
Access Swagger UI at `http://localhost:5167/docs`

## Services
The backend runs two services:
1. Whisper.cpp Server: Handles real-time audio transcription
2. FastAPI Backend: Manages API endpoints, LLM integration, and data storage


## Troubleshooting
- If services fail to start, the script will automatically clean up processes
- Check logs for detailed error messages
- Ensure all ports (5167 for backend) are available
- Verify API keys if using Claude or Groq
- For Ollama, ensure the Ollama service is running and models are pulled
- If build fails:
  - Ensure all dependencies (CMake, C++ compiler) are installed
  - Check if git submodules are properly initialized
  - Verify you have write permissions in the directory
