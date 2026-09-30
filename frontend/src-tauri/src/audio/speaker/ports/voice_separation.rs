use anyhow::Result;

/// One recovered voice from a separated span. Samples cover exactly the
/// requested span at the caller's sample rate. Stream order carries no
/// meaning — the vote assembly anchors each stream independently against
/// the enrolled references. `pre_rms_ratio` is the stream's span RMS
/// BEFORE the adapter's normalization, relative to the mixture clip's RMS:
/// the only honest silence signal, because normalization amplifies
/// near-silence to clip level (a residue-only stream is otherwise
/// indistinguishable from a voice downstream).
#[derive(Clone, Debug)]
pub struct SeparatedStream {
    pub samples: Vec<f32>,
    pub pre_rms_ratio: f32,
}

/// Source-separation boundary (hexagonal): the engine asks for the voices
/// inside an attested overlap span; adapters (Conv-TasNet ONNX, Phase 2)
/// implement it. The engine never knows whether a model exists — a missing
/// or failed adapter degrades to mixture-only behavior at the call site,
/// never a run failure.
pub trait VoiceSeparationPort: Send + Sync {
    fn separate(&self, samples: &[f32], span_secs: (f64, f64)) -> Result<Vec<SeparatedStream>>;
}
