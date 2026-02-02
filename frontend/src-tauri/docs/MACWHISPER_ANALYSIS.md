# MacWhisper Transcription Architecture Analysis

> Analysis of MacWhisper.app v13.12.2 (Build 1376) - January 2025
> Purpose: Understanding streaming transcription patterns for NutWhisper implementation

## Table of Contents

1. [Overview](#overview)
2. [Application Structure](#application-structure)
3. [Whisper Engine Architecture](#whisper-engine-architecture)
4. [Streaming Transcription (EagerMode)](#streaming-transcription-eagermode)
5. [Audio Capture Pipeline](#audio-capture-pipeline)
6. [State Management](#state-management)
7. [Speaker Diarization](#speaker-diarization)
8. [Key Insights for NutWhisper](#key-insights-for-nutwhisper)

---

## Overview

MacWhisper is a macOS transcription application that uses a sophisticated multi-engine approach:

- **Primary Engine**: ArgmaxSDK/WhisperKitPro (proprietary, for Pro users)
- **Fallback Engine**: WhisperCPP (whisper.cpp based, for free tier)
- **Cloud Backends**: Groq, OpenAI, Deepgram runners as alternatives

**Key Innovation**: EagerMode - a progressive transcription system that shows unconfirmed text immediately and updates it as confidence increases through repeated transcription passes.

---

## Application Structure

### Bundle Contents

```
MacWhisper.app/Contents/
├── MacOS/
│   └── MacWhisper              # 64MB main executable (Swift)
├── Frameworks/
│   ├── ArgmaxSDK.framework     # 7.4MB - WhisperKitPro (proprietary)
│   ├── RecordKit.framework     # 19MB - Audio capture (includes Metal shaders)
│   ├── Sparkle.framework       # Auto-update
│   ├── YbridOgg.framework      # OGG codec
│   └── YbridOpus.framework     # Opus codec
└── Resources/
    ├── Whisper Bundles (8 total)
    │   ├── whisper_whisper.bundle
    │   ├── WhisperCPP_Runner_WhisperCPP_Runner.bundle
    │   ├── WhisperCPP_Runner_Wispa.bundle          # CoreML variant
    │   ├── WhisperManager_WhisperManager.bundle
    │   ├── WhisperRunnerLoader_WhisperRunnerLoader.bundle
    │   ├── WhisperGroq_Runner_WhisperGroq_Runner.bundle
    │   ├── WhisperOpenAI_Runner_WhisperOpenAI_Runner.bundle
    │   └── WhisperOpenAICompatible_Runner_WhisperOpenAICompatible_Runner.bundle
    ├── Audio Bundles
    │   ├── Audio_Audio.bundle
    │   ├── Audio_AudioMacOS.bundle
    │   └── AudioKit_AudioKit.bundle
    ├── LLM Bundles (for summarization)
    │   ├── PromptAnthropic_Runner_PromptAnthropic_Runner.bundle
    │   ├── PromptGemini_Runner_PromptGemini_Runner.bundle
    │   ├── PromptOllama_Runner_PromptOllama_Runner.bundle
    │   └── PromptOpenAI_Runner_PromptOpenAI_Runner.bundle
    ├── Argmax_Runner_Argmax_Runner.bundle
    │   └── Models/speakerkit-v1.9.3.zip    # Speaker diarization model
    └── Other bundles (Dictation, Recording, Persistence, etc.)
```

### Key Dependencies

| Component | Purpose |
|-----------|---------|
| ArgmaxSDK v1.9.10 | WhisperKitPro - proprietary streaming transcription |
| RecordKit | Audio capture with CoreAudio, ScreenCaptureKit, aggregation |
| AudioKit | Audio processing utilities |
| GRDB | SQLite database for persistence |
| swift-transformers | Hugging Face Hub integration for model downloads |

---

## Whisper Engine Architecture

### Engine Selection Logic

```swift
// Pseudo-code based on string analysis
if allowsProFeatures {
    // Use WhisperKitPro (ArgmaxSDK)
    print("`allowsProFeatures` is true, using WhisperKitPro")
} else {
    // Use standard WhisperKit
    print("`allowsProFeatures` is false, using WhisperKit")
}
```

### WhisperManager States

The `WhisperManager` class manages transcription state:

```
States:
├── isReady              # Model loaded, ready to transcribe
├── startingStreaming    # Initializing stream session
├── streaming            # Active streaming transcription
├── transcribing         # Batch transcription in progress
└── idle                 # No active tasks

Task Types:
├── streamingTask        # Real-time audio stream transcription
└── transcribingTask     # File/batch transcription
```

**State Validation**:
```
[%s] Should only start transcribing when WhisperManager state.isReady
[%s] Throwing `whisperManagerWasNotIdle` because hasTranscribingTask = X, hasStreamingTask = Y
```

### Model Loading

```swift
// Model configuration
WhisperKitProConfig(
    model: modelName,
    downloadBase: URL?,
    modelRepo: String?,
    modelToken: String?,
    modelFolder: URL?,
    tokenizerFolder: URL?,
    computeOptions: ModelComputeOptions,
    audioProcessor: AudioProcessing?,
    featureExtractor: FeatureExtracting?,
    audioEncoder: AudioEncoding?,
    textDecoder: TextDecoding?,
    logitsFilters: [LogitsFiltering]?,
    segmentSeeker: SegmentSeeking?,
    voiceActivityDetector: VoiceActivityDetector?,
    verbose: Bool,
    logLevel: LogLevel,
    prewarm: Bool?,
    load: Bool?,
    useBackgroundDownloadSession: Bool
)
```

**Compute Options**:
- `melPreferredCompute` - For Mel spectrogram computation
- `encoderPreferredCompute` - For audio encoder (Metal/ANE)
- `decoderPreferredCompute` - For text decoder
- `fastLoading` flags for optimized startup

---

## Streaming Transcription (EagerMode)

### Core Concept

EagerMode provides progressive transcription by:
1. Showing **unconfirmed** text immediately (hypothesis)
2. Confirming text when it appears consistently across multiple transcription passes
3. Using **common word matching** to determine confirmation

### EagerMode Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                     Audio Buffer Input                          │
└─────────────────────────────────┬───────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────┐
│              Check: Buffer Ready for Processing?                │
│         [EagerMode] Buffer duration: X seconds                  │
│         [EagerMode] Buffer not ready for processing (if short)  │
└─────────────────────────────────┬───────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────┐
│                    Whisper Transcription                        │
│              Produces: hypothesisSegments                       │
└─────────────────────────────────┬───────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────┐
│                   Word Comparison Logic                         │
│         [EagerMode] Previous words: ["hello", "world"]          │
│         [EagerMode] Current words: ["hello", "world", "how"]    │
│         [EagerMode] Common words: ["hello", "world"]            │
└─────────────────────────────────┬───────────────────────────────┘
                                  ↓
┌─────────────────────────────────────────────────────────────────┐
│                   Confirmation Decision                         │
│    IF commonWords.count >= minCommonPrefixCount:                │
│        → [EagerMode] Common words confirmed                     │
│        → Emit as "confirmed segment text"                       │
│    ELSE:                                                        │
│        → [EagerMode] Not enough common words confirmed          │
│        → Emit as "UNconfirmed segment text"                     │
└─────────────────────────────────────────────────────────────────┘
```

### Key Configuration Parameters

| Parameter | Description |
|-----------|-------------|
| `minCommonPrefixCount` | Minimum common words needed for confirmation |
| `tokenConfirmationsNeeded` | How many passes a token must survive |
| `requiredSegmentsForConfirmation` | Segments needed before confirming |
| `transcribeInterval` | How often to process accumulated audio |
| `audioCallbackInterval` | Audio chunk delivery interval |
| `compressionCheckWindow` | Window for hallucination detection |

### TranscribeStreamSession

The main streaming session class handles:

```
[TranscribeStreamSession] Buffer duration: X
[TranscribeStreamSession] Historical words: [...]
[TranscribeStreamSession] Current words: [...]
[TranscribeStreamSession] Common words: [...]
[TranscribeStreamSession] Using prefix words: [...]   # For continuity
[TranscribeStreamSession] Transcribing from X seconds
[TranscribeStreamSession] Using prefill prompt: "..."
```

### Prefix Prompting for Continuity

EagerMode uses confirmed words as prefix/prompt for subsequent transcriptions:
- `[EagerMode] Using prefix words: ["the", "meeting", "is"]`
- This helps maintain context across chunks
- Similar to `condition_on_previous_text` but with confirmed text only

### Event Emission Pattern

```swift
// WhisperManager streaming events
WhisperManager streaming event: State update: <state>
WhisperManager streaming event: Received confirmed segment text: <timestamp> -> <text>
WhisperManager streaming event: Received UNconfirmed segment text: <timestamp> -> <text>
WhisperManager streaming event: Received completed segments: [...]
WhisperManager streaming event: Stopped before completion
```

---

## Audio Capture Pipeline

### RecordKit Components

```
Audio Sources
├── RKMicrophone          # Microphone input
├── CoreAudioRecorder     # System audio via CoreAudio
├── SCKAudioRecorder      # ScreenCaptureKit audio
└── AggregateDevice       # Combines multiple sources

Processing
├── AudioGapFiller        # Fills gaps in audio stream
├── TimedAudioGapFiller   # Time-aware gap filling
└── AudioStreamRecorder   # Continuous recording

Output
├── AudioFileRecorder     # Write to file
├── MovieFileRecorder     # Video + audio
└── intervalAudioStream   # Streaming to transcription
```

### Audio Format

Based on the code patterns:
- Sample format: Float32
- Channels: Mono (converted for Whisper)
- Sample rate: Likely 16kHz for Whisper input (resampled from capture rate)

### Audio Buffer Management

```
samples, most recent buffer: X
samples, most recent energy: Y
Buffer did not contain floatChannelData (error case)
```

---

## State Management

### WhisperManager State Machine

```
                    ┌──────────────┐
                    │    idle      │
                    └──────┬───────┘
                           │ loadModel()
                           ↓
                    ┌──────────────┐
                    │   isReady    │←──────────────────┐
                    └──────┬───────┘                   │
           ┌───────────────┼───────────────┐          │
           │               │               │          │
           ↓               ↓               ↓          │
    ┌──────────┐    ┌──────────┐    ┌──────────┐     │
    │streaming │    │startingS.│    │transcrib.│     │
    └────┬─────┘    └────┬─────┘    └────┬─────┘     │
         │               │               │           │
         │               ↓               │           │
         │         ┌──────────┐          │           │
         │         │streaming │          │           │
         │         └────┬─────┘          │           │
         │               │               │           │
         └───────────────┴───────────────┴───────────┘
                         │ stop/complete
```

### Error States

```
whisperManagerWasNotIdle     # Tried to start while busy
whisperManagerWasNotReady    # Model not loaded
whisperManagerHasNoSelectedModel  # No model configured
```

---

## Speaker Diarization

### SpeakerKit (Pro Feature)

Located in `Argmax_Runner_Argmax_Runner.bundle/Contents/Resources/Models/speakerkit-v1.9.3.zip`

**Pipeline**:
```
Audio → SpeakerSegmenterModel → Voice Activity Detection
                              ↓
                     SpeakerEmbedder → Generate embeddings per chunk
                              ↓
                     Clustering → Assign speaker IDs
                              ↓
                     addSpeakerInfo → Merge with transcription
```

**Key Classes**:
- `SpeakerKitPro` - Main diarization interface
- `SpeakerSegmenterModel` - VAD + segmentation
- `SpeakerEmbedder` - Generate speaker embeddings
- `DiarizationResult` - Contains speaker segments

**Logs**:
```
[SpeakerSegmenter] initialized with X
[SpeakerEmbedder] Processing single chunk X
[SpeakerEmbedder] segmenterOutput: ...
[SpeakerKit] No words for segment: X
[SK] updateSegments with min X
```

---

## Key Insights for NutWhisper

### 1. Progressive Confirmation Pattern

**Adopt the EagerMode approach**:
```rust
struct NutWhisperState {
    confirmed_words: Vec<WordTiming>,
    hypothesis_words: Vec<WordTiming>,
    min_common_prefix_count: usize,  // e.g., 3
    token_confirmations_needed: usize, // e.g., 2
}

fn process_transcription(&mut self, new_result: TranscriptionResult) {
    let current_words = extract_words(&new_result);
    let common_words = find_common_prefix(&self.hypothesis_words, &current_words);

    if common_words.len() >= self.min_common_prefix_count {
        // Confirm the common words
        self.confirmed_words.extend(common_words.clone());
        self.hypothesis_words = current_words[common_words.len()..].to_vec();
        emit_confirmed(common_words);
    } else {
        // Update hypothesis only
        self.hypothesis_words = current_words;
        emit_unconfirmed(&self.hypothesis_words);
    }
}
```

### 2. Buffer Duration Threshold

Don't process until buffer has enough audio:
```rust
const MIN_BUFFER_DURATION_SECS: f32 = 1.0;  // Minimum before processing
const PROCESS_INTERVAL_SECS: f32 = 2.0;     // How often to transcribe

if buffer_duration < MIN_BUFFER_DURATION_SECS {
    log::debug!("[NutWhisper] Buffer not ready for processing");
    return;
}
```

### 3. Prefix Prompting

Use confirmed text as prompt for continuity:
```rust
fn build_prompt(&self) -> Option<String> {
    if self.confirmed_words.is_empty() {
        return None;
    }

    // Use last N confirmed words as prompt
    let prompt_words: Vec<&str> = self.confirmed_words
        .iter()
        .rev()
        .take(10)
        .map(|w| w.text.as_str())
        .collect();

    Some(prompt_words.into_iter().rev().collect::<Vec<_>>().join(" "))
}
```

### 4. Event Structure

Emit two types of events like MacWhisper:
```rust
enum TranscriptEvent {
    Confirmed {
        text: String,
        start_time: f64,
        end_time: f64,
        words: Vec<WordTiming>,
    },
    Unconfirmed {
        text: String,
        start_time: f64,
        words: Vec<WordTiming>,
    },
}
```

### 5. Hallucination Prevention

MacWhisper uses `compressionCheckWindow` and early stopping:
```rust
// If compression ratio is too high, likely hallucination
if compression_ratio > COMPRESSION_THRESHOLD {
    log::warn!("[NutWhisper] Early stopping due to compression threshold");
    return;
}

// If logprob is too low, likely garbage
if avg_logprob < LOGPROB_THRESHOLD {
    log::warn!("[NutWhisper] Early stopping due to logprob threshold");
    return;
}
```

### 6. Segment Post-Processing

MacWhisper applies post-processing:
- `automaticallyCombineSegmentsToSentences` - Merge short segments
- `ignoreAllBracketedSegments` - Filter `[music]`, `[silence]`, etc.
- `removeAsteriskSegments` - Filter hallucinated patterns

```rust
fn should_filter_segment(text: &str) -> bool {
    // Filter bracketed content
    if text.starts_with('[') && text.ends_with(']') {
        return true;
    }
    // Filter asterisk patterns (often hallucination)
    if text.contains('*') {
        return true;
    }
    // Filter very short segments
    if text.trim().len() < 2 {
        return true;
    }
    false
}
```

### 7. Chunk Configuration

Based on MacWhisper's approach:
```rust
struct ChunkConfig {
    frames_per_chunk: usize,      // Audio frames per chunk
    chunk_stride: usize,          // Overlap between chunks
    transcribe_interval_secs: f32, // How often to transcribe
}

// Suggested values based on analysis
const DEFAULT_CONFIG: ChunkConfig = ChunkConfig {
    frames_per_chunk: 16000 * 3,  // 3 seconds at 16kHz
    chunk_stride: 16000,          // 1 second overlap
    transcribe_interval_secs: 2.0,
};
```

---

## Comparison: MacWhisper vs Current NutWhisper

| Feature | MacWhisper | NutWhisper (Current) |
|---------|------------|---------------------|
| Buffer Strategy | Time-based with minimum duration | 2-second fixed chunks |
| Confirmation | Common word prefix matching | None (immediate emit) |
| Event Types | Confirmed + Unconfirmed | Single type |
| Prefix Prompting | Confirmed words as prompt | Not implemented |
| Hallucination Filter | Compression + logprob checks | Not implemented |
| Post-Processing | Segment merging, bracket filtering | Not implemented |

---

## Recommendations for NutWhisper

1. **Implement two-tier event system**: Emit unconfirmed immediately, update to confirmed
2. **Add common word confirmation**: Compare consecutive transcriptions
3. **Use prefix prompting**: Feed confirmed text back to Whisper
4. **Add filtering**: Remove hallucinated patterns and bracketed content
5. **Track word timings**: Enable word-level timestamps for better confirmation
6. **Consider buffer overlap**: Use sliding window with overlap for smoother results

---

## Appendix: Key Strings from Binary Analysis

### EagerMode Logs
```
[EagerMode] Buffer duration:
[EagerMode] Buffer not ready for processing
[EagerMode] Common words:
[EagerMode] Current words:
[EagerMode] Previous words:
[EagerMode] Using prefix words:
[EagerMode] Common words confirmed
[EagerMode] Not enough common words confirmed
[EagerMode] Processing final buffer:
[EagerMode] Skipping empty audio buffer
[EagerMode] Starting transcription task
[EagerMode] Finalizing transcription
```

### WhisperManager Logs
```
WhisperManager streaming event: State update:
WhisperManager streaming event: Received confirmed segment text:
WhisperManager streaming event: Received UNconfirmed segment text:
WhisperManager streaming event: Received completed segments:
WhisperManager streaming event: Stopped before completion
```

### Configuration Keys
```
minCommonPrefixCount
tokenConfirmationsNeeded
requiredSegmentsForConfirmation
transcribeInterval
audioCallbackInterval
compressionCheckWindow
hypothesisSegments
confirmedSegments
unconfirmedBuffer
```

---

*Document generated: February 2025*
*Source: Analysis of MacWhisper.app v13.12.2*
