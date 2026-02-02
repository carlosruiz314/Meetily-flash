# Streaming Transcription Implementation Recommendations

> Synthesized insights from MacWhisper v13.12.2, Hyprnote Nightly v1.0.3, and Original Nutshell
> Purpose: Guide NutWhisper/Meetily implementation

## Executive Summary

All analyzed apps use similar core patterns for streaming transcription:

1. **EagerMode** - Progressive transcription with word confirmation (MacWhisper, Hyprnote)
2. **Common prefix matching** - Confirm words that appear consistently
3. **Two-tier results** - Immediate hypothesis + delayed confirmation
4. **Time-based chunking** - Regular intervals, not VAD-gated
5. **`condition_on_previous_text=False`** - Critical for preventing hallucination loops (Nutshell)

---

## Architecture Comparison

| Aspect | MacWhisper | Hyprnote | Nutshell (Original) | Meetily/NutWhisper |
|--------|------------|----------|---------------------|-------------------|
| **Language** | Swift (native) | Rust + Swift sidecar | Python + Electron | Rust |
| **ML Runtime** | ArgmaxSDK (proprietary) | ArgmaxSDK (sidecar) | mlx_whisper (MLX) | whisper-rs (C++) |
| **Architecture** | Monolithic | Sidecar + WebSocket | Server + WebSocket | Monolithic |
| **Streaming** | In-process | WebSocket API | WebSocket (port 3005) | Direct calls |
| **Progressive** | EagerMode | EagerMode | PhraseAccumulator | NutWhisper (WIP) |
| **Actor System** | None | ractor | RxPY Observers | Manual threads |
| **Diarization** | None | Deepgram API | diart2/pyannote | None |
| **Word Timestamps** | ✅ Extracted | ✅ Via API | ✅ Extracted | ❌ **Not extracted** |

---

## Original Nutshell Insights

### Architecture (From Decompiled Code)

```
Electron App
     │
     ├── NutshellElectron.node (C++ - CoreAudio)
     │   └── Multi-Output Device (no BlackHole needed!)
     │
     └── WebSocket (port 3005)
             │
             └── Python Server (PyInstaller)
                 ├── mlx_whisper (MLX-optimized for Apple Silicon)
                 ├── TranscriptionPipeline (RxPY reactive streams)
                 ├── PhraseAccumulator (Observer pattern)
                 ├── MergeHandler (with diarization)
                 └── diart2/pyannote (speaker diarization)
```

### Critical Nutshell Parameters

From `transcribe_handler.py`:
```python
model_name = 'whisper-base.en-mlx'
word_timestamps = True
condition_on_previous_text = False  # CRITICAL - prevents hallucination loops!
no_speech_prob_threshold = 0.65
chunk_size = 5  # messages per chunk
```

### Nutshell Hallucination Detection

```python
# Pattern-based hallucination filter
hallucination_string = 'alittlebitofalittlebitofalittlebitofalittlebitof'
if hallucination_string in segment['text'].replace(' ', ''):
    segment['text'] = ''
    segment['words'] = []

# Repetition filter (4+ consecutive same words)
def handle_repeated_words(words, min_repetitions=4):
    # Detects and removes repeated word sequences
```

### Nutshell Data Model

```python
class Word:
    start: float      # Start time in seconds
    end: float        # End time in seconds
    word: str         # The word text
    probability: float  # Confidence score

class Phrase:
    phrase_id: uuid.UUID
    text: str
    is_completed: bool
    start: float
    end: float
    words: List[Word]
    speaker: str      # From diarization
    label: str
```

---

## Core Algorithm: EagerMode

### The Pattern

Both apps use identical pattern from Argmax:

```
Audio Buffer (time-based chunks)
         ↓
    Transcribe Chunk
         ↓
    Extract Current Words
         ↓
    Compare with Previous Words
         ↓
    Find Common Prefix
         ↓
┌────────┴────────┐
│                 │
↓                 ↓
Enough common     Not enough
words?            common words
↓                 ↓
Confirm &         Emit as
Emit Final        Hypothesis
```

### Implementation

```rust
pub struct EagerModeState {
    /// Previously transcribed words
    previous_words: Vec<WordTiming>,
    /// Currently confirmed words
    confirmed_words: Vec<WordTiming>,
    /// Words awaiting confirmation
    hypothesis_words: Vec<WordTiming>,
    /// Minimum common words for confirmation
    min_common_prefix_count: usize,
    /// How many times token must appear
    token_confirmations_needed: usize,
    /// Time of last confirmed word
    last_confirmed_time: f64,
}

impl EagerModeState {
    pub fn new() -> Self {
        Self {
            previous_words: Vec::new(),
            confirmed_words: Vec::new(),
            hypothesis_words: Vec::new(),
            min_common_prefix_count: 3,        // From Argmax
            token_confirmations_needed: 2,     // From Argmax
            last_confirmed_time: 0.0,
        }
    }

    pub fn process_transcription(&mut self, result: TranscriptionResult) -> EagerModeOutput {
        let current_words = extract_words(&result);

        // Find common prefix between previous and current
        let common_count = self.find_common_prefix(&current_words);

        if common_count >= self.min_common_prefix_count {
            // Confirm the common words
            let confirmed = current_words[..common_count].to_vec();
            let remaining = current_words[common_count..].to_vec();

            // Update state
            self.confirmed_words.extend(confirmed.clone());
            self.last_confirmed_time = confirmed.last()
                .map(|w| w.end_time)
                .unwrap_or(self.last_confirmed_time);

            // Prepare next comparison
            self.previous_words = current_words;
            self.hypothesis_words = remaining.clone();

            EagerModeOutput::Confirmed {
                confirmed_text: words_to_text(&confirmed),
                hypothesis_text: words_to_text(&remaining),
                confirmed_words: confirmed,
                hypothesis_words: remaining,
            }
        } else {
            // Not enough confirmation, update hypothesis
            self.previous_words = current_words.clone();
            self.hypothesis_words = current_words.clone();

            EagerModeOutput::Hypothesis {
                text: words_to_text(&current_words),
                words: current_words,
            }
        }
    }

    fn find_common_prefix(&self, current: &[WordTiming]) -> usize {
        let mut count = 0;
        for (prev, curr) in self.previous_words.iter().zip(current.iter()) {
            if prev.text.to_lowercase() == curr.text.to_lowercase() {
                count += 1;
            } else {
                break;
            }
        }
        count
    }
}

pub enum EagerModeOutput {
    Confirmed {
        confirmed_text: String,
        hypothesis_text: String,
        confirmed_words: Vec<WordTiming>,
        hypothesis_words: Vec<WordTiming>,
    },
    Hypothesis {
        text: String,
        words: Vec<WordTiming>,
    },
}
```

---

## Prefix Prompting

Both apps use confirmed words as prompt for next transcription:

```rust
impl EagerModeState {
    /// Build prompt from confirmed words for Whisper
    pub fn build_prompt(&self, max_words: usize) -> Option<String> {
        if self.confirmed_words.is_empty() {
            return None;
        }

        let prompt_words: Vec<&str> = self.confirmed_words
            .iter()
            .rev()
            .take(max_words)
            .map(|w| w.text.as_str())
            .collect();

        Some(prompt_words.into_iter().rev().collect::<Vec<_>>().join(" "))
    }
}

// Usage with whisper-rs
fn transcribe_chunk(whisper: &WhisperContext, audio: &[f32], prompt: Option<&str>) {
    let mut params = whisper_rs::FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    if let Some(prompt_text) = prompt {
        params.set_initial_prompt(prompt_text);
    }

    // Important: Don't use previous context between chunks
    params.set_no_context(true);

    whisper.full(params, audio).unwrap();
}
```

---

## Buffer Configuration

### Recommended Values

Based on both apps:

| Parameter | MacWhisper | Hyprnote | Recommended |
|-----------|------------|----------|-------------|
| Chunk duration | 2-3 sec | 2 sec | 2 sec |
| Chunk overlap | ~1 sec | Variable | 0.5-1 sec |
| Process interval | 2 sec | 2 sec | 2 sec |
| Redemption time | N/A | 400ms | 400ms |
| Min buffer | 1 sec | ~1 sec | 1 sec |

### Implementation

```rust
pub struct ChunkConfig {
    /// Audio sample rate
    pub sample_rate: u32,
    /// Chunk duration in seconds
    pub chunk_duration_secs: f32,
    /// Minimum buffer before processing
    pub min_buffer_secs: f32,
    /// Overlap between chunks
    pub overlap_secs: f32,
    /// Silence before finalizing
    pub redemption_time_ms: u32,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            chunk_duration_secs: 2.0,
            min_buffer_secs: 1.0,
            overlap_secs: 0.5,
            redemption_time_ms: 400,
        }
    }
}

impl ChunkConfig {
    pub fn samples_per_chunk(&self) -> usize {
        (self.sample_rate as f32 * self.chunk_duration_secs) as usize
    }

    pub fn min_buffer_samples(&self) -> usize {
        (self.sample_rate as f32 * self.min_buffer_secs) as usize
    }

    pub fn overlap_samples(&self) -> usize {
        (self.sample_rate as f32 * self.overlap_secs) as usize
    }
}
```

---

## Event Structure

### Two-Tier Events

```rust
#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum TranscriptEvent {
    /// Immediate hypothesis (may change)
    Interim {
        text: String,
        start_time: f64,
        words: Vec<WordTiming>,
    },
    /// Confirmed transcription (stable)
    Final {
        text: String,
        start_time: f64,
        end_time: f64,
        words: Vec<WordTiming>,
        confidence: f32,
    },
    /// Speech detected starting
    SpeechStarted {
        timestamp: f64,
    },
    /// End of utterance (after redemption time)
    UtteranceEnd {
        last_word_end: f64,
    },
}
```

### Frontend Handling

```typescript
interface TranscriptState {
  confirmed: TranscriptSegment[];
  hypothesis: string | null;
}

function handleTranscriptEvent(event: TranscriptEvent, state: TranscriptState): TranscriptState {
  switch (event.type) {
    case 'Interim':
      return {
        ...state,
        hypothesis: event.text,
      };

    case 'Final':
      return {
        confirmed: [...state.confirmed, {
          text: event.text,
          startTime: event.start_time,
          endTime: event.end_time,
          words: event.words,
        }],
        hypothesis: null, // Clear hypothesis when confirmed
      };

    case 'UtteranceEnd':
      return {
        ...state,
        hypothesis: null,
      };

    default:
      return state;
  }
}
```

---

## Hallucination Prevention

Both apps implement similar safeguards:

### 1. Compression Threshold

```rust
const COMPRESSION_THRESHOLD: f32 = 2.4;

fn check_compression(text: &str) -> bool {
    // High repetition = likely hallucination
    let unique_words: HashSet<_> = text.split_whitespace().collect();
    let total_words = text.split_whitespace().count();

    if total_words == 0 {
        return false;
    }

    let ratio = total_words as f32 / unique_words.len() as f32;
    ratio > COMPRESSION_THRESHOLD
}
```

### 2. Logprob Threshold

```rust
const LOGPROB_THRESHOLD: f32 = -1.0;

fn check_logprob(avg_logprob: f32) -> bool {
    // Very low logprob = likely garbage
    avg_logprob < LOGPROB_THRESHOLD
}
```

### 3. Bracketed Content Filter

```rust
fn filter_hallucinations(text: &str) -> String {
    // Filter patterns like [music], [silence], (inaudible)
    let bracketed = regex::Regex::new(r"\[.*?\]|\(.*?\)").unwrap();
    let filtered = bracketed.replace_all(text, "");

    // Filter asterisk patterns
    let asterisks = regex::Regex::new(r"\*+.*?\*+").unwrap();
    let filtered = asterisks.replace_all(&filtered, "");

    // Filter repeated phrases
    // TODO: Implement repetition detection

    filtered.trim().to_string()
}
```

### 4. No Context Between Chunks

```rust
// CRITICAL: Prevents hallucination loops
params.set_no_context(true);

// Use explicit prompt instead
if let Some(prompt) = eager_state.build_prompt(10) {
    params.set_initial_prompt(&prompt);
}
```

---

## Architecture Recommendations

### Option 1: Enhanced Monolithic (Simpler)

Keep current architecture but add:

```
┌─────────────────────────────────────────────────────────────┐
│                    Meetily (Tauri)                          │
│  ┌────────────┐    ┌──────────────┐    ┌────────────────┐  │
│  │ Audio      │ → │ NutWhisper   │ → │ EagerMode      │  │
│  │ Pipeline   │    │ (whisper-rs) │    │ Processor     │  │
│  └────────────┘    └──────────────┘    └────────┬───────┘  │
│                                                  │          │
│  ┌──────────────────────────────────────────────┴───────┐  │
│  │                    Event Emitter                      │  │
│  │  ┌─────────┐  ┌─────────┐  ┌────────────────────┐   │  │
│  │  │ Interim │  │  Final  │  │  UtteranceEnd      │   │  │
│  │  └─────────┘  └─────────┘  └────────────────────┘   │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**Pros**: Simpler, less IPC overhead
**Cons**: All in one process, harder to update ML separately

### Option 2: Sidecar Pattern (Like Hyprnote)

```
┌─────────────────────────────────────────────────────────────┐
│                    Meetily (Tauri)                          │
│  ┌────────────┐                  ┌────────────────────────┐ │
│  │ Audio      │ → ws://localhost │  Event Handler         │ │
│  │ Pipeline   │                  │  (Tauri events)        │ │
│  └────────────┘                  └────────────────────────┘ │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ↓
┌─────────────────────────────────────────────────────────────┐
│              meetily-stt-sidecar (Rust)                     │
│  ┌────────────┐    ┌──────────────┐    ┌────────────────┐  │
│  │ WebSocket  │ → │ whisper-rs   │ → │ EagerMode      │  │
│  │ Server     │    │ Transcriber  │    │ Processor     │  │
│  └────────────┘    └──────────────┘    └────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**Pros**: Process isolation, easier testing, can swap implementations
**Cons**: More complex, WebSocket overhead

### Option 3: Hybrid with Actor System

```
┌─────────────────────────────────────────────────────────────┐
│                    Meetily (Tauri + ractor)                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                   Session Supervisor                    │ │
│  │  ┌──────────┐  ┌────────────┐  ┌────────────────────┐ │ │
│  │  │ Source   │  │ Transcribe │  │     Recorder       │ │ │
│  │  │ Actor    │  │   Actor    │  │      Actor         │ │ │
│  │  └────┬─────┘  └─────┬──────┘  └────────────────────┘ │ │
│  │       │              │                                 │ │
│  │       └──────┬───────┘                                 │ │
│  │              ↓                                         │ │
│  │      Audio Channel (mpsc)                              │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

**Pros**: Clean separation, crash recovery, backpressure
**Cons**: Learning curve for actor pattern

---

## Immediate Implementation Steps

### Step 1: Fix Current NutWhisper Result Flow

Based on previous debugging, ensure results flow:
1. `process_chunk()` → results in `result_tx`
2. `poll_nut_whisper_results()` → retrieves from `result_rx`
3. Worker emits `transcript-update` event
4. Frontend receives and displays

### Step 2: Add EagerMode Processing

```rust
// In worker.rs or new eager_mode.rs
pub struct NutWhisperWorker {
    eager_state: EagerModeState,
    chunk_config: ChunkConfig,
}

impl NutWhisperWorker {
    pub fn process_results(&mut self, results: Vec<TranscriptionResult>) {
        for result in results {
            let output = self.eager_state.process_transcription(result);

            match output {
                EagerModeOutput::Confirmed { confirmed_text, hypothesis_text, .. } => {
                    // Emit Final event
                    self.emit_final(confirmed_text);
                    // Emit Interim for remaining hypothesis
                    if !hypothesis_text.is_empty() {
                        self.emit_interim(hypothesis_text);
                    }
                }
                EagerModeOutput::Hypothesis { text, .. } => {
                    // Emit Interim event
                    self.emit_interim(text);
                }
            }
        }
    }
}
```

### Step 3: Update Frontend Event Handling

```typescript
// In TranscriptDisplay.tsx or similar
const [transcriptState, setTranscriptState] = useState<TranscriptState>({
  confirmed: [],
  hypothesis: null,
});

useEffect(() => {
  const unlisten = listen<TranscriptEvent>('transcript-update', (event) => {
    setTranscriptState(prev => handleTranscriptEvent(event.payload, prev));
  });

  return () => { unlisten.then(fn => fn()); };
}, []);

// Render with hypothesis styling
return (
  <div>
    {transcriptState.confirmed.map(segment => (
      <span key={segment.startTime}>{segment.text} </span>
    ))}
    {transcriptState.hypothesis && (
      <span className="text-gray-400 italic">{transcriptState.hypothesis}</span>
    )}
  </div>
);
```

---

## Current NutWhisper Gap Analysis

### What NutWhisper Has ✅

| Feature | Status | Implementation |
|---------|--------|----------------|
| Time-based chunking | ✅ | 2 second chunks |
| No external VAD | ✅ | Uses `no_speech_prob` threshold |
| `no_context(true)` | ✅ | Prevents hallucination loops |
| Re-transcription | ✅ | Every 3 chunks (6 seconds) |
| Hallucination filter | ✅ | Pattern + repetition detection |
| Token timestamps | ✅ | Enabled in params |
| Progressive results | ✅ | partial/refined/final |

### What NutWhisper is Missing ❌

| Feature | Nutshell/MacWhisper/Hyprnote | NutWhisper | Priority |
|---------|------------------------------|------------|----------|
| **Word timestamps** | Extracted per-word | `words: Vec::new()` | **HIGH** |
| **EagerMode confirmation** | Common prefix matching | Not implemented | **HIGH** |
| **Two-tier events** | Confirmed + Hypothesis | Single event type | **MEDIUM** |
| **Phrase finalization** | Redemption time (400ms) | Inline logic | **MEDIUM** |
| **Speaker diarization** | diart2/pyannote | None | LOW |

### Critical Fix: Word Timestamp Extraction

The current implementation enables token timestamps but **doesn't extract them**:

```rust
// Current code in transcribe_audio_sync
params.set_token_timestamps(true);  // ✅ Enabled

// But in the result construction:
Ok(TranscriptionResult {
    // ...
    words: Vec::new(), // ❌ Always empty!
    // ...
})
```

**Required Fix:**
```rust
// Extract word-level timestamps from Whisper state
let mut words = Vec::new();
for seg_idx in 0..num_segments {
    if let Ok(n_tokens) = state.full_n_tokens(seg_idx) {
        for tok_idx in 0..n_tokens {
            if let Ok(token_data) = state.full_get_token_data(seg_idx, tok_idx) {
                if let Ok(token_text) = state.full_get_token_text(seg_idx, tok_idx) {
                    let text = token_text.trim();
                    // Filter special tokens like [BLANK], <|endoftext|>
                    if !text.is_empty() && !text.starts_with('[') && !text.starts_with('<') {
                        words.push(WordInfo {
                            word: text.to_string(),
                            start: token_data.t0 as f32 / 100.0, // centiseconds → seconds
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

## Testing Checklist

- [ ] Buffer accumulates 2 seconds before first transcription
- [ ] Results appear on frontend within 200ms of transcription
- [ ] Confirmed text is stable (doesn't change)
- [ ] Hypothesis text updates progressively
- [ ] No hallucination loops during silence
- [ ] Clean finalization after speech ends
- [ ] Memory usage stable over long recordings
- [ ] **Word timestamps populated** (not empty)
- [ ] **EagerMode confirmation working** (common prefix detected)

---

## Implementation Priority

### Phase 1: Fix Word Timestamps (Immediate)
1. Update `transcribe_audio_sync` in `nut_whisper.rs`
2. Extract token data from Whisper state
3. Filter special tokens
4. Populate `words` field in `TranscriptionResult`

### Phase 2: Add EagerMode Confirmation
1. Create `EagerModeState` struct
2. Implement common prefix matching
3. Separate confirmed vs hypothesis text
4. Update frontend to display both tiers

### Phase 3: Two-Tier Event Emission
1. Add `transcript-confirmed` event
2. Add `transcript-hypothesis` event
3. Update frontend state management
4. Style hypothesis text differently (italic/gray)

### Phase 4: Polish
1. Implement redemption time (400ms)
2. Add phrase finalization logic
3. Consider broadcast channels vs polling

---

## References

- MacWhisper Analysis: `MACWHISPER_ANALYSIS.md`
- Hyprnote Analysis: `HYPRNOTE_ANALYSIS.md`
- NutWhisper Analysis: `NUTWHISPER_ANALYSIS.md`
- Argmax WhisperKit: https://github.com/argmaxinc/WhisperKit
- ractor (Rust actors): https://github.com/slawlor/ractor
- Original Nutshell (decompiled): `research/nutshell-decompiled/`

---

*Document updated: February 2025*
*Sources: MacWhisper v13.12.2, Hyprnote Nightly v1.0.3-nightly.2, Nutshell (decompiled), NutWhisper (current)*
