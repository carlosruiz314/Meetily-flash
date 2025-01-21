# Whisper Server Package

This package contains a pre-built Whisper server for real-time speech recognition with diarization support.

## Contents
- `whisper-server`: The main server binary
- `models/`: Directory containing the Whisper model
- `public/`: Web interface files
- `run-server.sh`: Script to start the server

## Requirements
- Mac OS X (for Metal support)
- curl (for downloading models if needed)

## Usage

1. Start the server:
   ```bash
   ./run-server.sh
   ```

2. Optional parameters:
   ```bash
   ./run-server.sh --host 0.0.0.0 --port 8178
   ```

3. Access the web interface:
   Open http://localhost:8178 in your browser

## Features
- Real-time speech recognition
- Speaker diarization
- Web interface for audio capture and transcription
- JSON API for programmatic access

## API Documentation
See the API.md file in the original repository for detailed API documentation.
