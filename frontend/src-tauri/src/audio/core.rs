use super::audio_processing::audio_to_mono; 
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamError;
use lazy_static::lazy_static;
use log::{ error, info, warn, debug};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use std::{fmt, thread};
use tokio::sync::{broadcast, oneshot};
lazy_static! {
    pub static ref LAST_AUDIO_CAPTURE: AtomicU64 = AtomicU64::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioTranscriptionEngine {
    Deepgram,
    WhisperTiny,
    WhisperDistilLargeV3,
    WhisperLargeV3Turbo,
    WhisperLargeV3,
}

impl fmt::Display for AudioTranscriptionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioTranscriptionEngine::Deepgram => write!(f, "Deepgram"),
            AudioTranscriptionEngine::WhisperTiny => write!(f, "WhisperTiny"),
            AudioTranscriptionEngine::WhisperDistilLargeV3 => write!(f, "WhisperLarge"),
            AudioTranscriptionEngine::WhisperLargeV3Turbo => write!(f, "WhisperLargeV3Turbo"),
            AudioTranscriptionEngine::WhisperLargeV3 => write!(f, "WhisperLargeV3"),
        }
    }
}

impl Default for AudioTranscriptionEngine {
    fn default() -> Self {
        AudioTranscriptionEngine::WhisperLargeV3Turbo
    }
}

#[derive(Clone, Debug)]
pub struct DeviceControl {
    pub is_running: bool,
    pub is_paused: bool,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Debug, Deserialize)]
pub enum DeviceType {
    Input,
    Output,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Debug)]
pub struct AudioDevice {
    pub name: String,
    pub device_type: DeviceType,
}

impl AudioDevice {
    pub fn new(name: String, device_type: DeviceType) -> Self {
        AudioDevice { name, device_type }
    }

    pub fn from_name(name: &str) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(anyhow!("Device name cannot be empty"));
        }

        let (name, device_type) = if name.to_lowercase().ends_with("(input)") {
            (
                name.trim_end_matches("(input)").trim().to_string(),
                DeviceType::Input,
            )
        } else if name.to_lowercase().ends_with("(output)") {
            (
                name.trim_end_matches("(output)").trim().to_string(),
                DeviceType::Output,
            )
        } else {
            return Err(anyhow!(
                "Device type (input/output) not specified in the name"
            ));
        };

        Ok(AudioDevice::new(name, device_type))
    }
}

impl fmt::Display for AudioDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} ({})",
            self.name,
            match self.device_type {
                DeviceType::Input => "input",
                DeviceType::Output => "output",
            }
        )
    }
}

pub fn parse_audio_device(name: &str) -> Result<AudioDevice> {
    AudioDevice::from_name(name)
}

pub async fn get_device_and_config(
    audio_device: &AudioDevice,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
    let host = cpal::default_host();

    let is_output_device = audio_device.device_type == DeviceType::Output;
    let is_display = audio_device.to_string().contains("Display");

    let cpal_audio_device = if audio_device.to_string() == "default" {
        match audio_device.device_type {
            DeviceType::Input => host.default_input_device(),
            DeviceType::Output => host.default_output_device(),
        }
    } else {
        let mut devices = match audio_device.device_type {
            DeviceType::Input => host.input_devices()?,
            DeviceType::Output => host.output_devices()?,
        };

        #[cfg(target_os = "macos")]
        {
            if is_output_device {
                if let Ok(screen_capture_host) = cpal::host_from_id(cpal::HostId::ScreenCaptureKit)
                {
                    devices = screen_capture_host.input_devices()?;
                }
            }
        }

        devices.find(|x| {
            x.name()
                .map(|y| {
                    y == audio_device
                        .to_string()
                        .replace(" (input)", "")
                        .replace(" (output)", "")
                        .trim()
                })
                .unwrap_or(false)
        })
    }
    .ok_or_else(|| anyhow!("Audio device not found"))?;

    // if output device and windows, using output config
    let config = if is_output_device && !is_display {
        cpal_audio_device.default_output_config()?
    } else {
        cpal_audio_device.default_input_config()?
    };
    Ok((cpal_audio_device, config))
}


pub async fn list_audio_devices() -> Result<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let mut devices = Vec::new();

    for device in host.input_devices()? {
        if let Ok(name) = device.name() {
            devices.push(AudioDevice::new(name, DeviceType::Input));
        }
    }

    // Filter function to exclude macOS speakers and AirPods for output devices
    fn should_include_output_device(name: &str) -> bool {
        #[cfg(target_os = "macos")]
        {
            !name.to_lowercase().contains("speakers") && !name.to_lowercase().contains("airpods")
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Avoid "unused variable" warning in non-macOS systems
            let _ = name;
            true
        }
    }

    // macos hack using screen capture kit for output devices - does not work well
    #[cfg(target_os = "macos")]
    {
        // !HACK macos is supposed to use special macos feature "display capture"
        // ! see https://github.com/RustAudio/cpal/pull/894
        if let Ok(host) = cpal::host_from_id(cpal::HostId::ScreenCaptureKit) {
            for device in host.input_devices()? {
                if let Ok(name) = device.name() {
                    if should_include_output_device(&name) {
                        devices.push(AudioDevice::new(name, DeviceType::Output));
                    }
                }
            }
        }
    }

    // add default output device - on macos think of custom virtual devices
    for device in host.output_devices()? {
        if let Ok(name) = device.name() {
            if should_include_output_device(&name) {
                devices.push(AudioDevice::new(name, DeviceType::Output));
            }
        }
    }

    // last, add devices that are listed in .devices() which are not already in the devices vector
    let other_devices = host.devices().unwrap();
    for device in other_devices {
        if !devices.iter().any(|d| d.name == device.name().unwrap())
            && should_include_output_device(&device.name().unwrap())
        {
            // TODO: not sure if it can be input, usually aggregate or multi output
            devices.push(AudioDevice::new(device.name().unwrap(), DeviceType::Output));
        }
    }

    Ok(devices)
}

pub fn default_input_device() -> Result<AudioDevice> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or(anyhow!("No default input device detected"))?;
    Ok(AudioDevice::new(device.name()?, DeviceType::Input))
}
// this should be optional ?
pub fn default_output_device() -> Result<AudioDevice> {
    #[cfg(target_os = "macos")]
    {
        // ! see https://github.com/RustAudio/cpal/pull/894
        if let Ok(host) = cpal::host_from_id(cpal::HostId::ScreenCaptureKit) {
            if let Some(device) = host.default_input_device() {
                if let Ok(name) = device.name() {
                    return Ok(AudioDevice::new(name, DeviceType::Output));
                }
            }
        }
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No default output device found"))?;
        return Ok(AudioDevice::new(device.name()?, DeviceType::Output));
    }

    #[cfg(not(target_os = "macos"))]
    {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No default output device found"))?;
        return Ok(AudioDevice::new(device.name()?, DeviceType::Output));
    }
}

pub fn trigger_audio_permission() -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No default input device found"))?;

    let config = device.default_input_config()?;

    // Build and start an input stream to trigger the permission request
    let stream = device.build_input_stream(
        &config.into(),
        |_data: &[f32], _: &cpal::InputCallbackInfo| {
            // Do nothing, we just want to trigger the permission request
        },
        |err| error!("Error in audio stream: {}", err),
        None,
    )?;

    // Start the stream to actually trigger the permission dialog
    stream.play()?;

    // Sleep briefly to allow the permission dialog to appear
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Stop the stream
    drop(stream);

    Ok(())
}

#[derive(Clone)]
pub struct AudioStream {
    pub device: Arc<AudioDevice>,
    pub device_config: cpal::SupportedStreamConfig,
    transmitter: Arc<tokio::sync::broadcast::Sender<Vec<f32>>>,
    stream_control: mpsc::Sender<StreamControl>,
    stream_thread: Option<Arc<tokio::sync::Mutex<Option<thread::JoinHandle<()>>>>>,
    is_disconnected: Arc<AtomicBool>,
}

enum StreamControl {
    Stop(oneshot::Sender<()>),
}

impl AudioStream {
    pub async fn from_device(
        device: Arc<AudioDevice>,
        is_running: Arc<AtomicBool>,
    ) -> Result<Self> {
        info!("Initializing audio stream for device: {}", device.to_string());
        let (tx, _) = broadcast::channel::<Vec<f32>>(1000);
        let tx_clone = tx.clone();
        let (cpal_audio_device, config) = get_device_and_config(&device).await?;
        
        // Verify we can actually get input config
        match cpal_audio_device.default_input_config() {
            Ok(conf) => info!("Default input config: {:?}", conf),
            Err(e) => {
                error!("Failed to get default input config: {}", e);
                return Err(anyhow!("Failed to get default input config: {}", e));
            }
        }
        
        let channels = config.channels();
        info!("Audio config - Sample rate: {}, Channels: {}, Format: {:?}", 
            config.sample_rate().0, channels, config.sample_format());

        let is_running_weak_2 = Arc::downgrade(&is_running);
        let is_disconnected = Arc::new(AtomicBool::new(false));
        let device_clone = device.clone();
        let config_clone = config.clone();
        let (stream_control_tx, stream_control_rx) = mpsc::channel();

        let is_disconnected_clone = is_disconnected.clone();
        let stream_control_tx_clone = stream_control_tx.clone();
        let stream_thread = Arc::new(tokio::sync::Mutex::new(Some(thread::spawn(move || {
            let device = device_clone;
            let device_name = device.to_string();
            let device_name_clone = device_name.clone();  // Clone for the closure
            let config = config_clone;
            info!("Starting audio stream thread for device: {}", device_name);
            let error_callback = move |err: StreamError| {
                if err
                    .to_string()
                    .contains("The requested device is no longer available")
                {
                    warn!(
                        "audio device {} disconnected. stopping recording.",
                        device_name_clone
                    );
                    stream_control_tx_clone
                        .send(StreamControl::Stop(oneshot::channel().0))
                        .unwrap();

                    is_disconnected_clone.store(true, Ordering::Relaxed);
                } else if err.to_string().to_lowercase().contains("permission denied") || 
                         err.to_string().to_lowercase().contains("access denied") {
                    error!("Permission denied for audio device {}. Please check microphone permissions.", device_name_clone);
                    if let Some(arc) = is_running_weak_2.upgrade() {
                        arc.store(false, Ordering::Relaxed);
                    }
                } else {
                    error!("an error occurred on the audio stream: {}", err);
                    if err.to_string().contains("device is no longer valid") {
                        warn!("audio device disconnected. stopping recording.");
                        if let Some(arc) = is_running_weak_2.upgrade() {
                            arc.store(false, Ordering::Relaxed);
                        }
                    }
                }
            };

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    match cpal_audio_device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _: &_| {
                            let mono = audio_to_mono(data, channels);
                            debug!("Received audio chunk: {} samples", mono.len());
                            if let Err(e) = tx.send(mono) {
                                error!("Failed to send audio data: {}", e);
                            }
                        },
                        error_callback.clone(),
                        None,
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Failed to build input stream: {}", e);
                            return;
                        }
                    }
                }
                cpal::SampleFormat::I16 => {
                    match cpal_audio_device.build_input_stream(
                        &config.into(),
                        move |data: &[i16], _: &_| {
                            let mono = audio_to_mono(bytemuck::cast_slice(data), channels);
                            debug!("Received audio chunk: {} samples", mono.len());
                            if let Err(e) = tx.send(mono) {
                                error!("Failed to send audio data: {}", e);
                            }
                        },
                        error_callback.clone(),
                        None,
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Failed to build input stream: {}", e);
                            return;
                        }
                    }
                }
                cpal::SampleFormat::I32 => {
                    match cpal_audio_device.build_input_stream(
                        &config.into(),
                        move |data: &[i32], _: &_| {
                            let mono = audio_to_mono(bytemuck::cast_slice(data), channels);
                            debug!("Received audio chunk: {} samples", mono.len());
                            if let Err(e) = tx.send(mono) {
                                error!("Failed to send audio data: {}", e);
                            }
                        },
                        error_callback.clone(),
                        None,
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Failed to build input stream: {}", e);
                            return;
                        }
                    }
                }
                cpal::SampleFormat::I8 => {
                    match cpal_audio_device.build_input_stream(
                        &config.into(),
                        move |data: &[i8], _: &_| {
                            let mono = audio_to_mono(bytemuck::cast_slice(data), channels);
                            debug!("Received audio chunk: {} samples", mono.len());
                            if let Err(e) = tx.send(mono) {
                                error!("Failed to send audio data: {}", e);
                            }
                        },
                        error_callback.clone(),
                        None,
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Failed to build input stream: {}", e);
                            return;
                        }
                    }
                }
                _ => {
                    error!("unsupported sample format: {}", config.sample_format());
                    return;
                }
            };

            if let Err(e) = stream.play() {
                error!("failed to play stream for {}: {}", device.to_string(), e);
                let err_str = e.to_string().to_lowercase();
                if err_str.contains("permission") {
                    error!("Permission error detected. Please check microphone permissions in System Preferences");
                } else if err_str.contains("busy") {
                    error!("Device is busy. Another application might be using it");
                }
                return;
            }
            info!("Audio stream started successfully for device: {}", device_name);
            if let Ok(StreamControl::Stop(response)) = stream_control_rx.recv() {
                info!("stopping audio stream...");
                // First stop the stream
                if let Err(e) = stream.pause() {
                    error!("failed to pause stream: {}", e);
                }
                // Close the stream to release OS resources
                drop(stream);
                // Signal completion
                response.send(()).ok();
                info!("audio stream stopped and cleaned up");
            }
        }))));

        Ok(AudioStream {
            device,
            device_config: config,
            transmitter: Arc::new(tx_clone),
            stream_control: stream_control_tx,
            stream_thread: Some(stream_thread),
            is_disconnected,
        })
    }

    pub async fn subscribe(&self) -> broadcast::Receiver<Vec<f32>> {
        self.transmitter.subscribe()
    }

    pub async fn stop(&self) -> Result<()> {
        // Mark as disconnected first
        self.is_disconnected.store(true, Ordering::Release);
        
        // Send stop signal and wait for confirmation
        let (tx, _rx) = oneshot::channel();
        self.stream_control.send(StreamControl::Stop(tx))?;

        // Wait for thread to finish
        if let Some(thread_arc) = &self.stream_thread {
            let thread_arc = thread_arc.clone();
            let thread_handle = tokio::task::spawn_blocking(move || {
                let mut thread_guard = thread_arc.blocking_lock();
                if let Some(join_handle) = thread_guard.take() {
                    join_handle
                        .join()
                        .map_err(|_| anyhow!("failed to join stream thread"))
                } else {
                    Ok(())
                }
            });

            thread_handle.await??;
        }

        Ok(())
    }
}
