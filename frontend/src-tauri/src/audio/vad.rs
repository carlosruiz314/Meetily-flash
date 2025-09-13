use anyhow::{anyhow, Result};
use silero_rs::{VadConfig, VadSession, VadTransition};
use log::{debug};


/// Runs a quick Silero VAD over a mono 16kHz buffer.
/// Returns concatenated speech-only samples if any speech is detected,
/// otherwise returns an empty Vec to indicate no speech.
pub fn extract_speech_16k(samples_mono_16k: &[f32]) -> Result<Vec<f32>> {
    let mut config = VadConfig::default();
    config.sample_rate = 16_000usize;
    // Very lenient settings to avoid filtering out speech
    config.redemption_time = std::time::Duration::from_millis(50);      // Very short redemption
    config.pre_speech_pad = std::time::Duration::from_millis(100);      // Short pre-pad
    config.post_speech_pad = std::time::Duration::from_millis(25);      // Short post-pad
    config.min_speech_time = std::time::Duration::from_millis(5);       // Very low minimum

    let mut session = VadSession::new(config).map_err(|_| anyhow!("VadSessionCreationFailed"))?;

    // Process in 30ms frames (480 samples @ 16kHz)
    let frame_len = 480usize;
    let mut speech_out: Vec<f32> = Vec::new();
    let mut in_speech = false;
    let mut speech_start_idx = 0;

    debug!("VAD: Processing {} samples in {} frames", samples_mono_16k.len(), samples_mono_16k.len() / frame_len);

    for (frame_idx, frame) in samples_mono_16k.chunks(frame_len).enumerate() {
        if frame.is_empty() { continue; }
        
        let transitions = session.process(frame)
            .map_err(|e| anyhow!("VadProcessingFailed: {}", e))?;
        
        for t in transitions {
            match t {
                VadTransition::SpeechStart { .. } => {
                    debug!("VAD: Speech started at frame {}", frame_idx);
                    in_speech = true;
                    speech_start_idx = frame_idx * frame_len;
                }
                VadTransition::SpeechEnd { samples, .. } => {
                    debug!("VAD: Speech ended at frame {}, collected {} samples", frame_idx, samples.len());
                    in_speech = false;
                    // Add the samples from this transition
                    if !samples.is_empty() {
                        speech_out.extend_from_slice(&samples);
                    }
                    // Also add any samples we collected during speech
                    let speech_end_idx = (frame_idx + 1) * frame_len;
                    if speech_start_idx < speech_end_idx {
                        let collected_samples = &samples_mono_16k[speech_start_idx..speech_end_idx];
                        speech_out.extend_from_slice(collected_samples);
                    }
                }
            }
        }
        
        // If we're in speech, collect this frame's samples
        if in_speech {
            speech_out.extend_from_slice(frame);
        }
    }

    debug!("VAD: Input {} samples, output {} speech samples", 
          samples_mono_16k.len(), speech_out.len());
    
    // Adaptive threshold based on input audio levels
    let input_avg_level = samples_mono_16k.iter().map(|&x| x.abs()).sum::<f32>() / samples_mono_16k.len() as f32;
    
    if speech_out.len() < frame_len / 32 { // Super lenient - only 1/32 of a frame (15 samples)
        // If input has very low levels, it's probably silence - skip it
        if input_avg_level < 0.001 {
            debug!("VAD: Very low input levels ({:.6}), skipping silent chunk", input_avg_level);
            return Ok(Vec::new());
        } else {
            // Input has some audio but VAD didn't detect speech - include it anyway
            // This prevents losing audio during VAD false negatives
            debug!("VAD: Input has audio ({:.6}) but VAD detected no speech, including input anyway", input_avg_level);
            return Ok(samples_mono_16k.to_vec());
        }
    }

    Ok(speech_out)
}

 
