#!/usr/bin/env python3
"""
LFM2.5-Audio Sidecar for Meetily
Handles audio transcription using Liquid AI's LFM2.5-Audio-1.5B model.

Protocol: JSON-over-stdin/stdout (one JSON object per line)

Requests:
    {"type": "load_model", "model_id": "LiquidAI/LFM2.5-Audio-1.5B"}
    {"type": "transcribe", "audio_path": "/path/to/audio.wav"}
    {"type": "transcribe_samples", "samples": [...], "sample_rate": 16000}
    {"type": "unload"}
    {"type": "ping"}
    {"type": "shutdown"}

Responses:
    {"type": "response", "text": "...", "error": null}
    {"type": "pong"}
    {"type": "goodbye"}
    {"type": "error", "message": "..."}
"""

import json
import sys
import os
import warnings

# Suppress warnings for cleaner output
warnings.filterwarnings("ignore")
os.environ['TOKENIZERS_PARALLELISM'] = 'false'

import torch
import numpy as np


class LFMAudioEngine:
    """LFM2.5-Audio transcription engine."""

    def __init__(self):
        self.model = None
        self.processor = None
        self.device = None
        self.model_id = None

    def _detect_device(self) -> str:
        """Detect best available device."""
        if torch.cuda.is_available():
            return "cuda"
        elif hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
            return "mps"
        return "cpu"

    def load_model(self, model_id: str = "LiquidAI/LFM2.5-Audio-1.5B") -> dict:
        """Load the LFM2.5-Audio model."""
        try:
            from liquid_audio import LFM2AudioModel, LFM2AudioProcessor

            self.device = self._detect_device()
            self.model_id = model_id

            log_stderr(f"Loading LFM model on {self.device}...")

            # IMPORTANT: liquid-audio defaults to "cuda" - we must pass device explicitly
            # See: https://github.com/Liquid4All/liquid-audio/issues/9 (MLX support request)
            # The library has hardcoded .cuda() calls that fail on Mac

            # Load processor with explicit device (avoids cuda default)
            self.processor = LFM2AudioProcessor.from_pretrained(
                model_id,
                device=self.device
            ).eval()

            # Load model with explicit device (library defaults to cuda)
            # Use float16 for MPS, bfloat16 for CUDA, float32 for CPU
            if self.device == "mps":
                dtype = torch.float16
            elif self.device == "cuda":
                dtype = torch.bfloat16
            else:
                dtype = torch.float32

            self.model = LFM2AudioModel.from_pretrained(
                model_id,
                device=self.device,
                dtype=dtype
            ).eval()

            log_stderr(f"LFM model loaded successfully on {self.device}")
            return {"type": "response", "text": "Model loaded", "error": None}

        except Exception as e:
            import traceback
            log_stderr(f"Failed to load model: {e}")
            log_stderr(traceback.format_exc())
            return {"type": "response", "text": "", "error": str(e)}

    def transcribe_file(self, audio_path: str) -> dict:
        """Transcribe audio from a file."""
        try:
            if self.model is None:
                return {"type": "response", "text": "", "error": "Model not loaded"}

            import torchaudio
            from liquid_audio import ChatState

            # Load audio
            waveform, sample_rate = torchaudio.load(audio_path)

            # Convert to mono if stereo
            if waveform.shape[0] > 1:
                waveform = waveform.mean(dim=0, keepdim=True)

            # Resample to 16kHz if needed
            if sample_rate != 16000:
                resampler = torchaudio.transforms.Resample(sample_rate, 16000)
                waveform = resampler(waveform)
                sample_rate = 16000

            # Transcribe using chat interface
            text = self._transcribe_waveform(waveform.squeeze().numpy(), sample_rate)

            return {"type": "response", "text": text, "error": None}

        except Exception as e:
            log_stderr(f"Transcription failed: {e}")
            return {"type": "response", "text": "", "error": str(e)}

    def transcribe_samples(self, samples: list, sample_rate: int) -> dict:
        """Transcribe audio from raw samples."""
        try:
            if self.model is None:
                return {"type": "response", "text": "", "error": "Model not loaded"}

            import torchaudio

            # Convert to numpy array
            waveform = np.array(samples, dtype=np.float32)

            # Resample to 16kHz if needed
            if sample_rate != 16000:
                waveform_tensor = torch.from_numpy(waveform).unsqueeze(0)
                resampler = torchaudio.transforms.Resample(sample_rate, 16000)
                waveform = resampler(waveform_tensor).squeeze().numpy()
                sample_rate = 16000

            text = self._transcribe_waveform(waveform, sample_rate)

            return {"type": "response", "text": text, "error": None}

        except Exception as e:
            log_stderr(f"Transcription failed: {e}")
            return {"type": "response", "text": "", "error": str(e)}

    def _transcribe_waveform(self, waveform: np.ndarray, sample_rate: int) -> str:
        """Internal transcription using the LFM chat interface."""
        from liquid_audio import ChatState, LFMModality

        # Convert numpy to torch tensor
        wav_tensor = torch.from_numpy(waveform).unsqueeze(0)  # Add channel dim

        # Create chat state for transcription
        chat = ChatState(self.processor)

        # System turn: instruct to transcribe
        chat.new_turn("system")
        chat.add_text("Transcribe the following audio accurately. Output only the spoken words, nothing else.")
        chat.end_turn()

        # User turn: add audio
        chat.new_turn("user")
        chat.add_audio(wav_tensor, sample_rate)
        chat.end_turn()

        # Generate response (text only, no audio output)
        chat.new_turn("assistant")

        text_tokens = []
        for t in self.model.generate_interleaved(**chat.tokenize(), max_new_tokens=448):
            if t.numel() == 1:  # Single token = text
                text_tokens.append(t)

        # Decode text
        if text_tokens:
            text_tensor = torch.cat(text_tokens)
            text = self.processor.text.decode(text_tensor.tolist())
        else:
            text = ""

        return text.strip()

    def unload(self) -> dict:
        """Unload the model to free memory."""
        try:
            if self.model is not None:
                del self.model
                self.model = None
            if self.processor is not None:
                del self.processor
                self.processor = None

            # Clear CUDA cache if available
            if torch.cuda.is_available():
                torch.cuda.empty_cache()

            log_stderr("Model unloaded")
            return {"type": "response", "text": "Model unloaded", "error": None}
        except Exception as e:
            return {"type": "response", "text": "", "error": str(e)}


def log_stderr(msg: str):
    """Log message to stderr (visible in Tauri console)."""
    print(f"[LFM] {msg}", file=sys.stderr, flush=True)


def main():
    """Main sidecar loop - reads JSON from stdin, writes JSON to stdout."""
    engine = LFMAudioEngine()
    log_stderr("LFM Audio sidecar started")

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
            request_type = request.get("type", "")

            if request_type == "load_model":
                model_id = request.get("model_id", "LiquidAI/LFM2.5-Audio-1.5B")
                response = engine.load_model(model_id)

            elif request_type == "transcribe":
                audio_path = request.get("audio_path", "")
                response = engine.transcribe_file(audio_path)

            elif request_type == "transcribe_samples":
                samples = request.get("samples", [])
                sample_rate = request.get("sample_rate", 16000)
                response = engine.transcribe_samples(samples, sample_rate)

            elif request_type == "unload":
                response = engine.unload()

            elif request_type == "ping":
                response = {"type": "pong"}

            elif request_type == "shutdown":
                engine.unload()
                response = {"type": "goodbye"}
                print(json.dumps(response), flush=True)
                break

            else:
                response = {"type": "error", "message": f"Unknown request type: {request_type}"}

            print(json.dumps(response), flush=True)

        except json.JSONDecodeError as e:
            response = {"type": "error", "message": f"Invalid JSON: {e}"}
            print(json.dumps(response), flush=True)
        except Exception as e:
            response = {"type": "error", "message": str(e)}
            print(json.dumps(response), flush=True)

    log_stderr("LFM Audio sidecar exiting")


if __name__ == "__main__":
    main()
