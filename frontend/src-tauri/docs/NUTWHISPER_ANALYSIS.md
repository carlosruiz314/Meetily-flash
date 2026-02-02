# NutWhisper Implementation Analysis

> Analysis of NutWhisper (Meetily) vs Original Nutshell
> Purpose: Gap analysis and improvement recommendations

## Table of Contents

1. [Overview](#overview)
2. [Original Nutshell Architecture](#original-nutshell-architecture)
3. [Current NutWhisper Implementation](#current-nutwhisper-implementation)
4. [Gap Analysis](#gap-analysis)
5. [Data Flow Analysis](#data-flow-analysis)
6. [Known Issues and Debugging](#known-issues-and-debugging)
7. [Recommendations](#recommendations)

---

## Overview

### What is NutWhisper?

NutWhisper is Meetily's implementation of Nutshell-style streaming transcription:
- **No external VAD** - Uses Whisper's internal no_speech_prob
- **Time-based chunking** - Fixed 2-second chunks instead of VAD-triggered
- **Progressive transcription** - Partial → Refined → Final results
- **Re-transcription** - Accumulates audio for better accuracy

### Files Involved

| File | Purpose |
|------|---------|
| `nut_whisper.rs` | Core NutWhisper/NutWhisperPipeline implementation |
| `commands.rs` | Tauri commands and pipeline integration |
| `worker.rs` | `run_nut_whisper_transcription_loop` polling loop |
| `pipeline.rs` | Audio routing when NutWhisper is active |
| `TranscriptSettings.tsx` | UI for provider selection |

---

## Original Nutshell Architecture

### From Decompiled Code

```
Electron App
     │
     ├── NutshellElectron.node (C++ - CoreAudio)
     │   └── Multi-Output Device (no BlackHole needed!)
     │
     └── WebSocket (port 3005)
             │
             └── Python Server (PyInstaller)
                 ├── mlx_whisper (MLX-optimized)
                 ├── TranscriptionPipeline (RxPY reactive)
                 ├── PhraseAccumulator (Observer)
                 ├── MergeHandler (with diarization)
                 └── diart2/pyannote (speaker diarization)
```

### Key Nutshell Parameters

From `transcribe_handler.py`:
```python
model_name = 'whisper-base.en-mlx'
word_timestamps = True
condition_on_previous_text = False  # CRITICAL - prevents loops!
no_speech_prob_threshold = 0.65
chunk_size = 5  # messages per chunk
```

### Nutshell Data Flow

```
Audio (16-bit PCM, 16kHz)
         ↓
WebSocket Message Queue
         ↓
TranscriptionPipeline
    │
    ├── chunk_size messages → aggregate
    │
    ├── TranscribeHandler (mlx_whisper.transcribe)
    │   ├── word_timestamps=True
    │   └── condition_on_previous_text=False
    │
    ├── process_segments()
    │   ├── handle_repeated_words(min_repetitions=4)
    │   ├── deduplicate_segments()
    │   └── hallucination_filter()
    │
    ├── PhraseAccumulator (Observer)
    │   └── accumulates words into phrases
    │
    ├── MergeHandler (Observer)
    │   └── re-transcribes accumulated audio
    │       └── merges with speaker diarization
    │
    └── Send phrase JSON via WebSocket
```

### Nutshell Hallucination Detection

From `transcribe_handler.py`:
```python
# Pattern-based hallucination filter
hallucination_string = 'alittlebitofalittlebitofalittlebitofalittlebitof'
if hallucination_string in segment['text'].replace(' ', ''):
    segment['text'] = ''
    segment['words'] = []
```

### Nutshell Data Model

From `data_model.py`:
```python
class Word:
    start: float
    end: float
    word: str
    probability: float

class Phrase:
    phrase_id: uuid.UUID
    text: str
    is_completed: bool
    start: float
    end: float
    words: List[Word]
    speaker: str
    label: str
```

---

## Current NutWhisper Implementation

### Architecture

```
Audio Pipeline (Rust)
         │
    NutWhisper Active?
         │
    ├── Yes → Bypass VAD
    │         │
    │    Resample 48kHz → 16kHz
    │         │
    │    Buffer 100ms chunks
    │         │
    │    send_audio_to_nut_whisper()
    │         │
    │    NutWhisperPipeline (internal task)
    │         │
    │    Accumulate 2 seconds
    │         │
    │    process_chunk()
    │         │
    │    result_tx channel
    │         │
    │    poll_nut_whisper_results()
    │         │
    │    Worker loop (100ms polling)
    │         │
    │    app.emit("transcript-update")
    │         │
    └────────────────────────────────→ Frontend
```

### NutWhisperConfig

From `nut_whisper.rs`:
```rust
pub struct NutWhisperConfig {
    pub sample_rate: u32,               // 16000
    pub chunk_duration_secs: f32,       // 2.0 seconds
    pub chunks_before_retranscribe: usize, // 3 (every 6 seconds)
    pub no_speech_threshold: f32,       // 0.65 (from Nutshell)
    pub max_accumulation_secs: f32,     // 30.0 seconds
    pub language: Option<String>,       // "en"
}
```

### Transcription Parameters

From `transcribe_audio_sync`:
```rust
// === NUTSHELL KEY SETTINGS ===
params.set_no_context(true);           // condition_on_previous_text=False
params.set_no_speech_thold(0.65);      // no_speech_prob_threshold
params.set_suppress_blank(true);
params.set_suppress_non_speech_tokens(true);
params.set_single_segment(true);       // For streaming chunks
params.set_token_timestamps(true);     // For word-level timing
params.set_temperature(0.0);           // Greedy decoding
params.set_max_len(100);               // Prevent runaway
```

### TranscriptionResult

```rust
pub struct TranscriptionResult {
    pub text: String,
    pub is_partial: bool,      // Chunk result
    pub is_final: bool,        // Phrase complete
    pub confidence: f32,       // Avg token probability
    pub start_time: f32,
    pub end_time: f32,
    pub words: Vec<WordInfo>,  // Currently empty
    pub source: String,
}
```

### Channel Architecture

```
NutWhisperPipeline::new()
         │
    ┌────┴────┐
    │         │
audio_tx   result_tx
    │         │
    ↓         ↓
audio_rx   result_rx
    │         │
    └────┬────┘
         │
    Internal Task
         │
    process_chunk()
         │
    result_tx.send()
```

---

## Gap Analysis

### What NutWhisper Has ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Time-based chunking | ✅ | 2 second chunks |
| No external VAD | ✅ | Uses no_speech_prob |
| `no_context(true)` | ✅ | Prevents hallucination loops |
| Re-transcription | ✅ | Every 3 chunks (6 seconds) |
| Hallucination filter | ✅ | Pattern + repetition detection |
| Token timestamps | ✅ | Enabled in params |
| Progressive results | ✅ | partial/refined/final |

### What NutWhisper is Missing ❌

| Feature | Original Nutshell | NutWhisper | Impact |
|---------|-------------------|------------|--------|
| **Word timestamps** | Extract per-word timing | `words: Vec::new()` | Can't sync words to audio |
| **Reactive streams** | RxPY Observer pattern | Channels + polling | Different paradigm |
| **Phrase accumulation** | PhraseAccumulator class | Inline logic | Less modular |
| **Speaker diarization** | diart2/pyannote | Not implemented | No speaker labels |
| **MLX optimization** | mlx_whisper | whisper-rs | Slower on Apple Silicon |
| **Prefix prompting** | Uses confirmed words | Not implemented | No EagerMode confirmation |

### Key Missing Feature: Word Timestamps

The current implementation enables token timestamps but **doesn't extract them**:

```rust
// In transcribe_audio_sync
params.set_token_timestamps(true);

// But in the result construction:
Ok(TranscriptionResult {
    // ...
    words: Vec::new(), // TODO: Extract word timestamps
    // ...
})
```

**Fix Required:**
```rust
// Extract word-level timestamps
let mut words = Vec::new();
for seg_idx in 0..num_segments {
    if let Ok(n_tokens) = state.full_n_tokens(seg_idx) {
        for tok_idx in 0..n_tokens {
            if let Ok(token_data) = state.full_get_token_data(seg_idx, tok_idx) {
                if let Ok(token_text) = state.full_get_token_text(seg_idx, tok_idx) {
                    let text = token_text.trim();
                    if !text.is_empty() && !text.starts_with('[') {
                        words.push(WordInfo {
                            word: text.to_string(),
                            start: token_data.t0 as f32 / 100.0, // centiseconds to seconds
                            end: token_data.t1 as f32 / 100.0,
                            probability: token_data.p,
                        });
                    }
                }
            }
        }
    }
}
```

---

## Data Flow Analysis

### Audio Input Flow

```
pipeline.rs:876-905
         │
         ├── Check is_nut_whisper_active()
         │
         ├── Resample 48kHz → 16kHz
         │   └── rubato resampler
         │
         ├── Buffer in nut_whisper_buffer
         │   └── 100ms batches (1600 samples)
         │
         └── send_audio_to_nut_whisper()
                 │
                 ↓
commands.rs:96-107
         │
         ├── get_nut_whisper_pipeline()
         │
         └── pipeline.send_audio()
                 │
                 ↓
nut_whisper.rs:617-625
         │
         ├── Create NutAudioChunk
         │
         └── audio_tx.send(chunk)
```

### Processing Flow

```
NutWhisperPipeline internal task (nut_whisper.rs:529-602)
         │
         ├── audio_rx.recv().await
         │
         ├── Accumulate in audio_buffer
         │
         ├── Check: audio_buffer.len() >= samples_per_chunk (32000)
         │   └── 2 seconds at 16kHz = 32000 samples
         │
         ├── Extract chunk from buffer
         │
         ├── whisper_clone.process_chunk(audio_chunk).await
         │         │
         │         ├── transcribe_audio() - PARTIAL result
         │         │
         │         ├── Check silence (no_speech_prob)
         │         │
         │         ├── Accumulate audio
         │         │
         │         ├── Check chunks_before_retranscribe
         │         │
         │         └── transcribe_audio() - REFINED result (if threshold)
         │
         └── result_tx.send(result)
```

### Result Flow

```
result_tx.send(result)
         │
         ↓
poll_nut_whisper_results() (commands.rs:111-150)
         │
         ├── pipeline.try_recv().await
         │         │
         │         └── result_rx.try_recv()
         │
         └── Collect all available results
                 │
                 ↓
run_nut_whisper_transcription_loop (worker.rs:46-196)
         │
         ├── tokio::select! every 100ms
         │
         ├── poll_nut_whisper_results().await
         │
         ├── Create TranscriptUpdate
         │
         └── app.emit("transcript-update", &update)
                 │
                 ↓
            Frontend (React)
```

---

## Known Issues and Debugging

### Issue 1: Results Not Appearing on Frontend

**Symptom:** Whisper processes audio successfully but results don't appear in UI.

**Debugging Added:**
```rust
// In nut_whisper.rs
log::info!("[NutWhisper] 📊 Buffer status: {} samples");
log::info!("[NutWhisper] 🔄 Processing chunk #{} ({} samples)");
log::info!("[NutWhisper] ✅ Chunk #{} produced {} results");
log::info!("[NutWhisper] 📤 Sending result to channel: '{}'");

// In commands.rs
log::info!("[NutWhisper] 📥 poll #{} got result: '{}'");

// In worker.rs
log::info!("🥜 Worker received {} results from poll");
log::info!("🥜 ✅ transcript-update event emitted successfully");
```

**Potential Causes:**
1. Audio not reaching NutWhisper (check `send_audio_to_nut_whisper` logs)
2. Buffer not reaching threshold (check "Buffer status" logs)
3. Results stuck in channel (check `try_recv` logs)
4. Event emission failing (check "transcript-update" logs)

### Issue 2: RwLock Deadlock

**Previous Fix:** The code was holding RwLock across await points. Fixed by:
- Cloning data before releasing lock
- Acquiring lock inside `spawn_blocking` instead of outside

```rust
// WRONG - holds lock across await
let guard = self.accumulated_audio.read().await;
self.transcribe_audio(&guard, ...).await  // DEADLOCK!

// RIGHT - clone then release
let accumulated = {
    let guard = self.accumulated_audio.read().await;
    guard.clone()
};
self.transcribe_audio(&accumulated, ...).await  // OK
```

### Issue 3: Polling vs Event-Driven

Current architecture uses polling (100ms intervals). This introduces latency and CPU overhead.

**Alternative: Event-driven with broadcast channel**
```rust
// Instead of polling
let (event_tx, _) = tokio::sync::broadcast::channel(100);

// Internal task sends to broadcast
event_tx.send(result)?;

// Worker subscribes
let mut rx = event_tx.subscribe();
while let Ok(result) = rx.recv().await {
    // Handle result immediately
}
```

---

## Recommendations

### 1. Extract Word Timestamps (High Priority)

Add word extraction in `transcribe_audio_sync`:
```rust
// After getting segment text, extract tokens
for tok_idx in 0..n_tokens {
    if let Ok(token_data) = state.full_get_token_data(seg_idx, tok_idx) {
        if let Ok(token_text) = state.full_get_token_text(seg_idx, tok_idx) {
            // Filter special tokens
            let text = token_text.trim();
            if !text.is_empty() && !text.starts_with('[') && !text.starts_with('<') {
                words.push(WordInfo {
                    word: text.to_string(),
                    start: token_data.t0 as f32 / 100.0,
                    end: token_data.t1 as f32 / 100.0,
                    probability: token_data.p,
                });
            }
        }
    }
}
```

### 2. Implement EagerMode Confirmation (High Priority)

Add common word prefix matching like MacWhisper/Hyprnote:
```rust
pub struct EagerModeState {
    previous_words: Vec<String>,
    confirmed_text: String,
    hypothesis_text: String,
    min_common_prefix: usize,  // 3
}

impl EagerModeState {
    pub fn process(&mut self, new_text: &str) -> (Option<String>, String) {
        let new_words: Vec<&str> = new_text.split_whitespace().collect();
        let common_count = self.find_common_prefix(&new_words);

        if common_count >= self.min_common_prefix {
            // Confirm common words
            let confirmed = new_words[..common_count].join(" ");
            let hypothesis = new_words[common_count..].join(" ");
            self.confirmed_text.push_str(&confirmed);
            self.previous_words = new_words.iter().map(|s| s.to_string()).collect();
            (Some(confirmed), hypothesis)
        } else {
            self.previous_words = new_words.iter().map(|s| s.to_string()).collect();
            (None, new_text.to_string())
        }
    }
}
```

### 3. Two-Tier Event Emission (Medium Priority)

Emit separate events for confirmed vs hypothesis:
```rust
// Confirmed text (stable)
app.emit("transcript-confirmed", ConfirmedUpdate {
    text: confirmed,
    words: confirmed_words,
    is_final: false,
});

// Hypothesis text (may change)
app.emit("transcript-hypothesis", HypothesisUpdate {
    text: hypothesis,
    is_partial: true,
});
```

### 4. Replace Polling with Broadcast (Medium Priority)

```rust
// In NutWhisperPipeline
pub fn subscribe(&self) -> broadcast::Receiver<TranscriptionResult> {
    self.result_broadcast.subscribe()
}

// In worker
let mut rx = pipeline.subscribe();
loop {
    tokio::select! {
        result = rx.recv() => {
            // Handle immediately, no polling delay
        }
    }
}
```

### 5. Add Phrase Finalization (Low Priority)

Better phrase boundary detection:
```rust
const REDEMPTION_TIME_MS: u64 = 400;  // From Hyprnote

// After silence threshold
if silence_duration_ms > REDEMPTION_TIME_MS {
    emit_finalized_phrase();
    reset_phrase();
}
```

---

## Comparison Summary

| Aspect | Nutshell (Original) | NutWhisper (Current) | Recommendation |
|--------|---------------------|---------------------|----------------|
| ML Backend | mlx_whisper (MLX) | whisper-rs (C++) | Keep whisper-rs |
| Word Timestamps | ✅ Extracted | ❌ Empty | **Fix needed** |
| Confirmation | Implicit | None | **Add EagerMode** |
| Events | WebSocket JSON | Tauri events | Keep Tauri |
| Polling | RxPY reactive | 100ms polling | Consider broadcast |
| Diarization | diart2/pyannote | None | Future feature |

---

## Testing Checklist

- [ ] Audio reaches `send_audio_to_nut_whisper` (check logs)
- [ ] Buffer accumulates to 32000 samples (check "Buffer status")
- [ ] `process_chunk` produces results (check "produced X results")
- [ ] Results reach `result_tx` channel (check "Result sent to channel")
- [ ] `poll_nut_whisper_results` receives results (check "poll got result")
- [ ] Worker emits events (check "transcript-update emitted")
- [ ] Frontend receives events (check browser console)

---

*Document generated: February 2025*
*Source: Analysis of Meetily NutWhisper implementation and decompiled Nutshell code*
