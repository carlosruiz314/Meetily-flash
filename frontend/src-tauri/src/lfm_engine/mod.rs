//! LFM2.5-Audio (Liquid AI) speech recognition engine module.
//!
//! This module provides ASR using Liquid AI's LFM2.5-Audio model via a Python sidecar.
//! LFM2.5-Audio offers competitive transcription quality with Whisper while also
//! supporting TTS and conversational audio capabilities.
//!
//! # Features
//!
//! - **Competitive ASR**: 7.53% WER (vs Whisper's 7.44%)
//! - **Python Sidecar**: Uses `liquid-audio` package for inference
//! - **GPU Support**: Automatic CUDA/MPS/CPU detection
//! - **Unified API**: Compatible interface with Whisper/Parakeet engines
//! - **Auto Setup**: Automatic venv creation and dependency installation
//!
//! # Architecture
//!
//! On first use, the engine automatically:
//! 1. Creates a Python virtual environment in the app data directory
//! 2. Installs liquid-audio and dependencies via pip
//! 3. Downloads the LFM2.5-Audio model (~3GB) from HuggingFace
//! 4. Manages the Python sidecar process for inference

pub mod lfm_engine;
pub mod commands;
pub mod setup;

pub use lfm_engine::{LfmEngine, LfmEngineError, ModelStatus};
pub use commands::*;
pub use setup::{LfmSetupManager, SetupProgress, SetupStage};
