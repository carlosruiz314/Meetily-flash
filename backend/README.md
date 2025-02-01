# Meeting Minutes Backend

FastAPI backend for meeting transcription and analysis

## Features
- Audio file upload and storage
- Whisper-based transcription
- Meeting analysis with LLMs
- REST API endpoints

## Requirements
- Python 3.9+
- FFmpeg
- API Keys
- ChromaDB

## Installation
```bash
pip install -r requirements.txt
```

## Environment Setup (Optional if you are using API keys)
Create `.env` file:
```
ANTHROPIC_API_KEY=your_key_here
GROQ_API_KEY=your_key_here
```

## Running the Server
```bash
./clean_start_backend.sh
```

## API Documentation
Access Swagger UI at `http://localhost:8000/docs`

## Testing
```bash
pytest tests/
```

## Deployment
See `deploy/` directory for Kubernetes manifests
