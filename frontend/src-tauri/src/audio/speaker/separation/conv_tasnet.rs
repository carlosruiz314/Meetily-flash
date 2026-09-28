//! Conv-TasNet separation adapter (overlap-separation-prepass task 2.3):
//! ORT session over the hash-pinned ONNX export (task 2.1), implementing the
//! [`VoiceSeparationPort`]. Measured in-domain behavior the geometry below is
//! built on (probe 2.2, 2026-09-28):
//! - the model needs CONTEXT — a bare 0.4 s span separates weakly, the same
//!   span carved from a 3 s run beats the mixture (0.34 vs 0.21 cosine);
//! - the export's output gain is wild (stream RMS thousands × the input),
//!   and TitaNet is log-compressed, so streams are RMS-normalized to the
//!   input clip before anything downstream sees them.

use anyhow::{anyhow, Result};
use std::sync::Mutex;

use crate::audio::speaker::ports::{SeparatedStream, VoiceSeparationPort};

/// Seconds of extra context on each side of the span the model infers over;
/// the span itself is carved from the streams afterwards. Measured: 1.3 s
/// per side (the 3 s neighborhood of the 0.4 s measured span) is what makes
/// short spans separate at all.
pub const SEPARATION_CONTEXT_SECS: f64 = 1.3;

pub struct ConvTasNetSeparator {
    session: Mutex<ort::session::Session>,
    sample_rate: u32,
}

impl std::fmt::Debug for ConvTasNetSeparator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConvTasNetSeparator")
            .field("sample_rate", &self.sample_rate)
            .finish()
    }
}

impl ConvTasNetSeparator {
    /// Build from an ONNX file. Errors on a missing or unloadable model —
    /// the caller degrades to mixture-only; this never fails a run.
    pub fn new(model_path: &str) -> Result<Self> {
        let path = std::path::PathBuf::from(model_path);
        if !path.exists() {
            return Err(anyhow!("separation model not found: {}", model_path));
        }
        let providers = vec![ort::execution_providers::CPUExecutionProvider::default().build()];
        let session = ort::session::Session::builder()
            .map_err(|e| anyhow!("session builder: {e}"))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("opt level: {e}"))?
            .with_execution_providers(providers)
            .map_err(|e| anyhow!("providers: {e}"))?
            .with_intra_threads(2)
            .map_err(|e| anyhow!("intra threads: {e}"))?
            .commit_from_file(&path)
            .map_err(|e| anyhow!("commit separation session: {e}"))?;
        let input = session
            .inputs
            .first()
            .ok_or_else(|| anyhow!("separation model has no inputs"))?;
        if input.name != "mix" {
            return Err(anyhow!(
                "unexpected separation input name {:?} (expected \"mix\")",
                input.name
            ));
        }
        Ok(Self {
            session: Mutex::new(session),
            sample_rate: 16_000,
        })
    }

    /// The registered model from the local models dir, or None when it is
    /// absent — the silent-degrade construction path.
    pub fn from_models_dir() -> Option<Self> {
        let path = crate::audio::speaker::model_download::separation_model_path();
        match Self::new(path.to_str().expect("models path utf-8")) {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!("separation pre-pass unavailable: {e}");
                None
            }
        }
    }
}

impl VoiceSeparationPort for ConvTasNetSeparator {
    fn separate(&self, samples: &[f32], span_secs: (f64, f64)) -> Result<Vec<SeparatedStream>> {
        let sr = self.sample_rate as f64;
        // Context-padded clip, clamped to the recording.
        let c0 = ((span_secs.0 - SEPARATION_CONTEXT_SECS).max(0.0) * sr) as usize;
        let c1 = (((span_secs.1 + SEPARATION_CONTEXT_SECS) * sr) as usize).min(samples.len());
        let s0 = (span_secs.0 * sr) as usize;
        let s1 = ((span_secs.1 * sr) as usize).min(samples.len());
        if c1 <= c0 || s1 <= s0 || s0 < c0 || s1 > c1 {
            return Err(anyhow!(
                "span [{:.2}, {:.2}] not inside the padded clip [{:.2}, {:.2}]",
                span_secs.0,
                span_secs.1,
                c0 as f64 / sr,
                c1 as f64 / sr
            ));
        }
        let clip = &samples[c0..c1];
        let clip_rms = {
            let sum: f32 = clip.iter().map(|v| v * v).sum();
            (sum / clip.len() as f32).sqrt()
        };

        let input_2d: ndarray::Array2<f32> = ndarray::Array1::from(clip.to_vec())
            .into_shape_with_order([1, clip.len()])
            .map_err(|e| anyhow!("input shape: {e}"))?;
        let t = ort::value::TensorRef::from_array_view(input_2d.view())
            .map_err(|e| anyhow!("tensor: {e}"))?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow!("separation session lock poisoned"))?;
        let outputs = session
            .run(ort::inputs!["mix" => t])
            .map_err(|e| anyhow!("separation forward: {e}"))?;
        let out = outputs
            .get("est_source")
            .ok_or_else(|| anyhow!("output est_source missing"))?;
        let arr = out
            .try_extract_array::<f32>()
            .map_err(|e| anyhow!("extract: {e}"))?;
        let shape = arr.shape();
        if shape[1] != 2 {
            return Err(anyhow!("expected a 2-source model, got {}", shape[1]));
        }
        let slice: &[f32] = arr.as_slice().unwrap_or_else(|| arr.to_slice().unwrap());

        // Carve the span out of each stream's context-padded timeline and
        // normalize to the input clip's RMS (the export's gain is an
        // artifact, not voice information).
        let carve0 = s0 - c0;
        let carve1 = carve0 + (s1 - s0);
        let per_src = shape[2];
        let avail = per_src.min(c1 - c0);
        if carve1 > avail {
            return Err(anyhow!(
                "model returned {} samples, too short to carve the span",
                avail
            ));
        }
        let mut streams = Vec::with_capacity(2);
        for s in 0..2 {
            let mut span_samples = slice[s * per_src + carve0..s * per_src + carve1].to_vec();
            let r = {
                let sum: f32 = span_samples.iter().map(|v| v * v).sum();
                (sum / span_samples.len().max(1) as f32).sqrt()
            };
            if r > 1e-6 {
                let g = clip_rms / r;
                for v in span_samples.iter_mut() {
                    *v *= g;
                }
            }
            streams.push(SeparatedStream { samples: span_samples });
        }
        Ok(streams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_is_an_error_not_a_panic() {
        let err = ConvTasNetSeparator::new("Z:/definitely/not/here.onnx")
            .expect_err("missing model must be an Err");
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    #[ignore = "live: needs the exported model in ~/.meetily-models"]
    fn separate_returns_two_span_length_rms_normalized_streams() {
        if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
            return;
        }
        let sep = ConvTasNetSeparator::from_models_dir().expect("model present");
        let samples: Vec<f32> = (0..16000 * 5)
            .map(|i| 0.1 * ((i as f32) * 0.01).sin())
            .collect();
        let streams = sep.separate(&samples, (1.0, 1.5)).expect("separate");
        assert_eq!(streams.len(), 2);
        let want = (0.5 * 16_000.0) as usize;
        for s in &streams {
            assert_eq!(s.samples.len(), want, "stream must cover exactly the span");
            let sum: f32 = s.samples.iter().map(|v| v * v).sum();
            let rms = (sum / s.samples.len() as f32).sqrt();
            // normalized to the input clip's RMS (~0.07 for this tone)
            assert!(
                rms > 0.01 && rms < 0.5,
                "stream must be RMS-normalized to input scale, got {rms}"
            );
        }
    }
}
