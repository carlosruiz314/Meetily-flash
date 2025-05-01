<div align="center" style="border-bottom: none">
    <h1>
        <img src="docs/Meetily-6.png" style="border-radius: 10px;" />
        <br>
        Your AI-Powered Meeting Assistant
    </h1>
    <a href="https://trendshift.io/repositories/13272" target="_blank"><img src="https://trendshift.io/api/badge/repositories/13272" alt="Zackriya-Solutions%2Fmeeting-minutes | Trendshift" style="width: 250px; height: 55px;" width="250" height="55"/></a>
    <br>
    <br>
    <a href="https://github.com/Zackriya-Solutions/meeting-minutes/releases/"><img src="https://img.shields.io/badge/Pre_Release-Link-brightgreen" alt="Pre-Release"></a>
    <a href="https://github.com/Zackriya-Solutions/meeting-minutes/releases"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/zackriya-solutions/meeting-minutes?style=flat">
</a>
 <a href="https://github.com/Zackriya-Solutions/meeting-minutes/releases"> <img alt="GitHub Downloads (all assets, all releases)" src="https://img.shields.io/github/downloads/zackriya-solutions/meeting-minutes/total?style=plastic"> </a>
    <a href="https://github.com/Zackriya-Solutions/meeting-minutes/releases"><img src="https://img.shields.io/badge/License-MIT-blue" alt="License"></a>
    <a href="https://github.com/Zackriya-Solutions/meeting-minutes/releases"><img src="https://img.shields.io/badge/Supported_OS-macOS,_Windows-white" alt="Supported OS"></a>
    <a href="https://github.com/Zackriya-Solutions/meeting-minutes/releases"><img alt="GitHub Tag" src="https://img.shields.io/github/v/tag/zackriya-solutions/meeting-minutes?include_prereleases&color=yellow">
</a>
    <br>
    <h3>
    <br>
    Open source Ai Assistant for taking meeting notes
    </h3>
    <p align="center">
    Get latest <a href="https://www.zackriya.com/meetily-subscribe/"><b>Product updates</b></a> <br><br>
    <a href="https://meetily.zackriya.com"><b>Website</b></a> •
    <a href="https://in.linkedin.com/company/zackriya-solutions"><b>Authors</b></a>
    •
    <a href="https://discord.gg/crRymMQBFH"><b>Discord Channel</b></a>
</p>
    <p align="center">
    
 An AI-Powered Meeting Assistant that captures live meeting audio, transcribes it in real-time, and generates summaries while ensuring user privacy. Perfect for teams who want to focus on discussions while automatically capturing and organizing meeting content without the need for external servers or complex infrastructure. 
</p>

<p align="center">
    <img src="docs/demo_small.gif" width="650" alt="Meetily Demo" />
    <br>
    <a href="https://youtu.be/5k_Q5Wlahuk">View full Demo Video</a>
</p>

</div>

# Table of Contents
- [Overview](#overview)
- [Features](#features)
- [System Architecture](#system-architecture)
  - [Core Components](#core-components)
  - [Deployment Architecture](#deployment-architecture)
- [Prerequisites](#prerequisites)
- [Setup Instructions](#setup-instructions)
  - [Windows OS](#windows-os)
    - [Frontend Setup](#1-frontend-setup)
    - [Backend Setup](#2-backend-setup)
  - [macOS](#for-macos)
    - [Frontend Setup](#1-frontend-setup-1)
    - [Backend Setup](#2-backend-setup-1)
- [Development Setup](#development-setup)
- [Whisper Model Selection](#whisper-model-selection)
- [Known Issues](#known-issues)
- [LLM Integration](#llm-integration)
  - [Supported Providers](#supported-providers)
  - [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
  - [Backend Issues](#backend-issues)
  - [Frontend Issues](#frontend-issues)
- [Uninstallation](#uninstallation)
- [Development Guidelines](#development-guidelines)
- [Contributing](#contributing)
- [License](#license)
- [Introducing Subscription](#introducing-subscription)
- [Contributions](#contributions)
- [Acknowledgments](#acknowledgments)
- [Star History](#star-history)

# Overview

An AI-powered meeting assistant that captures live meeting audio, transcribes it in real-time, and generates summaries while ensuring user privacy. Perfect for teams who want to focus on discussions while automatically capturing and organizing meeting content.

### Why?

While there are many meeting transcription tools available, this solution stands out by offering:
- **Privacy First**: All processing happens locally on your device
- **Cost Effective**: Uses open-source AI models instead of expensive APIs
- **Flexible**: Works offline, supports multiple meeting platforms
- **Customizable**: Self-host and modify for your specific needs
- **Intelligent**: Built-in knowledge graph for semantic search across meetings

# Features

✅ Modern, responsive UI with real-time updates

✅ Real-time audio capture (microphone + system audio)

✅ Live transcription using locally-running Whisper

✅ Local processing for privacy

✅ Packaged the app for macOS and Windows

✅ Rich text editor for notes

🚧 Export to Markdown/PDF/HTML

🚧 Obsidian Integration 

🚧 Speaker diarization

---


# System Architecture

<p align="center">
    <img src="docs/HighLevel.jpg" width="900" alt="Meetily High Level Architecture" />
</p>

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
- Cmake 3.22+ (for building the frontend)
- For Windows: Visual Studio Build Tools with C++ development workload


# Setup Instructions

## Windows OS

### 1. Frontend Setup

**Option 1: Using the Setup Executable (.exe) (Recommended)**
1. Download the `meetily-frontend_0.0.4_x64-setup.exe` file
2. Double-click the installer to run it
3. Follow the on-screen instructions to complete the installation
4. The application will be available on your desktop

**Note:** Windows may display a security warning. To bypass this:
- Click `More info` and choose `Run anyway`, or
- Right-click on the installer (.exe), select Properties, and check the Unblock checkbox at the bottom

<p align="center">
    <img src="https://github.com/user-attachments/assets/f2a2655d-9881-42ed-88aa-357a1f5b6118" width="300" alt="Windows Security Warning" />
</p>

**Option 2: Using the MSI Installer (.msi)**
1. Download the `meetily-frontend_0.0.4_x64_en-US.msi` file
2. Double-click the MSI file to run it
3. Follow the installation wizard to complete the setup
4. The application will be installed and available on your desktop

Provide necessary permissions for audio capture and microphone access.

### 2. Backend Setup


<p align="center">
<a href="https://www.youtube.com/watch?v=Tu_8wXgoaDE">
    <img src="https://img.youtube.com/vi/Tu_8wXgoaDE/0.jpg"  alt="Windows Security Warning" />
</a>
</p>


**Option 1: Manual Setup**
1. Clone the repository:
```bash
git clone https://github.com/Zackriya-Solutions/meeting-minutes
cd meeting-minutes/backend
```

2. Build dependencies:
```bash
.\build_whisper.cmd
```

3. Start the backend servers:
```bash
.\start_with_output.ps1
```

**Option 2: Docker Setup (including ARM64/Snapdragon)**
```bash
# Clone the repository
git clone https://github.com/Zackriya-Solutions/meeting-minutes.git
cd meeting-minutes

# Run the Docker build script (interactive setup)
.\docker-build.bat
```

### Docker Configuration Options

The Docker setup for both macOS and Windows allows you to configure:
- Whisper model selection (tiny, base, small, medium, large-v3, etc.)
- Language preference (auto-detection or specific language)
- Logging level

## For macOS:

### 1. Frontend Setup

Go to the [releases page](https://github.com/Zackriya-Solutions/meeting-minutes/releases) and download the latest version.


**Option 1: Using Homebrew (Recommended)**

> **Note** : This step installs the backend server and the frontend app.
> Once the backend and the frontend are started, you can open the application from the Applications folder.

```bash
# Install Meetily using Homebrew
brew tap zackriya-solutions/meetily
brew install --cask meetily

# Start the backend server
meetily-server --language en --model medium
```

**Option 2: Manual Installation**
- Download the `dmg_darwin_arch64.zip` file
- Extract the file
- Double-click the `.dmg` file inside the extracted folder
- Drag the application to your Applications folder
- Execute the following command in terminal to remove the quarantine attribute:
```
  xattr -c /Applications/meetily-frontend.app
```

Provide necessary permissions for audio capture and microphone access.

### 2. Backend Setup

**Option 1: Using Homebrew (Recommended)**
```bash

(Optional)

# If meetily is already installed in your system, uninstall the current versions

brew uninstall meetily

brew uninstall meetily-backend

brew untap zackriya-solutions/meetily

```

```bash

  

# Install Meetily using Homebrew

brew tap zackriya-solutions/meetily

brew install --cask meetily

  

# Start the backend server

meetily-server --language en --model medium

```

**Option 2: Manual Setup**
```bash
# Clone the repository
git clone https://github.com/Zackriya-Solutions/meeting-minutes.git
cd meeting-minutes/backend

# Create and activate virtual environment
python -m venv venv
source venv/bin/activate

# Install dependencies
pip install -r requirements.txt


# Build dependencies
chmod +x build_whisper.sh
./build_whisper.sh

# Start backend servers
./clean_start_backend.sh
```


### Development Setup

```bash
# Navigate to frontend directory
cd frontend

# Give execute permissions to clean_build.sh
chmod +x clean_build.sh

# run clean_build.sh
./clean_build.sh
```

### Whisper Model Selection

When setting up the backend (either via Homebrew, manual installation, or Docker), you can choose from various Whisper models based on your needs:

1. **Standard models** (balance of accuracy and speed):
   - tiny, base, small, medium

2. **English-optimized models** (faster for English content):
   - tiny.en, base.en, small.en, medium.en

3. **Advanced models** (for special needs):
   - large-v3, large-v3-turbo
   - small.en-tdrz (with speaker diarization)

4. **Quantized models** (reduced size, slightly lower quality):
   - tiny-q5_1, base-q5_1, small-q5_1, medium-q5_0


### Known issues
- Smaller LLMs can hallucinate, making summarization quality poor; Please use model above 32B parameter size
- Backend build process requires CMake, C++ compiler, etc. Making it harder to build
- Backend build process requires Python 3.10 or newer
- Frontend build process requires Node.js

## LLM Integration

The backend supports multiple LLM providers through a unified interface. Current implementations include:

### Supported Providers
- **Anthropic** (Claude models)
- **Groq** (Llama3.2 90 B)
- **Ollama** (Local models that supports function calling)



## Troubleshooting

### Backend Issues

#### Model Problems

If you encounter issues with the Whisper model:

```bash
# Try a different model size
meetily-download-model small

# Verify model installation
ls -la $(brew --prefix)/opt/meetily-backend/backend/whisper-server-package/models/
```

#### Server Connection Issues

If the server fails to start:

1. Check if ports 8178 and 5167 are available:
   ```bash
   lsof -i :8178
   lsof -i :5167
   ```

2. Verify that FFmpeg is installed correctly:
   ```bash
   which ffmpeg
   ffmpeg -version
   ```

3. Check the logs for specific error messages when running `meetily-server`

4. Try running the Whisper server manually:
   ```bash
   cd $(brew --prefix)/opt/meetily-backend/backend/whisper-server-package/
   ./run-server.sh --model models/ggml-medium.bin
   ```

### Frontend Issues

If the frontend application doesn't connect to the backend:

1. Ensure the backend server is running (`meetily-server`)
2. Check if the application can access localhost:5167
3. Restart the application after starting the backend

If the application fails to launch:

```bash
# Clear quarantine attributes
xattr -cr /Applications/meetily-frontend.app
```

## Uninstallation

To completely remove Meetily:

```bash
# Remove the frontend
brew uninstall --cask meetily

# Remove the backend
brew uninstall meetily-backend

# Optional: remove the taps
brew untap zackriya-solutions/meetily
brew untap zackriya-solutions/meetily-backend

# Optional: remove Ollama if no longer needed
brew uninstall ollama
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

## Introducing Subscription

We are planning to add a subscription option so that you don't have to run the backend on your own server. This will help you scale better and run the service 24/7. This is based on a few requests we received. If you are interested, please fill out the form [here](http://zackriya.com/aimeeting/).

## Contributions

Thanks for all the contributions. Our community is what makes this project possible. Below is the list of contributors:

<a href="https://github.com/zackriya-solutions/meeting-minutes/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=zackriya-solutions/meeting-minutes" />
</a>


We welcome contributions from the community! If you have any questions or suggestions, please open an issue or submit a pull request. Please follow the established project structure and guidelines. For more details, refer to the [CONTRIBUTING](CONTRIBUTING.md) file.

## Acknowledgments

- We borrowes some code from [Whisper.cpp](https://github.com/ggerganov/whisper.cpp)
- We borrowes some code from [Screenpipe](https://github.com/mediar-ai/screenpipe)


## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Zackriya-Solutions/meeting-minutes&type=Date)](https://star-history.com/#Zackriya-Solutions/meeting-minutes&Date)
