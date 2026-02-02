//! NutWhisper - Nutshell-style streaming transcription provider
//!
//! Key differences from standard WhisperEngine:
//! 1. No external VAD - uses Whisper's internal no_speech_prob
//! 2. Time-based chunking with re-transcription
//! 3. Accumulated context for better accuracy
//! 4. Progressive display (partial → refined → final)
//! 5. set_no_context(true) to prevent hallucination loops

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};

/// Configuration for NutWhisper transcription
#[derive(Debug, Clone)]
pub struct NutWhisperConfig {
    /// Sample rate (must be 16000 for Whisper)
    pub sample_rate: u32,
    /// Chunk duration in seconds for partial transcription
    pub chunk_duration_secs: f32,
    /// Number of chunks before re-transcription with full context
    pub chunks_before_retranscribe: usize,
    /// No speech probability threshold (0.0 - 1.0)
    /// Higher = more aggressive silence filtering
    pub no_speech_threshold: f32,
    /// Maximum accumulated audio duration in seconds
    pub max_accumulation_secs: f32,
    /// Language code (e.g., "en", "auto")
    pub language: Option<String>,
    /// Maximum phrase duration in seconds before forcing a new phrase
    /// This ensures users see multiple segments during long continuous speech
    pub max_phrase_duration_secs: f32,
}

impl Default for NutWhisperConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            chunk_duration_secs: 2.0,         // 2 second chunks for partial
            chunks_before_retranscribe: 3,    // Re-transcribe every 3 chunks (6 seconds)
            no_speech_threshold: 0.65,        // Nutshell uses 0.65
            max_accumulation_secs: 30.0,      // Max 30 seconds accumulation
            language: Some("en".to_string()),
            max_phrase_duration_secs: 15.0,   // Force new phrase after 15 seconds of continuous speech
        }
    }
}

/// Transcription result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// The transcribed text (full text for this result)
    pub text: String,
    /// Whether this is a partial (chunk) or refined (accumulated) result
    pub is_partial: bool,
    /// Whether this is the final result for the phrase
    pub is_final: bool,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Start time in seconds (relative to phrase start)
    pub start_time: f32,
    /// End time in seconds
    pub end_time: f32,
    /// Individual words with timestamps (if available)
    pub words: Vec<WordInfo>,
    /// Audio source label (e.g., "microphone", "speaker")
    pub source: String,
    /// EagerMode: Confirmed text (stable, won't change)
    #[serde(default)]
    pub confirmed_text: Option<String>,
    /// EagerMode: Hypothesis text (may change with next result)
    #[serde(default)]
    pub hypothesis_text: Option<String>,
    /// EagerMode: Whether this result includes newly confirmed words
    #[serde(default)]
    pub has_new_confirmed: bool,
    /// Phrase ID for streaming updates (same phrase_id = replace, new phrase_id = new entry)
    /// This enables Nutshell-style in-place text updates
    #[serde(default)]
    pub phrase_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordInfo {
    pub word: String,
    pub start: f32,
    pub end: f32,
    pub probability: f32,
}

/// EagerMode state for progressive word confirmation (from MacWhisper/Hyprnote pattern)
///
/// The algorithm compares consecutive transcriptions and confirms words that
/// appear consistently in a common prefix, providing stable output while
/// allowing hypothesis text to update freely.
#[derive(Debug, Clone)]
pub struct EagerModeState {
    /// Previously transcribed words for comparison
    previous_words: Vec<String>,
    /// Words that have been confirmed (stable, won't change)
    confirmed_words: Vec<WordInfo>,
    /// Text that has been confirmed
    confirmed_text: String,
    /// Minimum words in common prefix to trigger confirmation
    min_common_prefix_count: usize,
}

impl Default for EagerModeState {
    fn default() -> Self {
        Self {
            previous_words: Vec::new(),
            confirmed_words: Vec::new(),
            confirmed_text: String::new(),
            min_common_prefix_count: 3, // From Argmax/MacWhisper
        }
    }
}

/// Output from EagerMode processing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EagerModeOutput {
    /// Words were confirmed (stable)
    Confirmed {
        confirmed_text: String,
        hypothesis_text: String,
        confirmed_words: Vec<WordInfo>,
        hypothesis_words: Vec<WordInfo>,
    },
    /// Only hypothesis available (may change)
    Hypothesis {
        text: String,
        words: Vec<WordInfo>,
    },
}

impl EagerModeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a new transcription result and determine confirmed vs hypothesis text
    pub fn process(&mut self, text: &str, words: Vec<WordInfo>) -> EagerModeOutput {
        let current_words: Vec<String> = text
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        if current_words.is_empty() {
            return EagerModeOutput::Hypothesis {
                text: String::new(),
                words: Vec::new(),
            };
        }

        // Find common prefix between previous and current transcription
        let common_count = self.find_common_prefix(&current_words);

        if common_count >= self.min_common_prefix_count {
            // Confirm the common prefix words
            let original_words: Vec<&str> = text.split_whitespace().collect();
            let confirmed_new: Vec<&str> = original_words[..common_count].to_vec();
            let hypothesis_new: Vec<&str> = original_words[common_count..].to_vec();

            let confirmed_text_new = confirmed_new.join(" ");
            let hypothesis_text = hypothesis_new.join(" ");

            // Append to total confirmed text
            if !self.confirmed_text.is_empty() && !confirmed_text_new.is_empty() {
                self.confirmed_text.push(' ');
            }
            self.confirmed_text.push_str(&confirmed_text_new);

            // Split words into confirmed and hypothesis
            let (confirmed_words, hypothesis_words) = if !words.is_empty() {
                let split_point = common_count.min(words.len());
                (words[..split_point].to_vec(), words[split_point..].to_vec())
            } else {
                (Vec::new(), Vec::new())
            };

            self.confirmed_words.extend(confirmed_words.clone());

            // Update previous words for next comparison
            self.previous_words = current_words;

            EagerModeOutput::Confirmed {
                confirmed_text: confirmed_text_new,
                hypothesis_text,
                confirmed_words,
                hypothesis_words,
            }
        } else {
            // Not enough common prefix, treat everything as hypothesis
            self.previous_words = current_words;

            EagerModeOutput::Hypothesis {
                text: text.to_string(),
                words,
            }
        }
    }

    /// Find the length of the common prefix between previous and current words
    fn find_common_prefix(&self, current: &[String]) -> usize {
        let mut count = 0;
        for (prev, curr) in self.previous_words.iter().zip(current.iter()) {
            if prev == curr {
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    /// Get all confirmed text so far
    pub fn get_confirmed_text(&self) -> &str {
        &self.confirmed_text
    }

    /// Get all confirmed words so far
    pub fn get_confirmed_words(&self) -> &[WordInfo] {
        &self.confirmed_words
    }

    /// Reset state for a new phrase/utterance
    pub fn reset(&mut self) {
        self.previous_words.clear();
        self.confirmed_words.clear();
        self.confirmed_text.clear();
    }

    /// Build a prompt from confirmed words for Whisper (prefix prompting)
    pub fn build_prompt(&self, max_words: usize) -> Option<String> {
        if self.confirmed_words.is_empty() {
            return None;
        }

        let prompt: String = self.confirmed_words
            .iter()
            .rev()
            .take(max_words)
            .map(|w| w.word.as_str())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" ");

        if prompt.is_empty() {
            None
        } else {
            Some(prompt)
        }
    }
}

/// Audio chunk for NutWhisper processing
#[derive(Debug, Clone)]
pub struct NutAudioChunk {
    pub samples: Vec<f32>,
    pub source: String,
    pub timestamp: f64,
}

/// NutWhisper streaming transcription provider
///
/// Implements Nutshell-style progressive transcription:
/// 1. Receive audio continuously (no VAD gating)
/// 2. Transcribe small chunks for immediate feedback (partial)
/// 3. Accumulate audio and re-transcribe for better accuracy (refined)
/// 4. Finalize on silence or timeout (final)
pub struct NutWhisper {
    config: NutWhisperConfig,
    context: Arc<RwLock<Option<WhisperContext>>>,
    /// Accumulated audio samples for re-transcription
    accumulated_audio: Arc<RwLock<Vec<f32>>>,
    /// Number of chunks processed since last re-transcription
    chunk_count: Arc<RwLock<usize>>,
    /// Current phrase start time
    phrase_start_time: Arc<RwLock<f64>>,
    /// Track consecutive silence chunks for phrase finalization
    silence_chunk_count: Arc<RwLock<usize>>,
    /// Last transcription text (for deduplication)
    last_text: Arc<RwLock<String>>,
    /// EagerMode state for word confirmation
    eager_mode: Arc<RwLock<EagerModeState>>,
    /// Current phrase ID - increments only when phrase is finalized (Nutshell-style streaming)
    /// Same phrase_id means UI should REPLACE, new phrase_id means new entry
    current_phrase_id: Arc<RwLock<u64>>,
}

impl NutWhisper {
    /// Create a new NutWhisper instance
    pub fn new(config: NutWhisperConfig) -> Self {
        Self {
            config,
            context: Arc::new(RwLock::new(None)),
            accumulated_audio: Arc::new(RwLock::new(Vec::new())),
            chunk_count: Arc::new(RwLock::new(0)),
            phrase_start_time: Arc::new(RwLock::new(0.0)),
            silence_chunk_count: Arc::new(RwLock::new(0)),
            last_text: Arc::new(RwLock::new(String::new())),
            eager_mode: Arc::new(RwLock::new(EagerModeState::new())),
            current_phrase_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Load a Whisper model
    pub async fn load_model(&self, model_path: &str) -> Result<()> {
        log::info!("[NutWhisper] Loading model from: {}", model_path);

        let path = model_path.to_string();
        let ctx = tokio::task::spawn_blocking(move || {
            let params = WhisperContextParameters::default();
            WhisperContext::new_with_params(&path, params)
        })
        .await?
        .map_err(|e| anyhow!("Failed to load Whisper model: {:?}", e))?;

        *self.context.write().await = Some(ctx);
        log::info!("[NutWhisper] Model loaded successfully");
        Ok(())
    }

    /// Process an audio chunk and return transcription result(s)
    ///
    /// This implements the Nutshell-style progressive transcription:
    /// - Always returns a partial result for immediate display
    /// - Periodically returns a refined result with accumulated context
    /// - Returns final result when phrase is complete
    pub async fn process_chunk(&self, chunk: NutAudioChunk) -> Result<Vec<TranscriptionResult>> {
        log::info!("[NutWhisper] process_chunk called with {} samples", chunk.samples.len());
        let mut results = Vec::new();

        // Check if model is loaded (quick check, then release lock)
        {
            let ctx_guard = self.context.read().await;
            if ctx_guard.is_none() {
                return Err(anyhow!("Whisper model not loaded"));
            }
        }
        // Lock is now released - transcribe_audio will acquire its own lock

        // Calculate chunk duration
        let chunk_samples = chunk.samples.len();
        let chunk_duration = chunk_samples as f32 / self.config.sample_rate as f32;

        // Check if audio has enough energy to be worth transcribing
        if !Self::has_sufficient_energy(&chunk.samples) {
            log::debug!("[NutWhisper] Skipping chunk - insufficient audio energy");
            // Still accumulate for context, but don't transcribe this chunk
            {
                let mut accumulated = self.accumulated_audio.write().await;
                accumulated.extend_from_slice(&chunk.samples);
            }
            return Ok(results);
        }

        // 1. Transcribe the current chunk (PARTIAL)
        let mut partial_result = self.transcribe_audio(
            &chunk.samples,
            chunk.source.clone(),
            true, // is_partial
        ).await?;

        // Apply hallucination filter to ALL results (including partial)
        partial_result.text = Self::clean_repetitive_text(&partial_result.text);

        // Check if this chunk is silence using Whisper's no_speech_prob
        // Also treat empty text (filtered hallucination) as silence
        let is_silence = partial_result.confidence < (1.0 - self.config.no_speech_threshold)
            || partial_result.text.is_empty();

        if is_silence {
            // Track consecutive silence - get count and release lock immediately to avoid deadlock
            let should_finalize = {
                let mut silence_count = self.silence_chunk_count.write().await;
                *silence_count += 1;
                *silence_count >= 2
            }; // Lock released here

            // If we have 2+ consecutive silence chunks, finalize the phrase
            if should_finalize {
                // Clone accumulated audio to release lock before transcription
                let accumulated = {
                    let guard = self.accumulated_audio.read().await;
                    guard.clone()
                };

                if !accumulated.is_empty() {
                    // Final re-transcription with all accumulated audio
                    let final_result = self.transcribe_audio(
                        &accumulated,
                        chunk.source.clone(),
                        false, // not partial
                    ).await?;

                    let mut final_result = final_result;
                    final_result.is_final = true;

                    // Clean up repetitive text
                    final_result.text = Self::clean_repetitive_text(&final_result.text);

                    if !final_result.text.is_empty() {
                        // For final result, include all confirmed text from EagerMode
                        let all_confirmed = {
                            let eager = self.eager_mode.read().await;
                            eager.get_confirmed_text().to_string()
                        };

                        // The final text is the authoritative version
                        // Set confirmed_text to the full final text (everything is now confirmed)
                        final_result.confirmed_text = Some(final_result.text.clone());
                        final_result.hypothesis_text = None;  // No more hypothesis
                        final_result.has_new_confirmed = true;

                        log::info!("[NutWhisper] FINAL result: '{}' (previous confirmed: '{}')",
                                  final_result.text, all_confirmed);

                        results.push(final_result);
                    }

                    // Reset state for next phrase (no deadlock now - silence_count lock was released)
                    self.reset_phrase().await;
                }
                return Ok(results);
            }
        } else {
            // Reset silence counter on speech
            *self.silence_chunk_count.write().await = 0;

            // DON'T emit partial results to avoid UI clutter
            // Partial results are from individual 2-second chunks - they're incomplete
            // and will be superseded by refined results from accumulated audio
            // We only use them for silence detection above
            log::debug!("[NutWhisper] Partial (not emitted): '{}'", partial_result.text);
        }

        // 2. Accumulate audio for re-transcription
        let accumulated_duration_secs = {
            let mut accumulated = self.accumulated_audio.write().await;
            accumulated.extend_from_slice(&chunk.samples);

            // Limit accumulation to max duration
            let max_samples = (self.config.max_accumulation_secs * self.config.sample_rate as f32) as usize;
            if accumulated.len() > max_samples {
                // Keep only the latest audio
                let start = accumulated.len() - max_samples;
                *accumulated = accumulated[start..].to_vec();
            }

            accumulated.len() as f32 / self.config.sample_rate as f32
        };

        // 2b. Check if phrase duration exceeded - force new phrase for better UX
        // This ensures users see multiple segments during long continuous speech
        if accumulated_duration_secs >= self.config.max_phrase_duration_secs {
            log::info!("[NutWhisper] ⏱️ Phrase duration exceeded ({:.1}s >= {:.1}s), forcing new phrase",
                      accumulated_duration_secs, self.config.max_phrase_duration_secs);

            // Clone accumulated audio before reset
            let accumulated = {
                let guard = self.accumulated_audio.read().await;
                guard.clone()
            };

            if !accumulated.is_empty() {
                // Transcribe accumulated audio as final for this phrase
                let final_result = self.transcribe_audio(
                    &accumulated,
                    chunk.source.clone(),
                    false,
                ).await?;

                let mut final_result = final_result;
                final_result.is_final = true;
                final_result.text = Self::clean_repetitive_text(&final_result.text);

                if !final_result.text.is_empty() {
                    // Mark all text as confirmed for final result
                    final_result.confirmed_text = Some(final_result.text.clone());
                    final_result.hypothesis_text = None;
                    final_result.has_new_confirmed = true;

                    log::info!("[NutWhisper] 📝 Time-based phrase finalized (phrase_id={}): '{}'",
                              final_result.phrase_id, final_result.text);
                    results.push(final_result);
                }
            }

            // Reset for new phrase (this increments phrase_id)
            self.reset_phrase().await;

            // Return early - next chunk will start a fresh phrase
            return Ok(results);
        }

        // 3. Increment chunk count and check for re-transcription
        let should_retranscribe = {
            let mut count = self.chunk_count.write().await;
            *count += 1;
            *count >= self.config.chunks_before_retranscribe
        };

        // 4. Re-transcribe with accumulated context (REFINED)
        if should_retranscribe && !is_silence {
            // Clone accumulated audio to release lock before transcription
            let accumulated = {
                let guard = self.accumulated_audio.read().await;
                guard.clone()
            };

            if accumulated.len() > (self.config.sample_rate as usize) {  // At least 1 second
                let refined_result = self.transcribe_audio(
                    &accumulated,
                    chunk.source.clone(),
                    false, // not partial - this is refined
                ).await?;

                let mut refined_result = refined_result;
                refined_result.text = Self::clean_repetitive_text(&refined_result.text);

                if !refined_result.text.is_empty() {
                    // Check it's different from last - read then drop lock before writing
                    let is_different = {
                        let last = self.last_text.read().await;
                        refined_result.text.trim() != last.trim()
                    };

                    if is_different {
                        // Apply EagerMode for refined results too
                        let eager_output = {
                            let mut eager = self.eager_mode.write().await;
                            eager.process(&refined_result.text, refined_result.words.clone())
                        };

                        match eager_output {
                            EagerModeOutput::Confirmed {
                                confirmed_text,
                                hypothesis_text,
                                confirmed_words: _,
                                hypothesis_words,
                            } => {
                                log::info!("[NutWhisper] ✅ EagerMode CONFIRMED: '{}' | hypothesis: '{}'",
                                          confirmed_text, hypothesis_text);
                                refined_result.confirmed_text = Some(confirmed_text);
                                refined_result.hypothesis_text = if hypothesis_text.is_empty() { None } else { Some(hypothesis_text) };
                                refined_result.has_new_confirmed = true;
                                refined_result.words = hypothesis_words;
                            }
                            EagerModeOutput::Hypothesis { text, words } => {
                                log::info!("[NutWhisper] 📝 EagerMode hypothesis (waiting for confirmation): '{}'", text);
                                refined_result.hypothesis_text = Some(text);
                                refined_result.confirmed_text = None;
                                refined_result.has_new_confirmed = false;
                                refined_result.words = words;
                            }
                        }

                        results.push(refined_result.clone());
                        *self.last_text.write().await = refined_result.text;
                    }
                }
            }

            // Reset chunk count
            *self.chunk_count.write().await = 0;
        }

        log::info!("[NutWhisper] process_chunk returning {} results", results.len());
        for r in &results {
            log::info!("[NutWhisper] Result: '{}' partial={} final={} phrase_id={}",
                      r.text, r.is_partial, r.is_final, r.phrase_id);
        }
        Ok(results)
    }

    /// Async wrapper for transcription
    /// NOTE: Do NOT hold any locks when calling this - it will acquire its own lock
    async fn transcribe_audio(
        &self,
        audio: &[f32],
        source: String,
        is_partial: bool,
    ) -> Result<TranscriptionResult> {
        let audio = audio.to_vec();
        let audio_len = audio.len();
        let language = self.config.language.clone();
        let no_speech_threshold = self.config.no_speech_threshold;
        let ctx = self.context.clone();

        // Run transcription in blocking task
        // IMPORTANT: We acquire the lock inside spawn_blocking to avoid deadlock
        let (text, confidence, words) = tokio::task::spawn_blocking(move || {
            let ctx_guard = ctx.blocking_read();
            let ctx_ref = ctx_guard.as_ref()
                .ok_or_else(|| anyhow!("Model not loaded"))?;
            Self::transcribe_audio_sync(ctx_ref, &audio, language, no_speech_threshold)
        }).await??;

        // Get current phrase_id
        let phrase_id = *self.current_phrase_id.read().await;

        Ok(TranscriptionResult {
            text,
            is_partial,
            is_final: false,
            confidence,
            start_time: 0.0,
            end_time: audio_len as f32 / 16000.0,
            words,  // Now populated with actual word timestamps
            source,
            confirmed_text: None,
            hypothesis_text: None,
            has_new_confirmed: false,
            phrase_id,
        })
    }

    /// Synchronous transcription with Nutshell-style parameters
    /// Returns (text, confidence, words)
    fn transcribe_audio_sync(
        ctx: &WhisperContext,
        audio: &[f32],
        language: Option<String>,
        no_speech_threshold: f32,
    ) -> Result<(String, f32, Vec<WordInfo>)> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // === NUTSHELL KEY SETTINGS ===

        // 1. CRITICAL: Disable context from previous transcription
        // This is the equivalent of condition_on_previous_text=False
        params.set_no_context(true);

        // 2. Set language
        if let Some(lang) = &language {
            params.set_language(Some(lang));
        }

        // 3. No speech threshold (Nutshell uses 0.65)
        params.set_no_speech_thold(no_speech_threshold);

        // 4. Suppress non-speech tokens and blanks
        params.set_suppress_blank(true);
        params.set_suppress_non_speech_tokens(true);

        // 5. Single segment mode for streaming chunks
        params.set_single_segment(true);

        // 6. Disable printing (we handle output ourselves)
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);

        // 7. Token timestamps for word-level info
        params.set_token_timestamps(true);

        // 8. Temperature and other sampling params
        params.set_temperature(0.0);  // Greedy decoding
        params.set_temperature_inc(0.2);
        params.set_entropy_thold(2.4);
        params.set_logprob_thold(-1.0);

        // 9. Max tokens to prevent runaway generation
        params.set_max_len(100);

        // Run inference
        let mut state = ctx.create_state()
            .map_err(|e| anyhow!("Failed to create state: {:?}", e))?;

        state.full(params, audio)
            .map_err(|e| anyhow!("Transcription failed: {:?}", e))?;

        // Extract results
        let num_segments = state.full_n_segments()
            .map_err(|e| anyhow!("Failed to get segments: {:?}", e))?;

        let mut text = String::new();
        let mut total_prob = 0.0;
        let mut token_count = 0;
        let mut words = Vec::new();

        for i in 0..num_segments {
            if let Ok(segment_text) = state.full_get_segment_text(i) {
                let trimmed = segment_text.trim();
                if !trimmed.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(trimmed);
                }
            }

            // Extract word-level timestamps and probabilities
            if let Ok(n_tokens) = state.full_n_tokens(i) {
                for t in 0..n_tokens {
                    if let Ok(token_data) = state.full_get_token_data(i, t) {
                        total_prob += token_data.p;
                        token_count += 1;

                        // Extract word text and timestamps
                        if let Ok(token_text) = state.full_get_token_text(i, t) {
                            let word_text = token_text.trim();
                            // Filter out special tokens like [BLANK], <|endoftext|>, etc.
                            if !word_text.is_empty()
                                && !word_text.starts_with('[')
                                && !word_text.starts_with('<')
                                && !word_text.starts_with('(')
                            {
                                words.push(WordInfo {
                                    word: word_text.to_string(),
                                    // t0/t1 are in centiseconds, convert to seconds
                                    start: token_data.t0 as f32 / 100.0,
                                    end: token_data.t1 as f32 / 100.0,
                                    probability: token_data.p,
                                });
                            }
                        }
                    }
                }
            }
        }

        let confidence = if token_count > 0 {
            total_prob / token_count as f32
        } else {
            0.0
        };

        log::debug!("[NutWhisper] Extracted {} words from transcription", words.len());

        Ok((text, confidence, words))
    }

    /// Reset state for a new phrase
    async fn reset_phrase(&self) {
        *self.accumulated_audio.write().await = Vec::new();
        *self.chunk_count.write().await = 0;
        *self.silence_chunk_count.write().await = 0;
        *self.last_text.write().await = String::new();
        *self.phrase_start_time.write().await = 0.0;
        self.eager_mode.write().await.reset();
        // Increment phrase_id for next phrase (Nutshell-style: new phrase = new entry)
        let mut phrase_id = self.current_phrase_id.write().await;
        *phrase_id += 1;
        log::info!("[NutWhisper] 🆕 New phrase started with phrase_id={}", *phrase_id);
    }

    /// Flush any remaining audio and get final transcription
    pub async fn flush(&self) -> Result<Option<TranscriptionResult>> {
        // Check if model is loaded (quick check, then release lock)
        {
            let ctx_guard = self.context.read().await;
            if ctx_guard.is_none() {
                return Ok(None);
            }
        }

        // Clone accumulated audio to release the lock before transcription
        let accumulated = {
            let guard = self.accumulated_audio.read().await;
            if guard.len() < (self.config.sample_rate as usize / 2) {
                // Less than 0.5 seconds, not worth transcribing
                return Ok(None);
            }
            guard.clone()
        };

        let result = self.transcribe_audio(
            &accumulated,
            "flush".to_string(),
            false,
        ).await?;

        let mut result = result;
        result.is_final = true;
        result.text = Self::clean_repetitive_text(&result.text);

        // For flush, everything becomes confirmed
        if !result.text.is_empty() {
            result.confirmed_text = Some(result.text.clone());
            result.hypothesis_text = None;
            result.has_new_confirmed = true;
        }

        // Get current phrase_id before reset (final result belongs to current phrase)
        result.phrase_id = *self.current_phrase_id.read().await;

        self.reset_phrase().await;

        if result.text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// Clean repetitive/hallucinated text (from Nutshell)
    fn clean_repetitive_text(text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }

        // Normalize: lowercase and remove punctuation for comparison
        let text_normalized: String = text
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();

        // Nutshell's specific hallucination string
        let nutshell_hallucination = "alittlebitofalittlebitofalittlebitofalittlebitof";
        let text_no_spaces: String = text_normalized.chars().filter(|c| !c.is_whitespace()).collect();
        if text_no_spaces.contains(nutshell_hallucination) {
            log::debug!("[NutWhisper] Filtered Nutshell hallucination pattern");
            return String::new();
        }

        // Common Whisper hallucinations (case-insensitive, punctuation-stripped)
        let hallucination_patterns = [
            "thank you for watching",
            "thanks for watching",
            "like and subscribe",
            "please subscribe",
            "see you next time",
            "bye",
            "goodbye",
            "thank you",
            "thanks",
            "you",  // Single word "you" is often a hallucination
            "i",    // Single word "i" is often a hallucination
            "and",  // Single word "and" is often a hallucination
            "the",  // Single word "the" is often a hallucination
            "is",   // Single word "is" is often a hallucination
            "to",   // Single word "to" is often a hallucination
        ];

        // Check if entire text matches a hallucination pattern
        let text_trimmed = text_normalized.trim();
        log::info!("[NutWhisper] Hallucination check: normalized='{}', trimmed='{}'", text_normalized, text_trimmed);
        for pattern in &hallucination_patterns {
            if text_trimmed == *pattern {
                log::info!("[NutWhisper] 🚫 FILTERED exact hallucination match: '{}' == '{}'", text, pattern);
                return String::new();
            }
        }

        // Check if text contains hallucination phrases (for longer texts)
        for pattern in &["thank you for watching", "thanks for watching", "like and subscribe"] {
            if text_normalized.contains(pattern) {
                log::info!("[NutWhisper] 🚫 FILTERED hallucination phrase in: '{}'", text);
                return String::new();
            }
        }

        // Check for word repetition (same word 4+ times consecutively)
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() >= 4 {
            let mut repeat_count = 1;
            for i in 1..words.len() {
                let word_a = words[i].to_lowercase();
                let word_b = words[i-1].to_lowercase();
                // Strip punctuation for comparison
                let word_a_clean: String = word_a.chars().filter(|c| c.is_alphanumeric()).collect();
                let word_b_clean: String = word_b.chars().filter(|c| c.is_alphanumeric()).collect();

                if word_a_clean == word_b_clean && !word_a_clean.is_empty() {
                    repeat_count += 1;
                    if repeat_count >= 4 {
                        log::debug!("[NutWhisper] Filtered repeated word: '{}'", text);
                        return String::new();
                    }
                } else {
                    repeat_count = 1;
                }
            }
        }

        // Filter very short responses (likely noise)
        if words.len() <= 2 && text.len() < 10 {
            log::debug!("[NutWhisper] Filtered too-short text: '{}'", text);
            return String::new();
        }

        text.trim().to_string()
    }

    /// Check if audio has enough energy to be worth transcribing
    fn has_sufficient_energy(audio: &[f32]) -> bool {
        if audio.is_empty() {
            return false;
        }

        // Calculate RMS energy
        let sum_squares: f32 = audio.iter().map(|&x| x * x).sum();
        let rms = (sum_squares / audio.len() as f32).sqrt();

        // Threshold for minimum energy (empirically determined)
        // Values below this are likely silence/noise
        const MIN_RMS_THRESHOLD: f32 = 0.005;

        let has_energy = rms > MIN_RMS_THRESHOLD;
        if !has_energy {
            log::debug!("[NutWhisper] Audio energy too low: RMS={:.6} < threshold={}", rms, MIN_RMS_THRESHOLD);
        }
        has_energy
    }
}

/// Streaming pipeline that wraps NutWhisper for continuous audio processing
pub struct NutWhisperPipeline {
    whisper: Arc<NutWhisper>,
    /// Channel to send audio chunks
    audio_tx: mpsc::UnboundedSender<NutAudioChunk>,
    /// Channel to receive transcription results
    result_rx: Arc<RwLock<mpsc::UnboundedReceiver<TranscriptionResult>>>,
}

impl NutWhisperPipeline {
    /// Create a new streaming pipeline
    pub fn new(config: NutWhisperConfig) -> Self {
        let whisper = Arc::new(NutWhisper::new(config.clone()));
        let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<NutAudioChunk>();
        let (result_tx, result_rx) = mpsc::unbounded_channel::<TranscriptionResult>();

        let whisper_clone = whisper.clone();

        // Spawn processing task
        tokio::spawn(async move {
            log::info!("[NutWhisper] 🚀 Internal processing task started");

            let sample_rate = config.sample_rate;
            let chunk_duration = config.chunk_duration_secs;
            let samples_per_chunk = (sample_rate as f32 * chunk_duration) as usize;

            log::info!("[NutWhisper] Config: sample_rate={}, chunk_duration={}s, samples_per_chunk={}",
                      sample_rate, chunk_duration, samples_per_chunk);

            let mut audio_buffer: Vec<f32> = Vec::new();
            let mut current_source = String::new();
            let mut chunks_received: u64 = 0;
            let mut chunks_processed: u64 = 0;
            // Track total samples processed for accurate timestamp calculation
            let mut total_samples_processed: u64 = 0;
            let mut first_timestamp: Option<f64> = None;

            while let Some(chunk) = audio_rx.recv().await {
                chunks_received += 1;
                current_source = chunk.source.clone();
                audio_buffer.extend_from_slice(&chunk.samples);

                // Track the first timestamp as our reference point
                if first_timestamp.is_none() {
                    first_timestamp = Some(chunk.timestamp);
                }

                // Log buffer status every 10 chunks
                if chunks_received % 10 == 0 {
                    log::info!("[NutWhisper] 📊 Buffer status: {} samples ({:.1}s), need {} for processing",
                              audio_buffer.len(),
                              audio_buffer.len() as f32 / sample_rate as f32,
                              samples_per_chunk);
                }

                // Process when we have enough samples
                while audio_buffer.len() >= samples_per_chunk {
                    let chunk_samples: Vec<f32> = audio_buffer.drain(..samples_per_chunk).collect();
                    chunks_processed += 1;

                    // Calculate actual timestamp based on samples processed
                    let chunk_start_time = total_samples_processed as f64 / sample_rate as f64;
                    let chunk_duration_secs = chunk_samples.len() as f64 / sample_rate as f64;
                    let chunk_end_time = chunk_start_time + chunk_duration_secs;

                    log::info!("[NutWhisper] 🔄 Processing chunk #{} ({} samples = {}s of audio, time: {:.1}s-{:.1}s)",
                              chunks_processed, chunk_samples.len(),
                              chunk_samples.len() as f32 / sample_rate as f32,
                              chunk_start_time, chunk_end_time);

                    let audio_chunk = NutAudioChunk {
                        samples: chunk_samples.clone(),
                        source: current_source.clone(),
                        timestamp: chunk_start_time,  // Use calculated timestamp
                    };

                    match whisper_clone.process_chunk(audio_chunk).await {
                        Ok(results) => {
                            log::info!("[NutWhisper] ✅ Chunk #{} produced {} results", chunks_processed, results.len());
                            for (i, result) in results.iter().enumerate() {
                                // Update timestamps in the result
                                let mut result = result.clone();
                                result.start_time = chunk_start_time as f32;
                                result.end_time = chunk_end_time as f32;

                                log::info!("[NutWhisper] 📤 Sending result {}/{} to channel: '{}' (partial={}, final={}, time: {:.1}s-{:.1}s)",
                                          i + 1, results.len(), result.text, result.is_partial, result.is_final,
                                          result.start_time, result.end_time);
                                if let Err(e) = result_tx.send(result) {
                                    log::error!("[NutWhisper] ❌ FAILED to send result to channel: {}", e);
                                } else {
                                    log::info!("[NutWhisper] ✅ Result sent to channel successfully");
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("[NutWhisper] ❌ Processing error on chunk #{}: {}", chunks_processed, e);
                        }
                    }

                    // Update total samples processed
                    total_samples_processed += chunk_samples.len() as u64;
                }
            }

            log::info!("[NutWhisper] 🏁 Audio channel closed. Total: {} chunks received, {} processed",
                      chunks_received, chunks_processed);

            // Flush remaining audio
            if let Ok(Some(result)) = whisper_clone.flush().await {
                log::info!("[NutWhisper] 📤 Flushing final result: '{}'", result.text);
                let _ = result_tx.send(result);
            }

            log::info!("[NutWhisper] 🛑 Internal processing task ended");
        });

        Self {
            whisper,
            audio_tx,
            result_rx: Arc::new(RwLock::new(result_rx)),
        }
    }

    /// Load a Whisper model
    pub async fn load_model(&self, model_path: &str) -> Result<()> {
        self.whisper.load_model(model_path).await
    }

    /// Send audio for processing
    pub fn send_audio(&self, samples: Vec<f32>, source: &str, timestamp: f64) -> Result<()> {
        let chunk = NutAudioChunk {
            samples,
            source: source.to_string(),
            timestamp,
        };
        self.audio_tx.send(chunk)
            .map_err(|e| anyhow!("Failed to send audio: {}", e))
    }

    /// Try to receive a transcription result (non-blocking)
    pub async fn try_recv(&self) -> Option<TranscriptionResult> {
        let mut rx = self.result_rx.write().await;
        match rx.try_recv() {
            Ok(result) => {
                log::info!("[NutWhisper] try_recv: Got result from channel: '{}'", result.text);
                Some(result)
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                // Normal - no results available yet
                None
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                log::warn!("[NutWhisper] try_recv: Channel disconnected!");
                None
            }
        }
    }

    /// Receive a transcription result (blocking)
    pub async fn recv(&self) -> Option<TranscriptionResult> {
        let mut rx = self.result_rx.write().await;
        rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_repetitive_text() {
        // Test hallucination patterns
        assert_eq!(
            NutWhisper::clean_repetitive_text("Thank you for watching"),
            ""
        );

        // Test word repetition
        assert_eq!(
            NutWhisper::clean_repetitive_text("the the the the the"),
            ""
        );

        // Test normal text
        assert_eq!(
            NutWhisper::clean_repetitive_text("Hello, how are you?"),
            "Hello, how are you?"
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = NutWhisperConfig::default();
        assert_eq!(config.sample_rate, 16000);
        assert_eq!(config.chunk_duration_secs, 2.0);
        assert_eq!(config.no_speech_threshold, 0.65);
    }
}
