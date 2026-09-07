# EEND-EDA export spike: ESPnet2 diar checkpoint -> ONNX.
# Mirrors espnet2/bin/diar_inference.py's EEND-EDA path with fixed num_spk:
#   encode(raw waveform) -> EDA attractor (num_spk+1 zeros) -> bmm -> per-frame
#   speaker activity logits (1, T, num_spk).
import sys, os
import torch

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, ".tools", "espnet"))
from espnet2.tasks.diar import DiarizationTask  # noqa: E402

BASE = os.path.join(os.path.expanduser("~"), ".meetily-models")

JOBS = [
    (
        "eend-eda-spk3",
        os.path.join(BASE, "eend-eda-spk3", "exp_eda", "spk3", "diar_train_diar_eda_adapt_raw_spk3"),
        3,
    ),
    (
        "eend-eda-spk4",
        os.path.join(BASE, "eend-eda-spk4", "exp_eda", "spk4", "diar_train_diar_eda_adapt_raw_spk4"),
        4,
    ),
]


class DiarInference(torch.nn.Module):
    def __init__(self, m, n):
        super().__init__()
        self.m = m
        self.n = n

    def forward(self, speech, speech_lengths):
        enc, enc_lens = self.m.encode(speech, speech_lengths, None, None)
        zeros = torch.zeros(speech.size(0), self.n + 1, enc.size(2))
        attractor, _ = self.m.attractor(enc, enc_lens, zeros)
        return torch.bmm(enc, attractor[:, : self.n, :].permute(0, 2, 1))


def main():
    for name, ckpt, n in JOBS:
        out_path = os.path.join(BASE, f"{name}.onnx")
        if os.path.exists(out_path):
            print(f"[{name}] already exported: {out_path}")
            continue
        if not os.path.isdir(ckpt):
            print(f"[{name}] checkpoint dir missing: {ckpt}")
            continue
        print(f"[{name}] loading {ckpt}")
        old_cwd = os.getcwd()
        os.chdir(os.path.join(BASE, name))  # stats_file in config is relative to the exp root
        try:
            model, _ = DiarizationTask.build_model_from_file(
                os.path.join(ckpt, "config.yaml"), os.path.join(ckpt, "20epoch.pth")
            )
        finally:
            os.chdir(old_cwd)
        model.eval()

        wrapped = DiarInference(model, n).eval()
        speech = torch.randn(1, 16000 * 10)
        lengths = torch.tensor([16000 * 10])
        with torch.no_grad():
            ref = wrapped(speech, lengths)
            torch.onnx.export(
                wrapped,
                (speech, lengths),
                out_path,
                input_names=["speech", "speech_lengths"],
                output_names=["activity"],
                dynamic_axes={
                    "speech": {0: "batch", 1: "samples"},
                    "speech_lengths": {0: "batch"},
                    "activity": {0: "batch", 1: "frames"},
                },
                opset_version=17,
                dynamo=False,
            )
        print(f"[{name}] exported: {out_path} (ref out {tuple(ref.shape)})")


if __name__ == "__main__":
    main()
