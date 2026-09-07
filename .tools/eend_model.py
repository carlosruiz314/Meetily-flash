# Standalone EEND-EDA reconstruction for the .meetily-models checkpoints.
#
# The checkpoint ships the whole pipeline: the mel filterbank matrix
# (frontend.logmel.melmat), global-CMVN stats (normalize.mean/std), a
# post-norm transformer encoder with a 15-frame context-stacking linear embed
# (±7 frames of 23-dim 8 kHz fbank -> Linear(345, 256)), and the EDA LSTM
# attractor. Module tree mirrors the checkpoint keys exactly so strict
# loading is the correctness check for the reconstruction.
import math

import torch
import torch.nn as nn
import torch.nn.functional as F


class MultiHeadedAttention(nn.Module):
    def __init__(self, heads: int, d_model: int, dropout: float = 0.0):
        super().__init__()
        assert d_model % heads == 0
        self.d_k = d_model // heads
        self.h = heads
        self.linear_q = nn.Linear(d_model, d_model)
        self.linear_k = nn.Linear(d_model, d_model)
        self.linear_v = nn.Linear(d_model, d_model)
        self.linear_out = nn.Linear(d_model, d_model)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        b, t, _ = x.shape
        q = self.linear_q(x).view(b, t, self.h, self.d_k).transpose(1, 2)
        k = self.linear_k(x).view(b, t, self.h, self.d_k).transpose(1, 2)
        v = self.linear_v(x).view(b, t, self.h, self.d_k).transpose(1, 2)
        scores = torch.matmul(q, k.transpose(-2, -1)) / math.sqrt(self.d_k)
        out = torch.matmul(torch.softmax(scores, dim=-1), v)
        return self.linear_out(out.transpose(1, 2).reshape(b, t, self.h * self.d_k))


class PositionwiseFeedForward(nn.Module):
    def __init__(self, d_model: int, d_ff: int):
        super().__init__()
        self.w_1 = nn.Linear(d_model, d_ff)
        self.w_2 = nn.Linear(d_ff, d_model)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.w_2(F.relu(self.w_1(x)))


class EncoderLayer(nn.Module):
    """Espnet-style post-norm encoder layer."""

    def __init__(self, d_model: int, heads: int, d_ff: int):
        super().__init__()
        self.self_attn = MultiHeadedAttention(heads, d_model)
        self.feed_forward = PositionwiseFeedForward(d_model, d_ff)
        self.norm1 = nn.LayerNorm(d_model)
        self.norm2 = nn.LayerNorm(d_model)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        residual = x
        x = self.norm1(x)
        x = residual + self.self_attn(x)
        residual = x
        x = self.norm2(x)
        return residual + self.feed_forward(x)


class Encoder(nn.Module):
    def __init__(self, n_mels: int, context: int, d_model: int = 256, blocks: int = 4, d_ff: int = 512):
        super().__init__()
        self.context = context
        self.embed = nn.Sequential(
            nn.Linear(n_mels * (2 * context + 1), d_model),
            nn.LayerNorm(d_model),
        )
        self.encoders = nn.ModuleList([EncoderLayer(d_model, 4, d_ff) for _ in range(blocks)])
        self.after_norm = nn.LayerNorm(d_model)

    def forward(self, feats: torch.Tensor) -> torch.Tensor:
        c = self.context
        b, t, n = feats.shape
        padded = F.pad(feats.transpose(1, 2), (c, c))
        stacked = padded.unfold(2, 2 * c + 1, 1)  # (B, n, T, 2c+1)
        b, n, t, w = stacked.shape
        x = stacked.permute(0, 2, 3, 1).reshape(b, t, n * w)
        x = self.embed(x)
        for layer in self.encoders:
            x = layer(x)
        return self.after_norm(x)


class Attractor(nn.Module):
    def __init__(self, d_model: int = 256):
        super().__init__()
        self.attractor_encoder = nn.LSTM(d_model, d_model, 1, batch_first=True, bidirectional=False)
        self.attractor_decoder = nn.LSTM(d_model, d_model, 1, batch_first=True)
        self.linear_projection = nn.Linear(d_model, 1)


class EendEda(nn.Module):
    """Input: 8 kHz waveform (B, samples). Output: (B, T, num_spk) activity logits
    at the fbank frame rate (hop 128 @ 8 kHz = 16 ms per frame)."""

    def __init__(self, num_spk=3, n_mels=23, d_model=256, blocks=4):
        super().__init__()
        self.num_spk = num_spk
        self.n_mels = n_mels
        self.frontend = nn.Module()
        self.frontend.logmel = nn.Module()
        self.frontend.logmel.register_buffer("melmat", torch.zeros(101, n_mels))
        self.normalize = nn.Module()
        self.normalize.register_buffer("mean", torch.zeros(n_mels))
        self.normalize.register_buffer("std", torch.ones(n_mels))
        self.encoder = Encoder(n_mels, 7, d_model, blocks)
        self.attractor = Attractor(d_model)

    def logmel(self, wav: torch.Tensor) -> torch.Tensor:
        # Kaldi-style fbank front end: n_fft 200, hop 128 @ 8 kHz, power
        # spectrum projected through the checkpoint's own melmat, then log.
        n_fft = 200
        hop = 128
        frames = wav.unfold(1, n_fft, hop)  # (B, T, n_fft)
        window = torch.hann_window(n_fft, device=wav.device)
        spec = torch.abs(torch.fft.rfft(frames * window, dim=-1)) ** 2
        mel = spec @ self.frontend.logmel.melmat
        return torch.log(torch.clamp(mel, min=1e-10))

    def forward(self, wav: torch.Tensor) -> torch.Tensor:
        feats = self.logmel(wav)
        feats = (feats - self.normalize.mean) / self.normalize.std
        x = self.encoder(feats)
        b, t, d = x.shape
        # Official EEND-EDA inference: seed the attractor chain with the mean
        # of encoder outputs; the BiLSTM half is training-only.
        summary = torch.mean(x, dim=1, keepdim=True)
        att_out = []
        hx = None
        inp = summary
        for _ in range(self.num_spk + 1):
            out, hx = self.attractor.attractor_decoder(inp, hx)
            att_out.append(out[:, -1, :])
            inp = out
        attractor = torch.stack(att_out, dim=1)
        return torch.bmm(x, attractor[:, : self.num_spk, :].permute(0, 2, 1))
