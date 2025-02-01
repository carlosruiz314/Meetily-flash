// src/audio/mod.rs
pub mod core;
pub mod audio_processing;
pub mod encode;
pub mod ffmpeg;

pub use core::{
    default_input_device, default_output_device, get_device_and_config, list_audio_devices,
    parse_audio_device, trigger_audio_permission,
    AudioDevice, AudioStream, AudioTranscriptionEngine, DeviceControl, DeviceType,
    LAST_AUDIO_CAPTURE,
};
pub use encode::{
    encode_single_audio, AudioInput
};