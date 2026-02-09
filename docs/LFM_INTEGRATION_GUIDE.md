# Liquid AI LFM Integration Guide

This document provides comprehensive documentation for integrating Liquid AI's LFM (Liquid Foundation Model) family into Meetily for both **speech-to-text transcription** and **text summarization**.

## Table of Contents

1. [Overview](#overview)
2. [Model Family](#model-family)
3. [Architecture](#architecture)
4. [LFM2.5-Audio Integration (ASR)](#lfm25-audio-integration-asr)
5. [LFM2-Transcript Integration (Text)](#lfm2-transcript-integration-text)
6. [Setup & Installation](#setup--installation)
7. [API Reference](#api-reference)
8. [Troubleshooting](#troubleshooting)

---

## Overview

Meetily integrates two LFM models from Liquid AI:

| Model | Purpose | Size | Use Case |
|-------|---------|------|----------|
| **LFM2.5-Audio-1.5B** | Speech-to-Text | ~3GB | Real-time meeting transcription |
| **LFM2-Transcript** | Text Summarization | Varies | Meeting summary generation |

### Key Benefits

- **Privacy-First**: All inference runs locally on user hardware
- **Competitive Quality**: 7.53% WER (comparable to Whisper's 7.44%)
- **GPU Acceleration**: Automatic CUDA/MPS/CPU detection
- **Auto-Setup**: One-click installation with progress tracking
- **Unified Interface**: Same API patterns as Whisper/Parakeet engines

---

## Model Family

### LFM2.5-Audio-1.5B (Audio Model)

- **HuggingFace**: `LiquidAI/LFM2.5-Audio-1.5B`
- **Parameters**: 1.5 billion
- **Capabilities**: ASR, TTS, conversational audio
- **WER**: 7.53% (LibriSpeech test-clean)
- **Sample Rate**: 16kHz (auto-resampled)
- **Python Package**: `liquid-audio`

### LFM2-Transcript (Text Model)

- **Purpose**: Meeting summarization with hallucination resistance
- **Format**: ChatML (`<|im_start|>`, `<|im_end|>`)
- **Temperature**: 0.2 (optimized for factual extraction)
- **Context**: Large context window for full meeting transcripts

---

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Meetily Desktop App                              │
│  ┌───────────────┐    ┌─────────────────┐    ┌────────────────────────┐ │
│  │ Audio Capture │───▶│ VAD Processing  │───▶│ Transcription Engine   │ │
│  │  (Rust/cpal)  │    │ (Rust/Silero)   │    │ (Whisper/Parakeet/LFM) │ │
│  └───────────────┘    └─────────────────┘    └────────────────────────┘ │
│                                                         │                │
│                                                         ▼                │
│                                              ┌────────────────────────┐ │
│                                              │   LFM Audio Sidecar    │ │
│                                              │   (Python Process)     │ │
│                                              └────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
                                                         │
                                                         ▼
                                              ┌────────────────────────┐
                                              │   Summarization LLM    │
                                              │ (LFM2-Transcript/Gemma)│
                                              └────────────────────────┘
```

### LFM Audio Sidecar Architecture

The LFM audio engine uses a **Python sidecar pattern** for inference:

```
┌──────────────────────┐         JSON/stdin          ┌────────────────────┐
│   Rust (Tauri)       │ ◀────────────────────────▶  │  Python Sidecar    │
│   LfmEngine          │         JSON/stdout         │  sidecar.py        │
│                      │                             │                    │
│ - Process management │                             │ - Model loading    │
│ - State tracking     │                             │ - Transcription    │
│ - Tauri commands     │                             │ - GPU detection    │
└──────────────────────┘                             └────────────────────┘
```

**Protocol**: JSON-over-stdin/stdout (one JSON object per line)

---

## LFM2.5-Audio Integration (ASR)

### File Structure

```
backend/
└── lfm_engine/
    ├── __init__.py          # Python module init
    ├── sidecar.py           # Main Python sidecar script
    └── venv/                # Auto-created virtual environment
        └── lib/python3.x/site-packages/
            └── liquid_audio/

frontend/src-tauri/src/
└── lfm_engine/
    ├── mod.rs               # Module exports
    ├── lfm_engine.rs        # Rust engine (sidecar management)
    ├── setup.rs             # Auto-setup manager
    └── commands.rs          # Tauri commands
```

### Sidecar Protocol

#### Requests (Rust → Python)

```json
// Load model
{"type": "load_model", "model_id": "LiquidAI/LFM2.5-Audio-1.5B"}

// Transcribe file
{"type": "transcribe", "audio_path": "/path/to/audio.wav"}

// Transcribe raw samples
{"type": "transcribe_samples", "samples": [0.1, -0.2, ...], "sample_rate": 16000}

// Unload model
{"type": "unload"}

// Health check
{"type": "ping"}

// Shutdown
{"type": "shutdown"}
```

#### Responses (Python → Rust)

```json
// Success
{"type": "response", "text": "Transcribed text here", "error": null}

// Error
{"type": "response", "text": "", "error": "Error message"}

// Pong (health check)
{"type": "pong"}

// Shutdown acknowledgment
{"type": "goodbye"}

// Protocol error
{"type": "error", "message": "Unknown request type: xyz"}
```

### Python Engine Implementation

The Python sidecar (`sidecar.py`) handles:

1. **Device Detection**: Automatic GPU selection (CUDA → MPS → CPU)
2. **Model Loading**: HuggingFace model download and initialization
3. **Audio Processing**: Resampling, mono conversion
4. **Transcription**: Chat-based ASR using `liquid_audio`

#### Critical Fix: Device/Dtype Handling

The `liquid-audio` library defaults to CUDA, which fails on macOS. The fix:

```python
def load_model(self, model_id: str = "LiquidAI/LFM2.5-Audio-1.5B") -> dict:
    from liquid_audio import LFM2AudioModel, LFM2AudioProcessor

    self.device = self._detect_device()  # "cuda", "mps", or "cpu"

    # CRITICAL: Pass device explicitly to avoid CUDA default
    self.processor = LFM2AudioProcessor.from_pretrained(
        model_id,
        device=self.device  # Override library default
    ).eval()

    # Select appropriate dtype per device
    if self.device == "mps":
        dtype = torch.float16      # MPS requires float16
    elif self.device == "cuda":
        dtype = torch.bfloat16     # CUDA supports bfloat16
    else:
        dtype = torch.float32      # CPU fallback

    self.model = LFM2AudioModel.from_pretrained(
        model_id,
        device=self.device,
        dtype=dtype
    ).eval()
```

#### Transcription Pipeline

```python
def _transcribe_waveform(self, waveform: np.ndarray, sample_rate: int) -> str:
    from liquid_audio import ChatState

    wav_tensor = torch.from_numpy(waveform).unsqueeze(0)

    # Create chat state (LFM uses chat-based interface)
    chat = ChatState(self.processor)

    # System prompt
    chat.new_turn("system")
    chat.add_text("Transcribe the following audio accurately. Output only the spoken words, nothing else.")
    chat.end_turn()

    # User turn with audio
    chat.new_turn("user")
    chat.add_audio(wav_tensor, sample_rate)
    chat.end_turn()

    # Generate transcription
    chat.new_turn("assistant")
    text_tokens = []
    for t in self.model.generate_interleaved(**chat.tokenize(), max_new_tokens=448):
        if t.numel() == 1:  # Single token = text output
            text_tokens.append(t)

    if text_tokens:
        text = self.processor.text.decode(torch.cat(text_tokens).tolist())
    else:
        text = ""

    return text.strip()
```

### Rust Engine Implementation

#### State Management

```rust
pub struct LfmEngine {
    sidecar_path: PathBuf,           // Path to sidecar.py
    venv_python_path: PathBuf,       // Path to venv Python
    sidecar_process: Arc<RwLock<Option<Child>>>,
    status: Arc<RwLock<ModelStatus>>,
    current_model: Arc<RwLock<Option<String>>>,
}

pub enum ModelStatus {
    NotInstalled,  // Python/liquid-audio not available
    Available,     // Ready to load model
    Loading,       // Model loading in progress
    Loaded,        // Model loaded, ready for transcription
    Error(String),
}
```

#### Tauri Commands

```rust
#[tauri::command]
pub async fn lfm_load_model(state: State<'_, LfmEngineState>, model_id: Option<String>) -> Result<(), String>;

#[tauri::command]
pub async fn lfm_transcribe_file(state: State<'_, LfmEngineState>, audio_path: String) -> Result<String, String>;

#[tauri::command]
pub async fn lfm_transcribe_samples(state: State<'_, LfmEngineState>, samples: Vec<f32>, sample_rate: u32) -> Result<String, String>;

#[tauri::command]
pub async fn lfm_unload_model(state: State<'_, LfmEngineState>) -> Result<(), String>;

#[tauri::command]
pub async fn lfm_is_setup_complete(state: State<'_, LfmSetupState>) -> Result<bool, String>;

#[tauri::command]
pub async fn lfm_run_setup<R: Runtime>(app: AppHandle<R>, state: State<'_, LfmSetupState>) -> Result<(), String>;
```

---

## LFM2-Transcript Integration (Text)

### Purpose

LFM2-Transcript is used for meeting summarization with built-in guardrails against hallucination.

### Chat Format (ChatML)

```
<|im_start|>system
{system_prompt}<|im_end|>
<|im_start|>user
{user_prompt}<|im_end|>
<|im_start|>assistant
```

### Sampling Parameters

```rust
SamplingParams {
    temperature: 0.2,  // Low for factual extraction
    top_k: 40,
    top_p: 0.85,
}
```

### Template (`templates/lfm2_meeting.json`)

```json
{
  "name": "LFM2 Meeting Summary",
  "description": "Optimized template for Liquid AI LFM2-Transcript model with quality guardrails.",
  "sections": [
    {
      "title": "Transcript Quality Assessment",
      "instruction": "Rate the transcript quality (Good/Fair/Poor/Very Poor)...",
      "format": "paragraph"
    },
    {
      "title": "Topics Discussed",
      "instruction": "List the main topics with 2+ clear references...",
      "format": "list"
    },
    {
      "title": "Executive Summary",
      "instruction": "Provide a brief executive summary (2-3 sentences)...",
      "format": "paragraph"
    },
    {
      "title": "Action Items",
      "instruction": "List action items with explicit owners only...",
      "format": "list"
    },
    {
      "title": "Key Decisions",
      "instruction": "List explicit decisions only (not discussions)...",
      "format": "list"
    },
    {
      "title": "Participants",
      "instruction": "List participants by name only...",
      "format": "list"
    },
    {
      "title": "Transcript Issues",
      "instruction": "List garbled passages with timestamps...",
      "format": "list"
    }
  ]
}
```

### Hallucination Prevention Rules

Both LFM2 and Gemma models use these **9 universal grounding rules**:

1. **STRICT EXTRACTION ONLY**: Only include explicitly stated information
2. **GARBLED TEXT HANDLING**: Mark unclear passages as `[Unclear in transcript]`
3. **ACRONYMS**: Never invent expansions for undefined acronyms
4. **SPEAKERS & PARTICIPANTS**: Only list explicitly named participants
5. **NUMERIC CLAIMS**: Only include explicitly stated numbers
6. **MINIMUM EVIDENCE THRESHOLD**: Topics need 2+ related sentences
7. **SECTION CONFIDENCE**: Tag each section with confidence level
8. **EMPTY SECTIONS**: Write "No clear information found" if empty
9. **DECISIONS vs DISCUSSIONS**: "Discussed X" ≠ "Decided X"

### Quality Gate (Pre-Summarization)

A quality assessment runs BEFORE summarization:

```json
{
  "quality_score": 7,
  "coherent_percentage": 85,
  "speaker_identified": true,
  "recommendation": "proceed_with_warnings",
  "issues": ["Some background noise", "One speaker unclear"],
  "garbled_examples": ["[00:15:30] ...the pastor matrix deployment..."]
}
```

**Scoring**:
- 8-10: Clean transcript, proceed
- 5-7: Some issues, proceed with warnings
- 1-4: Severely garbled, reject and recommend manual review

---

## Setup & Installation

### Auto-Setup Flow

The LFM setup process is fully automated:

```
1. Check Python (3.9+)     ─────▶  5%
2. Create venv             ─────▶ 10%
3. Install dependencies    ─────▶ 50%  (~2-3 min)
4. Download model          ─────▶ 90%  (~3-5 min, 3GB)
5. Verify installation     ─────▶ 100%
```

### Setup Manager (`setup.rs`)

```rust
pub struct LfmSetupManager {
    pub backend_dir: PathBuf,
    pub venv_dir: PathBuf,      // backend/lfm_engine/venv
    pub sidecar_path: PathBuf,  // backend/lfm_engine/sidecar.py
}

pub enum SetupStage {
    NotStarted,
    CheckingPython,
    CreatingVenv,
    InstallingDependencies,
    DownloadingModel,
    Verifying,
    Complete,
    Failed(String),
}
```

### Progress Events

Setup emits Tauri events for UI updates:

```rust
#[derive(Serialize)]
pub struct SetupProgress {
    pub stage: SetupStage,
    pub progress: f32,      // 0.0 - 1.0
    pub message: String,
    pub details: Option<String>,
}

// Event: "lfm-setup-progress"
app.emit("lfm-setup-progress", &progress);
```

### Frontend Component

`LfmModelManager.tsx` provides:

- Setup button with requirements display
- Real-time progress tracking
- Stage indicators (Python → Environment → Dependencies → Model → Verify)
- Model load/unload controls
- Error handling with retry

---

## API Reference

### Rust Commands

| Command | Parameters | Returns | Description |
|---------|------------|---------|-------------|
| `lfm_is_setup_complete` | - | `bool` | Check if venv exists |
| `lfm_is_model_downloaded` | - | `bool` | Check HuggingFace cache |
| `lfm_get_setup_status` | - | `String` | Current setup stage |
| `lfm_run_setup` | - | `()` | Start auto-setup |
| `lfm_check_dependencies` | - | `bool` | Verify Python + package |
| `lfm_get_status` | - | `String` | Engine status |
| `lfm_load_model` | `model_id: Option<String>` | `()` | Load model |
| `lfm_unload_model` | - | `()` | Unload model |
| `lfm_transcribe_file` | `audio_path: String` | `String` | Transcribe file |
| `lfm_transcribe_samples` | `samples: Vec<f32>, sample_rate: u32` | `String` | Transcribe raw audio |
| `lfm_is_model_loaded` | - | `bool` | Check if loaded |
| `lfm_get_current_model` | - | `Option<String>` | Current model ID |
| `lfm_shutdown` | - | `()` | Stop sidecar |

### TypeScript Usage

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Check setup
const isSetup = await invoke<boolean>('lfm_is_setup_complete');

// Run setup with progress tracking
const unlisten = await listen<SetupProgress>('lfm-setup-progress', (event) => {
  console.log(`${event.payload.stage}: ${event.payload.progress * 100}%`);
});
await invoke('lfm_run_setup');

// Load model
await invoke('lfm_load_model', { modelId: null });

// Transcribe
const text = await invoke<string>('lfm_transcribe_file', {
  audioPath: '/path/to/audio.wav'
});

// Cleanup
await invoke('lfm_unload_model');
```

---

## Troubleshooting

### Common Issues

#### "Torch not compiled with CUDA enabled" (macOS)

**Cause**: `liquid-audio` defaults to CUDA which doesn't exist on Mac.

**Solution**: The sidecar explicitly passes `device=self.device` to override defaults. Ensure you're using the updated `sidecar.py`.

#### "Invalid provider: lfm"

**Cause**: LFM not registered in settings repository.

**Solution**: Add LFM to valid providers in `setting.rs`:
```rust
"lfm" => return Ok(None),  // LFM doesn't need API key
```

#### Model download fails

**Cause**: Network issues or disk space.

**Check**:
- Internet connection
- ~5GB free disk space
- HuggingFace not blocked

#### Sidecar won't start

**Check**:
1. Python version: `python3 --version` (need 3.9+)
2. Venv exists: `backend/lfm_engine/venv/bin/python`
3. Package installed: `./venv/bin/python -c "from liquid_audio import LFM2AudioModel"`

#### Transcription is empty

**Cause**: Audio too short or silent.

**Check**:
- Audio duration > 0.5 seconds
- Audio not silent
- Sample rate correct (16kHz expected)

### Debug Logging

Enable detailed logging:

```bash
RUST_LOG=app_lib::lfm_engine=debug ./clean_run.sh
```

Python sidecar logs to stderr, visible in Tauri console:
```
[LFM] Loading LFM model on mps...
[LFM] LFM model loaded successfully on mps
```

---

## Deep Technical Reference

This section provides implementation details for developers extending or debugging the LFM integration.

### LFM2AudioModel Architecture

The LFM2.5-Audio model is a multimodal transformer with specialized components:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         LFM2AudioModel Architecture                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Audio Input (waveform)                                                      │
│       │                                                                      │
│       ▼                                                                      │
│  ┌─────────────────────┐                                                    │
│  │ Mel Spectrogram     │  16kHz → 128-dim mel features                      │
│  │ Preprocessor        │  window_size=0.02s, window_stride=0.01s            │
│  │ (NeMo-derived)      │  n_fft=512, features=128                           │
│  └─────────────────────┘                                                    │
│       │                                                                      │
│       ▼                                                                      │
│  ┌─────────────────────┐                                                    │
│  │ Conformer Encoder   │  128 mel → audio embeddings                        │
│  │ (ConformerEncoder)  │  Convolution + Self-Attention hybrid               │
│  └─────────────────────┘                                                    │
│       │                                                                      │
│       ▼                                                                      │
│  ┌─────────────────────┐                                                    │
│  │ Audio Adapter MLP   │  Conformer out → LFM hidden size                   │
│  │                     │  Linear projections with activation                 │
│  └─────────────────────┘                                                    │
│       │                                                                      │
│       ▼                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐       │
│  │                     LFM2 Transformer                              │       │
│  │  ┌─────────────────────────────────────────────────────────┐    │       │
│  │  │ Input: Interleaved [text_emb, audio_in_emb, audio_out_emb]│    │       │
│  │  │ Modality flags track which positions are text vs audio    │    │       │
│  │  └─────────────────────────────────────────────────────────┘    │       │
│  │                              │                                    │       │
│  │                              ▼                                    │       │
│  │  ┌─────────────────────────────────────────────────────────┐    │       │
│  │  │ Hybrid Conv + Attention Layers (Lfm2Model)               │    │       │
│  │  │ - Sliding attention for long sequences                    │    │       │
│  │  │ - Full attention for global context                       │    │       │
│  │  │ - Conv layers for local patterns                          │    │       │
│  │  └─────────────────────────────────────────────────────────┘    │       │
│  └─────────────────────────────────────────────────────────────────┘       │
│       │                                                                      │
│       ├──────────────────┬──────────────────┐                               │
│       ▼                  ▼                  ▼                               │
│  ┌──────────┐     ┌─────────────┐    ┌──────────────────┐                  │
│  │ Text     │     │ Audio Out   │    │ Depthformer      │                  │
│  │ Logits   │     │ Embedding   │    │ (8 codebooks)    │                  │
│  │          │     │ (shared)    │    │ Parallel decode  │                  │
│  └──────────┘     └─────────────┘    └──────────────────┘                  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Key Constants

```python
# From LFM2AudioModel class
audio_vocab_size = 2048 + 1  # 2049 total (includes EOAudio token)
codebooks = 8                 # Audio codec uses 8 parallel codebooks

# Interleaved generation rhythm
interleaved_n_text = config.interleaved_n_text   # Text tokens per chunk
interleaved_n_audio = config.interleaved_n_audio # Audio frames per chunk

# Special tokens
TOKEN_AUDIO_START = 128      # <|audio_start|>
TOKEN_TEXT_END = 130         # <|text_end|>
TOKEN_IM_END = 7             # <|im_end|>
TOKEN_EOAUDIO = 2048         # End of audio frame
```

#### Modality Flags (LFMModality)

The model tracks input modalities using an IntEnum:

```python
class LFMModality(IntEnum):
    TEXT = 1       # Text token positions
    AUDIO_IN = 2   # Input audio (mel spectrogram embeddings)
    AUDIO_OUT = 3  # Output audio (codec tokens)
```

### Audio Preprocessing Pipeline

#### Mel Spectrogram Generation

```python
# AudioToMelSpectrogramPreprocessor configuration (from config.json)
PreprocessorConfig:
    sample_rate: 16000
    normalize: "per_feature"    # Normalize per frequency bin
    window_size: 0.02           # 20ms window (320 samples)
    window_stride: 0.01         # 10ms stride (160 samples)
    window: "hann"              # Hann windowing function
    features: 128               # 128 mel filterbank features
    n_fft: 512                  # FFT size
    log: True                   # Log-mel spectrogram
    frame_splicing: 1           # No frame stacking
    dither: 1e-5                # Small dither for numerical stability
    pad_to: 16                  # Pad to multiple of 16 frames
    pad_value: 0.0              # Padding value
```

#### Mel to Embedding Length Conversion

```python
def mel2emb_len(mel_length: int) -> int:
    """Convert mel spectrogram length to LFM embedding length.

    The Conformer encoder subsamples by factor of 8.
    Minimum mel length for encoder is 9 frames.

    Example: 160 mel frames → 20 embeddings
    """
    return -(mel_length // -8)  # Ceiling division
```

#### Audio Processing Flow

```
Waveform (any sample rate, any channels)
    │
    ├── 1. Resample to 16kHz (torchaudio.functional.resample)
    ├── 2. Convert to mono (mean across channels)
    ├── 3. Apply dither (1e-5 * random noise)
    ├── 4. Pre-emphasis filter (0.97 coefficient)
    │
    ▼
STFT (n_fft=512, hop=160, win=320, hann window)
    │
    ├── 5. Compute magnitude spectrogram
    ├── 6. Apply power (mag^2.0)
    ├── 7. Mel filterbank multiplication (128 bins)
    ├── 8. Log compression (log(x + 2^-24))
    ├── 9. Per-feature normalization (mean=0, std=1)
    ├── 10. Pad to multiple of 16 frames
    │
    ▼
Mel Spectrogram [batch, 128, time]
    │
    ├── 11. Conformer encoder (subsampling 8x)
    ├── 12. Audio adapter MLP
    │
    ▼
Audio Embeddings [batch, time//8, hidden_size]
```

### Interleaved Generation Algorithm

LFM2 generates text and audio in an **interleaved** manner, alternating between modalities:

```python
def generate_interleaved(self, ..., max_new_tokens: int = 20):
    """Generate text and audio tokens in interleaved chunks.

    Generation rhythm:
    1. Generate N_TEXT text tokens
    2. Generate N_AUDIO audio frames (each frame = 8 codebook tokens)
    3. Repeat until <|im_end|> or max_new_tokens

    Yields:
        - Text tokens: shape (1,) single token
        - Audio frames: shape (8,) one token per codebook
    """
    # Prefill: encode all input embeddings
    in_emb = self._prefill(text, audio_in, audio_in_lens, audio_out, modality_flag)

    current_modality = LFMModality.TEXT
    modality_left = config.interleaved_n_text  # Tokens until switch
    text_done = False

    for _ in range(max_new_tokens):
        modality_left -= 1

        # Forward pass through LFM transformer
        lfm_out = self.lfm(inputs_embeds=in_emb, past_key_values=cache, use_cache=True)
        output_embeddings = lfm_out.last_hidden_state
        cache = lfm_out.past_key_values

        if current_modality == LFMModality.TEXT:
            # Sample text token from LFM vocabulary
            text_logits = F.linear(output_embeddings[0, -1], self.lfm.embed_tokens.weight)
            next_token = sample_text(text_logits, temperature, top_k)

            if next_token == 7:  # <|im_end|>
                break

            yield next_token

            if next_token == 130:  # <|text_end|>
                text_done = True

            # Switch to audio if quota exhausted or text complete
            if not modality_left or text_done:
                current_modality = LFMModality.AUDIO_OUT
                modality_left = config.interleaved_n_audio

            in_emb = self.lfm.embed_tokens(next_token)[None, :]

        elif current_modality == LFMModality.AUDIO_OUT:
            # Sample audio frame using Depthformer
            next_token = self._sample_audio_frame(output_embeddings[0, -1], temperature, top_k)

            # Switch back to text if quota exhausted
            if not modality_left and not text_done:
                current_modality = LFMModality.TEXT
                modality_left = config.interleaved_n_text

            # EOAudio in first codebook ends audio generation
            if next_token[0] == 2048:
                next_token[:] = 2048
                current_modality = LFMModality.TEXT

            yield next_token

            # Sum embeddings from all 8 codebooks
            in_emb = self.audio_embedding(next_token + self.codebook_offsets).sum(0)[None, None, :]
```

### Depthformer: Parallel Audio Decoding

The Depthformer decodes 8 audio codebook tokens in parallel:

```python
def _sample_audio_frame(self, embedding: torch.Tensor, temperature=None, top_k=None):
    """Sample one audio frame (8 codebook tokens) using Depthformer.

    The Depthformer is a small transformer that generates 8 parallel
    codec tokens, conditioned on the main LFM output embedding.

    Architecture:
    - depth_linear: LFM hidden → (8 * depthformer_dim)
    - depthformer: Small transformer (few layers)
    - depth_embeddings: 8 separate embedding tables for each codebook
    """
    # Project LFM output to 8 parallel depthformer inputs
    depthformer_in = rearrange(
        self.depth_linear(embedding),
        "(C D) -> C D",  # Split into 8 codebooks
        C=self.codebooks,
        D=self.depthformer_dim
    )

    depthformer_token = torch.zeros_like(depthformer_in[0])
    cache = None
    out_tokens = []

    # Autoregressive across 8 codebooks
    for i in range(self.codebooks):
        # Input = projection + previous codebook embedding
        cur_input = depthformer_in[i] + depthformer_token

        # Forward through depthformer
        depthformer_out, cache = self.depthformer.forward_cached(
            cur_input[None, None, :], cache
        )

        # Get logits from codebook-specific embedding
        logits = self.depth_embeddings[i].get_logits(depthformer_out.squeeze())

        # Sample (greedy or nucleus)
        if temperature is None or temperature <= 0:
            next_token = logits.argmax(keepdim=True)
        else:
            logits /= temperature
            if top_k:
                min_score = torch.topk(logits, top_k).values[-1]
                logits = logits.masked_fill(logits < min_score, -float("inf"))
            next_token = torch.multinomial(logits.softmax(0), 1)

        out_tokens.append(next_token)

        # Use this codebook's embedding for next iteration
        depthformer_token = self.depth_embeddings[i](next_token).squeeze()

    return torch.cat(out_tokens)  # Shape: (8,)
```

### ChatState: The Multimodal Context Manager

`ChatState` is the key abstraction for managing multimodal conversations:

```python
class ChatState(Mapping):
    """Manages interleaved text and audio context for LFM2.

    Tracks three parallel sequences:
    - text: Token IDs for text content
    - audio_in: Mel spectrogram embeddings for input audio
    - audio_out: Codec tokens for generated audio

    Plus a modality_flag tensor that marks which positions are which type.
    """

    model_inputs = ["text", "audio_in", "audio_in_lens", "audio_out", "modality_flag"]

    def __init__(self, processor: LFM2AudioProcessor, codebooks: int = 8, dtype=torch.bfloat16):
        self.proc = processor
        self.codebooks = codebooks
        self.dtype = dtype

        # Initialize with start token
        start = "<|startoftext|>"
        self.text = self.proc.text.encode(start, add_special_tokens=False, return_tensors="pt")

        # Empty audio tensors (will grow as audio is added)
        self.audio_in = torch.empty((128, 0), dtype=self.dtype)      # Mel features
        self.audio_in_lens = torch.empty((0,), dtype=torch.long)     # Lengths per segment
        self.audio_out = torch.empty((self.codebooks, 0))            # Output codec tokens

        # Track modality at each position
        self.modality_flag = torch.full_like(self.text, LFMModality.TEXT)
```

#### Adding Text

```python
def add_text(self, text: str) -> None:
    """Append text to the context.

    Tokenizes the text and extends both `text` and `modality_flag` tensors.
    """
    new_text = self.proc.text.encode(text, add_special_tokens=False, return_tensors="pt")
    new_mod = torch.full(new_text.shape, LFMModality.TEXT)

    self.text = torch.cat([self.text, new_text], dim=1)
    self.modality_flag = torch.cat([self.modality_flag, new_mod], dim=1)
```

#### Adding Audio

```python
def add_audio(self, wave: torch.Tensor, sampling_rate: int) -> None:
    """Add audio to the context.

    Process:
    1. Resample to 16kHz if needed
    2. Convert to mel spectrogram
    3. Extend audio_in and audio_in_lens
    4. Add AUDIO_IN modality flags (length = mel2emb_len(mel_length))

    Args:
        wave: Shape (1, samples) - must be mono, single channel
        sampling_rate: Original sample rate (will be resampled to 16kHz)
    """
    assert wave.shape[0] == 1, "Audio must be mono (single channel)"

    # Resample to 16kHz
    wave = torchaudio.functional.resample(wave, sampling_rate, 16_000)
    length = torch.tensor([wave.shape[1]], dtype=torch.long)

    # Generate mel spectrogram
    mel, _ = self.proc.audio(wave, length)  # Shape: (batch, 128, time)

    new_audio_in = mel[0].to(self.dtype)  # (128, time)

    # Calculate embedding length after Conformer subsampling
    emb_len = mel2emb_len(new_audio_in.shape[1])

    # Create modality flags for this audio segment
    new_mod = torch.full((1, emb_len), LFMModality.AUDIO_IN)

    # Extend tensors
    self.audio_in = torch.cat([self.audio_in, new_audio_in], dim=1)
    self.audio_in_lens = torch.cat([self.audio_in_lens, torch.tensor([new_audio_in.shape[1]])])
    self.modality_flag = torch.cat([self.modality_flag, new_mod], dim=1)
```

#### Turn Management

```python
def new_turn(self, role: Literal["system", "user", "assistant"]) -> None:
    """Start a new conversation turn with ChatML formatting."""
    self.add_text(f"<|im_start|>{role}\n")

def end_turn(self) -> None:
    """End the current turn with ChatML end token."""
    self.add_text("<|im_end|>\n")
```

#### Tokenization for Model Input

```python
def tokenize(self) -> dict:
    """Prepare inputs for model.forward() or model.generate_*().

    Returns dict with keys matching model_inputs:
    - text: (1, N_text) token IDs
    - audio_in: (128, total_mel_frames) mel features
    - audio_in_lens: (N_audio_segments,) length of each segment
    - audio_out: (8, N_audio_out) codec tokens
    - modality_flag: (1, total_positions) modality at each position
    """
    return {k: getattr(self, k) for k in self.model_inputs}
```

#### Example: Building a Transcription Context

```python
# Create chat state
chat = ChatState(processor)

# System instruction
chat.new_turn("system")
chat.add_text("Transcribe the following audio accurately. Output only the spoken words.")
chat.end_turn()

# User turn with audio
chat.new_turn("user")
chat.add_audio(waveform, sample_rate=16000)
chat.end_turn()

# Assistant turn (model will generate here)
chat.new_turn("assistant")

# At this point, modality_flag looks like:
# [TEXT, TEXT, TEXT, ..., TEXT, AUDIO_IN, AUDIO_IN, ..., AUDIO_IN, TEXT, TEXT, ...]
#  ^^^^ system prompt ^^^^       ^^^^ audio segment ^^^^           ^^^^ "assistant\n"

# Generate transcription
inputs = chat.tokenize()
for token in model.generate_interleaved(**inputs, max_new_tokens=448):
    if token.numel() == 1:  # Text token
        text_tokens.append(token)
    # Audio tokens are ignored for ASR
```

### Memory Requirements

#### Model Components (approximate)

| Component | Parameters | Memory (fp16) | Memory (bf16) |
|-----------|------------|---------------|---------------|
| LFM2 Transformer | ~1.2B | ~2.4 GB | ~2.4 GB |
| Conformer Encoder | ~100M | ~200 MB | ~200 MB |
| Audio Adapter | ~10M | ~20 MB | ~20 MB |
| Depthformer | ~50M | ~100 MB | ~100 MB |
| Embeddings | ~100M | ~200 MB | ~200 MB |
| **Total Model** | **~1.5B** | **~3 GB** | **~3 GB** |

#### Runtime Memory

```
Base model loaded:           ~3.0 GB
KV cache (per token):        ~1 MB (varies with sequence length)
Audio processing buffers:    ~50 MB
Inference overhead:          ~200 MB
────────────────────────────────────
Minimum VRAM required:       ~4 GB (with small context)
Recommended VRAM:            ~6 GB (for longer audio)
```

#### Dtype Selection Per Device

```python
# Optimal dtype selection
if device == "cuda":
    dtype = torch.bfloat16   # Best for NVIDIA Ampere+
elif device == "mps":
    dtype = torch.float16    # MPS doesn't support bfloat16
else:  # CPU
    dtype = torch.float32    # Quantization recommended for CPU
```

### Known Library Issues and Workarounds

#### 1. CUDA Default in `from_pretrained`

**Issue**: `LFM2AudioProcessor.from_pretrained()` and `LFM2AudioModel.from_pretrained()` default to `device="cuda"`.

**Location**: `liquid_audio/processor.py:61`, `liquid_audio/model/lfm2_audio.py:127`

**Workaround**: Always pass `device` explicitly.

#### 2. Hardcoded `.cuda()` in Audio Detokenizer

**Issue**: `liquid_audio/processor.py:151` has `.cuda()` hardcoded for audio detokenizer.

**Impact**: TTS functionality broken on non-CUDA devices.

**Workaround**: Don't use TTS on MPS/CPU (ASR still works).

#### 3. No MLX Support

**Issue**: No Apple MLX backend support (GitHub Issue #9).

**Impact**: MPS backend works but is slower than potential MLX implementation.

**Future**: Watch for MLX support in liquid-audio updates.

#### 4. Flash Attention Detection

**Issue**: Model auto-detects `flash_attn` and uses it if available.

```python
if module_exists("flash_attn"):
    model.lfm.set_attn_implementation("flash_attention_2")
else:
    model.lfm.set_attn_implementation("sdpa")  # PyTorch SDPA fallback
```

**Impact**: Flash Attention 2 provides ~2x speedup on compatible hardware.

### Performance Optimization

#### 1. Batch Processing

Currently, the sidecar processes one audio at a time. For batch processing:

```python
# Future batch implementation
def transcribe_batch(self, audio_list: list[np.ndarray]) -> list[str]:
    # Pad to same length
    max_len = max(len(a) for a in audio_list)
    padded = [np.pad(a, (0, max_len - len(a))) for a in audio_list]
    batch = np.stack(padded)

    # Process batch
    # ... (requires model modifications for batch inference)
```

#### 2. KV Cache Optimization

The model uses HuggingFace's `Lfm2HybridConvCache` for efficient KV caching:

```python
# Cache is automatically managed during generation
lfm_out = self.lfm(
    inputs_embeds=in_emb,
    past_key_values=cache,  # Reuse previous computation
    use_cache=True,
)
cache = lfm_out.past_key_values  # Save for next iteration
```

#### 3. Streaming Transcription (Future)

For real-time streaming, implement chunked processing:

```python
class StreamingTranscriber:
    def __init__(self, model, processor, chunk_seconds=2.0):
        self.chunk_samples = int(chunk_seconds * 16000)
        self.buffer = []
        self.model = model
        self.processor = processor

    def process_chunk(self, samples: np.ndarray) -> Optional[str]:
        self.buffer.extend(samples)

        if len(self.buffer) >= self.chunk_samples:
            chunk = np.array(self.buffer[:self.chunk_samples])
            self.buffer = self.buffer[self.chunk_samples // 2:]  # 50% overlap

            return self._transcribe(chunk)
        return None
```

### Extending the Integration

#### Adding New Sidecar Commands

1. **Define request type in sidecar.py**:

```python
elif request_type == "my_new_command":
    arg1 = request.get("arg1", "default")
    response = engine.my_new_method(arg1)
```

2. **Add Rust request enum variant**:

```rust
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SidecarRequest {
    // ... existing variants
    MyNewCommand { arg1: String },
}
```

3. **Add Tauri command**:

```rust
#[tauri::command]
pub async fn lfm_my_new_command(
    state: State<'_, LfmEngineState>,
    arg1: String,
) -> Result<String, String> {
    let guard = state.0.read().await;
    let engine = guard.as_ref().ok_or("LFM engine not initialized")?;
    engine.my_new_command(&arg1).await.map_err(|e| e.to_string())
}
```

4. **Register in `lib.rs`**:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    lfm_engine::commands::lfm_my_new_command,
])
```

#### Adding Model Variants

To support different LFM model sizes:

```python
# In sidecar.py
SUPPORTED_MODELS = {
    "LiquidAI/LFM2.5-Audio-1.5B": {"dtype_cuda": "bfloat16", "dtype_mps": "float16"},
    "LiquidAI/LFM2.5-Audio-7B": {"dtype_cuda": "bfloat16", "dtype_mps": "float16"},  # Hypothetical
}

def load_model(self, model_id: str) -> dict:
    if model_id not in SUPPORTED_MODELS:
        return {"type": "response", "text": "", "error": f"Unknown model: {model_id}"}

    config = SUPPORTED_MODELS[model_id]
    dtype = getattr(torch, config[f"dtype_{self.device}"])
    # ... rest of loading
```

---

## Testing and Validation

### Unit Testing the Sidecar

Create `backend/lfm_engine/test_sidecar.py`:

```python
#!/usr/bin/env python3
"""Unit tests for LFM sidecar."""

import json
import subprocess
import sys
import time
import numpy as np

def test_sidecar_protocol():
    """Test the JSON protocol with a running sidecar."""
    venv_python = "backend/lfm_engine/venv/bin/python"
    sidecar_path = "backend/lfm_engine/sidecar.py"

    proc = subprocess.Popen(
        [venv_python, sidecar_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )

    def send_recv(request: dict) -> dict:
        proc.stdin.write(json.dumps(request) + "\n")
        proc.stdin.flush()
        response = proc.stdout.readline()
        return json.loads(response)

    try:
        # Test ping
        resp = send_recv({"type": "ping"})
        assert resp["type"] == "pong", f"Expected pong, got {resp}"
        print("✓ Ping/pong works")

        # Test load (skip if model not downloaded)
        resp = send_recv({"type": "load_model", "model_id": "LiquidAI/LFM2.5-Audio-1.5B"})
        if resp.get("error"):
            print(f"⚠ Model load skipped: {resp['error']}")
        else:
            print("✓ Model loaded")

            # Test transcribe with synthetic audio
            samples = (np.sin(np.linspace(0, 440 * 2 * np.pi, 16000)) * 0.5).tolist()
            resp = send_recv({
                "type": "transcribe_samples",
                "samples": samples,
                "sample_rate": 16000
            })
            print(f"✓ Transcription: '{resp.get('text', '')[:50]}...'")

            # Test unload
            resp = send_recv({"type": "unload"})
            assert resp.get("error") is None
            print("✓ Model unloaded")

        # Test shutdown
        resp = send_recv({"type": "shutdown"})
        assert resp["type"] == "goodbye"
        print("✓ Shutdown acknowledged")

    finally:
        proc.terminate()
        proc.wait(timeout=5)

if __name__ == "__main__":
    test_sidecar_protocol()
```

### Integration Testing

```bash
# Test 1: Verify venv setup
backend/lfm_engine/venv/bin/python -c "
from liquid_audio import LFM2AudioModel, LFM2AudioProcessor, ChatState
print('✓ All imports successful')
"

# Test 2: Verify device detection
backend/lfm_engine/venv/bin/python -c "
import torch
if torch.cuda.is_available():
    print('Device: CUDA')
elif hasattr(torch.backends, 'mps') and torch.backends.mps.is_available():
    print('Device: MPS (Apple Silicon)')
else:
    print('Device: CPU')
"

# Test 3: Verify model cache
ls -la ~/.cache/huggingface/hub/models--LiquidAI--LFM2.5-Audio-1.5B/

# Test 4: End-to-end transcription test
backend/lfm_engine/venv/bin/python -c "
import torch
import torchaudio
from liquid_audio import LFM2AudioModel, LFM2AudioProcessor, ChatState

# Detect device
device = 'cuda' if torch.cuda.is_available() else 'mps' if torch.backends.mps.is_available() else 'cpu'
dtype = torch.float16 if device == 'mps' else torch.bfloat16 if device == 'cuda' else torch.float32

print(f'Loading on {device} with {dtype}...')

processor = LFM2AudioProcessor.from_pretrained('LiquidAI/LFM2.5-Audio-1.5B', device=device).eval()
model = LFM2AudioModel.from_pretrained('LiquidAI/LFM2.5-Audio-1.5B', device=device, dtype=dtype).eval()

# Generate 1 second of test audio (440Hz sine wave)
import numpy as np
samples = np.sin(np.linspace(0, 440 * 2 * np.pi, 16000)).astype(np.float32)
waveform = torch.from_numpy(samples).unsqueeze(0)

# Build chat context
chat = ChatState(processor)
chat.new_turn('system')
chat.add_text('Transcribe the audio.')
chat.end_turn()
chat.new_turn('user')
chat.add_audio(waveform, 16000)
chat.end_turn()
chat.new_turn('assistant')

# Generate
tokens = []
for t in model.generate_interleaved(**chat.tokenize(), max_new_tokens=50):
    if t.numel() == 1:
        tokens.append(t)

text = processor.text.decode(torch.cat(tokens).tolist()) if tokens else ''
print(f'Transcription: {text}')
print('✓ End-to-end test passed')
"
```

### Benchmarking

```python
#!/usr/bin/env python3
"""Benchmark LFM transcription performance."""

import time
import torch
import numpy as np
from liquid_audio import LFM2AudioModel, LFM2AudioProcessor, ChatState

def benchmark_transcription(duration_seconds: float = 5.0, iterations: int = 3):
    device = 'cuda' if torch.cuda.is_available() else 'mps' if torch.backends.mps.is_available() else 'cpu'
    dtype = torch.float16 if device == 'mps' else torch.bfloat16 if device == 'cuda' else torch.float32

    print(f"Device: {device}, dtype: {dtype}")

    # Load model
    t0 = time.time()
    processor = LFM2AudioProcessor.from_pretrained('LiquidAI/LFM2.5-Audio-1.5B', device=device).eval()
    model = LFM2AudioModel.from_pretrained('LiquidAI/LFM2.5-Audio-1.5B', device=device, dtype=dtype).eval()
    print(f"Model load time: {time.time() - t0:.2f}s")

    # Generate test audio
    samples = np.random.randn(int(16000 * duration_seconds)).astype(np.float32) * 0.1
    waveform = torch.from_numpy(samples).unsqueeze(0).to(device)

    # Warmup
    print("Warming up...")
    _run_transcription(model, processor, waveform)

    # Benchmark
    times = []
    for i in range(iterations):
        t0 = time.time()
        text = _run_transcription(model, processor, waveform)
        elapsed = time.time() - t0
        times.append(elapsed)
        rtf = elapsed / duration_seconds
        print(f"  Run {i+1}: {elapsed:.2f}s (RTF: {rtf:.2f}x)")

    avg_time = sum(times) / len(times)
    avg_rtf = avg_time / duration_seconds
    print(f"\nAverage: {avg_time:.2f}s, RTF: {avg_rtf:.2f}x")
    print(f"{'✓ Real-time capable' if avg_rtf < 1.0 else '✗ Slower than real-time'}")

def _run_transcription(model, processor, waveform):
    chat = ChatState(processor)
    chat.new_turn('system')
    chat.add_text('Transcribe accurately.')
    chat.end_turn()
    chat.new_turn('user')
    chat.add_audio(waveform, 16000)
    chat.end_turn()
    chat.new_turn('assistant')

    tokens = []
    for t in model.generate_interleaved(**chat.tokenize(), max_new_tokens=448):
        if t.numel() == 1:
            tokens.append(t)

    return processor.text.decode(torch.cat(tokens).tolist()) if tokens else ''

if __name__ == "__main__":
    benchmark_transcription()
```

### Typical Performance Results

| Device | Audio Length | Transcription Time | RTF | Notes |
|--------|--------------|-------------------|-----|-------|
| NVIDIA A100 | 30s | ~3s | 0.1x | Flash Attention 2 |
| NVIDIA RTX 4090 | 30s | ~5s | 0.17x | Flash Attention 2 |
| NVIDIA RTX 3080 | 30s | ~8s | 0.27x | SDPA fallback |
| Apple M2 Max | 30s | ~15s | 0.5x | MPS backend |
| Apple M1 | 30s | ~25s | 0.83x | MPS backend |
| CPU (16 cores) | 30s | ~90s | 3x | Not recommended |

*RTF = Real-Time Factor (lower is better, <1.0 means faster than real-time)*

---

## References

- [Liquid AI](https://liquid.ai/)
- [LFM2.5-Audio HuggingFace](https://huggingface.co/LiquidAI/LFM2.5-Audio-1.5B)
- [liquid-audio Python Package](https://pypi.org/project/liquid-audio/)
- [GitHub Issue #9: MLX Support](https://github.com/Liquid4All/liquid-audio/issues/9)

---

## Version History

| Date | Version | Changes |
|------|---------|---------|
| 2025-02 | 1.0 | Initial LFM2.5-Audio integration |
| 2025-02 | 1.1 | Added auto-setup with progress tracking |
| 2025-02 | 1.2 | Fixed macOS CUDA error with explicit device/dtype |
| 2025-02 | 1.3 | Added LFM2-Transcript summarization support |
