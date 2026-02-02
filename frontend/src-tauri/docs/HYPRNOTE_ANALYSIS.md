# Hyprnote Transcription Architecture Analysis

> Analysis of Hyprnote Nightly.app v1.0.3-nightly.2 - February 2025
> Purpose: Understanding streaming transcription architecture using Argmax SDK

## Table of Contents

1. [Overview](#overview)
2. [Application Architecture](#application-architecture)
3. [Sidecar STT Server](#sidecar-stt-server)
4. [Actor-Based Audio Pipeline](#actor-based-audio-pipeline)
5. [Argmax SDK Deep Dive](#argmax-sdk-deep-dive)
6. [WebSocket Streaming Protocol](#websocket-streaming-protocol)
7. [Why Argmax is Efficient](#why-argmax-is-efficient)
8. [Key Insights for Implementation](#key-insights-for-implementation)

---

## Overview

Hyprnote is a Tauri-based meeting transcription app that uses a **sidecar architecture**:

```
┌─────────────────────────────────────────────────────────────────┐
│                 hyprnote-nightly (230MB)                        │
│                    Tauri + Rust Main App                        │
│  ┌───────────────┐  ┌────────────────┐  ┌───────────────────┐  │
│  │ Audio Capture │  │  Actor System  │  │   WebSocket       │  │
│  │  (mic/speaker)│→ │   (ractor)     │→ │   Client          │  │
│  └───────────────┘  └────────────────┘  └─────────┬─────────┘  │
└───────────────────────────────────────────────────┼─────────────┘
                                                    │ ws://localhost:PORT/v1/listen
                                                    ↓
┌─────────────────────────────────────────────────────────────────┐
│                 hyprnote-sidecar-stt (25MB)                     │
│                Swift Vapor Server + Argmax SDK                  │
│  ┌───────────────┐  ┌────────────────┐  ┌───────────────────┐  │
│  │    Vapor      │→ │  WhisperKit/   │→ │   CoreML/Metal    │  │
│  │   WebSocket   │  │  Parakeet Pro  │  │   Acceleration    │  │
│  └───────────────┘  └────────────────┘  └───────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

**Key Innovation**: Separation of concerns - Rust handles audio capture and UI, Swift handles ML inference with native Apple Silicon optimization.

---

## Application Architecture

### Bundle Structure

```
Hyprnote Nightly.app/Contents/
├── Info.plist                      # App metadata
├── MacOS/
│   ├── hyprnote-nightly           # 230MB - Main Tauri app (Rust)
│   └── hyprnote-sidecar-stt       # 25MB - STT server (Swift)
└── Resources/
    └── icons/                      # App icons
```

### Why This Architecture?

1. **Language Optimization**:
   - **Rust** (Main app): Excellent for audio I/O, threading, system integration
   - **Swift** (Sidecar): Native CoreML/Metal integration, optimized for Apple Silicon

2. **Process Isolation**:
   - ML inference doesn't block UI
   - Can restart sidecar independently
   - Memory management separated

3. **API Flexibility**:
   - Deepgram-compatible WebSocket API
   - Can swap local sidecar for cloud API seamlessly
   - Same client code works with either

### Tauri Plugins Used

From binary analysis:
```
tauri_plugin_listener       # Audio session management
tauri_plugin_listener2      # Batch transcription
tauri_plugin_network        # Network status monitoring
tauri_plugin_local_stt      # Sidecar management
tauri_plugin_detect         # Device detection
tauri_plugin_store          # State persistence
tauri_plugin_fs_db          # Database access
tauri_plugin_pdf            # PDF export
tauri_plugin_deeplink2      # Deep link handling
```

---

## Sidecar STT Server

### Server Overview

The sidecar is a **Vapor (Swift)** web server running locally:

```swift
// CLI Interface
argmax-local-server serve
  --port 50060
  --model large-v3-v20240930_626MB
  --stream-mode voiceTriggered
```

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/init` | POST | Initialize server with model |
| `/v1/status` | GET | Check server/model status |
| `/v1/waitForReady` | GET | Block until model loaded |
| `/v1/reset` | POST | Reinitialize server |
| `/v1/unload` | POST | Unload model |
| `/v1/shutdown` | POST | Stop server |
| `/v1/listen` | WS | Audio streaming endpoint |

### Initialization Flow

```
1. Main app spawns sidecar process
2. POST /v1/init with model config
3. Server loads model in background
4. Poll /v1/status or block on /v1/waitForReady
5. Connect WebSocket to /v1/listen
6. Stream audio, receive transcriptions
```

### Server Logs (from user):
```
[Vapor] Server started on http://::1:53439
[Vapor] POST /v1/init
res=Initializing { message: "Server initialization started...",
                   model: "nvidia_parakeet-v3_494MB",
                   verbose: false }
```

---

## Actor-Based Audio Pipeline

### Actor System (ractor)

Hyprnote uses **ractor** - a Rust actor framework for concurrent audio processing:

```
                    ┌──────────────────────────┐
                    │      root_actor          │
                    │   (listener_root_actor)  │
                    └────────────┬─────────────┘
                                 │
                    ┌────────────┴─────────────┐
                    │   session_supervisor     │
                    │ (per recording session)  │
                    └────────────┬─────────────┘
           ┌─────────────────────┼─────────────────────┐
           │                     │                     │
    ┌──────┴──────┐      ┌──────┴──────┐      ┌──────┴──────┐
    │   source    │      │  listener   │      │  recorder   │
    │  (audio)    │      │   actor     │      │   actor     │
    └──────┬──────┘      └──────┬──────┘      └─────────────┘
           │                    │
    ┌──────┴──────┐      ┌──────┴──────┐
    │   stream    │      │   stream    │
    │ (pipeline)  │      │   (ws)      │
    └─────────────┘      └─────────────┘
```

### Actor Responsibilities

| Actor | Purpose |
|-------|---------|
| `root_actor` | Spawns/manages session supervisors |
| `session_supervisor` | Coordinates single recording session |
| `source` | Manages audio sources (mic/speaker) |
| `listener_actor` | Sends audio to STT, receives results |
| `recorder_actor` | Saves audio to disk |
| `stream` (source) | Audio capture pipeline with AEC |
| `stream` (listener) | WebSocket streaming to sidecar |

### Audio Capture Flow

From logs:
```rust
// Source modes
start_source_loop new_mode=MicAndSpeaker mode_changed=false

// Audio sample rates
sample_rate=SampleRate(48000)  // Mic capture rate
sample_rate init=48000.0       // Speaker capture rate

// Resampled to 16kHz for STT
sample_rate=16000 (in WebSocket URL)
```

### Pipeline Features

```
Audio Sources (48kHz)
       │
       ├── Mic Stream (crates/audio/src/mic.rs)
       │      │
       │      └── Sample format conversion
       │
       └── Speaker Stream (crates/audio/src/speaker/macos.rs)
              │
              └── ScreenCaptureKit integration
       │
       ↓
Pipeline (plugins/listener/src/actors/source/pipeline.rs)
       │
       ├── Acoustic Echo Cancellation (AEC)
       │      └── aec_init_failed / aec_failed (error handling)
       │
       ├── Resampling (48kHz → 16kHz)
       │
       ├── Buffer management
       │      ├── mic_queue_overflow
       │      └── spk_queue_overflow
       │
       └── Audio mixing
              │
              ↓
         16kHz PCM → WebSocket
```

---

## Argmax SDK Deep Dive

### Available Models

From server help and binary analysis:

| Model | Type | Description |
|-------|------|-------------|
| `large-v3-v20240930_626MB` | Whisper | Default, high accuracy |
| `nvidia_parakeet-v3_494MB` | Parakeet | Fast, efficient |
| `am-parakeet-v3` | Parakeet | Argmax-optimized Parakeet |
| `parakeet-tdt_ctc-110m` | CTC | Lightweight CTC-based |

### WhisperKit vs Parakeet

**WhisperKit** (Whisper models):
- Encoder-decoder architecture
- Uses attention mechanism
- Higher accuracy, more compute

**Parakeet** (NVIDIA's CTC model):
- CTC (Connectionist Temporal Classification) based
- No decoder, direct audio-to-text
- Faster inference, less memory

### Key Argmax Components

From binary analysis:

```swift
// Core Transcription
WhisperKitPro              // Enhanced WhisperKit
ParakeetAudioEncoderPro    // Parakeet audio encoder
ParakeetTextDecoderPro     // Parakeet text decoder
ParakeetFeatureExtractor   // Mel spectrogram extraction

// Streaming
TranscribeStreamSession    // Main streaming session
AudioStreamTranscriber     // Audio-to-text streaming
EagerMode                  // Progressive transcription

// Speaker Diarization
SpeakerKitPro             // Main diarization
SpeakerSegmenterModel     // Voice segmentation
SpeakerEmbedderModel      // Speaker embeddings
Diarizer                  // Clustering + assignment

// Algorithms
ContextGraphCTC           // CTC with context boosting
WordSpotter               // Keyword detection
VBxClustering             // Speaker clustering
```

### EagerMode (Progressive Transcription)

Same pattern as MacWhisper:

```
[EagerMode] Buffer duration: X
[EagerMode] Previous words: [...]
[EagerMode] Current words: [...]
[EagerMode] Common words confirmed / Not enough common words confirmed

[TranscribeStreamSession] Common prefix: [...]
[TranscribeStreamSession] Confirming X words
[TranscribeStreamSession] Hypothesis: [...]
```

**Key Parameters**:
- `tokenConfirmationsNeeded` - Confirmations before word is final
- `requiredSegmentsForConfirmation` - Segments needed
- `transcribeInterval` - Processing interval

### Hallucination Prevention

```
[EagerMode] Early stopping due to compression threshold: X
[EagerMode] Early stopping due to logprob threshold: X
```

---

## WebSocket Streaming Protocol

### Connection URL

From logs:
```
ws://localhost:53439/v1/listen?
    provider=argmax&
    model=am-parakeet-v3&
    channels=1&
    sample_rate=16000&
    encoding=linear16&
    diarize=true&
    punctuate=true&
    smart_format=true&
    numerals=true&
    filler_words=false&
    mip_opt_out=true&
    interim_results=true&
    multichannel=false&
    vad_events=false&
    redemption_time_ms=400&
    language=en
```

### Query Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `provider` | string | Provider name (argmax) |
| `model` | string | Model identifier |
| `channels` | int | Audio channels (1=mono) |
| `sample_rate` | int | Audio sample rate (16000) |
| `encoding` | string | Audio format (linear16) |
| `diarize` | bool | Enable speaker diarization |
| `punctuate` | bool | Add punctuation |
| `smart_format` | bool | Smart formatting |
| `numerals` | bool | Convert numbers |
| `filler_words` | bool | Include um, uh, etc. |
| `interim_results` | bool | Send partial results |
| `redemption_time_ms` | int | Silence threshold before finalizing |
| `language` | string | Language code |

### Message Types (Deepgram-compatible)

**Incoming (Client → Server)**:
- Binary frames: Raw PCM audio

**Outgoing (Server → Client)**:
```json
{
  "type": "Results",
  "channel_index": [0, 1],
  "is_final": false,
  "speech_final": false,
  "channel": {
    "alternatives": [{
      "transcript": "hello world",
      "confidence": 0.95,
      "words": [
        {"word": "hello", "start": 0.0, "end": 0.5, "confidence": 0.95},
        {"word": "world", "start": 0.6, "end": 1.0, "confidence": 0.94}
      ]
    }]
  }
}
```

**Event Types**:
- `Results` - Transcription results (interim or final)
- `SpeechStarted` - Speech detected
- `UtteranceEnd` - End of utterance
- `Error` - Error occurred

### Stream Modes

```swift
enum StreamMode {
    case alwaysOn           // Continuous transcription
    case voiceTriggered     // Only when speech detected (default)
    case batteryOptimized   // Aggressive power saving
}
```

---

## Why Argmax is Efficient

### 1. CoreML Compilation and Caching

```
[[ParakeetAudioEncoderPro]] Found cache at X
[[ParakeetAudioEncoderPro]] Used cache
[[ParakeetAudioEncoderPro]] Loaded compiled model on Y
```

- Models compiled to optimized CoreML format on first load
- Cached on disk for instant subsequent loads
- "Fast load" mode skips re-compilation

### 2. Compute Unit Optimization

```
Encoder Compute Units: cpuAndNeuralEngine
Decoder Compute Units: cpuAndGPU
Fast Load Encoder Compute Units: cpuAndNeuralEngine
```

- **Neural Engine (ANE)**: Best for batch inference, very power efficient
- **GPU (Metal)**: Best for streaming, lower latency
- **CPU**: Fallback, used for operations ANE can't handle

### 3. CTC Architecture (Parakeet)

```
ContextGraphCTC           // CTC with context-aware decoding
CTCEncodingTask          // Parallel CTC encoding
ctcAliTokenWeight        // Token weight optimization
```

**Why CTC is faster**:
1. No autoregressive decoder - single forward pass
2. Direct audio → tokens mapping
3. Parallelizable across time dimension
4. Lower memory footprint

### 4. Streaming Optimizations

```
AudioChunkerPro          // Optimized chunking
WindowAudioChunker       // Windowed processing
CausalEncodingCache      // Cached encoder states
```

**Techniques**:
- Causal encoding cache - reuse previous computations
- Overlapping windows - reduce latency
- Voice activity detection - skip silence

### 5. Custom Vocabulary (Keyword Boosting)

```
ContextGraphCTC          // Context-aware decoding
WordSpotter              // Keyword detection
customVocabularyModelFolder  // Custom vocabulary models
```

**How it works**:
1. Build finite state automaton from keywords
2. Bias CTC decoding towards keywords
3. Higher confidence for expected terms

### 6. Speaker Diarization Pipeline

```
SpeakerSegmenterModel → SpeakerEmbedderModel → VBxClustering
```

**Efficient design**:
- Segment audio by voice activity
- Extract embeddings per segment (not per frame)
- VBx clustering - scalable speaker clustering

---

## Key Insights for Implementation

### 1. Sidecar Architecture Benefits

**Advantages**:
- Language-native ML frameworks (Swift for CoreML)
- Process isolation (crash recovery)
- API flexibility (local or cloud)
- Independent scaling

**Implementation**:
```rust
// Spawn sidecar from Tauri
let sidecar = app.shell().sidecar("hyprnote-sidecar-stt")?;
let (rx, child) = sidecar.args(["serve", "--port", "53439"]).spawn()?;
```

### 2. Actor Pattern for Audio

**Benefits**:
- Clear separation of concerns
- Easy error recovery (supervisor restarts failed actors)
- Concurrent processing without shared state
- Backpressure handling

**Suggested structure**:
```rust
struct SessionSupervisor {
    source_actor: ActorRef<SourceActor>,
    listener_actor: ActorRef<ListenerActor>,
    recorder_actor: ActorRef<RecorderActor>,
}
```

### 3. WebSocket Protocol Design

**Deepgram-compatible benefits**:
- Well-documented protocol
- Easy to swap providers
- Existing client libraries

**Key features to implement**:
- Binary audio frames (16kHz, mono, linear16)
- JSON response messages
- `interim_results` for progressive updates
- `speech_final` vs `is_final` distinction

### 4. Progressive Transcription

**Two-phase results**:
```json
// Interim (hypothesis)
{"is_final": false, "speech_final": false, "transcript": "hello wor"}

// Final (confirmed)
{"is_final": true, "speech_final": true, "transcript": "hello world"}
```

**Implementation pattern**:
```rust
enum TranscriptState {
    Interim { text: String, confidence: f32 },
    Final { text: String, confidence: f32, words: Vec<Word> },
}
```

### 5. Redemption Time

From URL: `redemption_time_ms=400`

**Purpose**: Delay before finalizing after silence
- Too short: Words cut off mid-sentence
- Too long: Slow response time

**Recommended**: 300-500ms

### 6. Hardware Acceleration Strategy

**For Apple Silicon**:
```swift
// Optimal compute unit selection
let config = WhisperKitConfig(
    computeOptions: .init(
        melCompute: .cpuAndNeuralEngine,
        audioEncoderCompute: .cpuAndNeuralEngine,
        textDecoderCompute: .cpuAndGPU  // GPU for streaming
    )
)
```

**Fallback handling**:
```
[[TextDecoderPro]] Applied fallback to cpuAndGPU compute units
```

---

## Comparison: Hyprnote vs Meetily

| Feature | Hyprnote | Meetily (Current) |
|---------|----------|-------------------|
| Architecture | Sidecar (Swift) | Monolithic (Rust) |
| ML Runtime | CoreML (Swift) | whisper-rs (C++) |
| Streaming | WebSocket API | Direct function calls |
| Actor System | ractor (Rust) | Manual threading |
| Progressive | EagerMode | NutWhisper (WIP) |
| Diarization | Built-in | Not implemented |

### Recommendations for Meetily

1. **Consider sidecar pattern** for better ML integration
2. **Adopt actor system** (ractor or actix) for cleaner audio pipeline
3. **Implement EagerMode-style** progressive transcription
4. **Use WebSocket protocol** for flexibility
5. **Add hardware acceleration hints** for whisper-rs

---

## Appendix: Key Strings from Binary Analysis

### EagerMode/TranscribeStreamSession
```
[EagerMode] Buffer duration:
[EagerMode] Common words confirmed
[EagerMode] Not enough common words confirmed
[TranscribeStreamSession] Common prefix:
[TranscribeStreamSession] Confirming X words
[TranscribeStreamSession] Hypothesis:
```

### Model Loading
```
[[ParakeetAudioEncoderPro]] Loading via cold start on compute units X
[[ParakeetAudioEncoderPro]] Found cache at X
[[ParakeetAudioEncoderPro]] Preparing fast load
[[ParakeetAudioEncoderPro]] Used cache
```

### Speaker Diarization
```
[Diarizer] audioArray:
[Diarizer] Embedding total time:
[SpeakerEmbedder] Processing single chunk
[SpeakerSegmenter] Voice activity detection
```

### Error Handling
```
audio_buffer_overflow
mic_queue_overflow
spk_queue_overflow
aec_init_failed
aec_failed
listener_unavailable_buffering
```

---

## Server CLI Reference

```bash
# Start server with defaults
./hyprnote-sidecar-stt serve

# Custom port and model
./hyprnote-sidecar-stt serve \
  --port 8080 \
  --model nvidia_parakeet-v3_494MB \
  --stream-mode voiceTriggered

# With custom vocabulary boosting
./hyprnote-sidecar-stt serve \
  --custom-vocabulary "Meetily,Tauri,Whisper"

# Verbose logging
./hyprnote-sidecar-stt serve -v -d
```

---

*Document generated: February 2025*
*Source: Analysis of Hyprnote Nightly.app v1.0.3-nightly.2*
