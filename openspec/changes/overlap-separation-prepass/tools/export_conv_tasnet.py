"""Export the Conv-TasNet 2-source separation checkpoint to ONNX.

Redistribution terms: the checkpoint is Asteroid's (MIT) trained on
Libri2Mix / LibriSpeech (CC-BY-4.0) — redistribution with attribution; the
exported artifact ships in the repo under frontend/models/.

overlap-separation-prepass task 2.1. Downloads the Asteroid pretrained
checkpoint (Hugging Face hub), exports it with a DYNAMIC time axis (the
architecture is pure 1-D convolution, so any span length is valid), prints
the graph I/O names (ORT consumes those verbatim) and the artifact sha256
(the model_download registration pins this hash).

Usage:
    python export_conv_tasnet.py [--out <path.onnx>]

Default output: <repo>/frontend/models/../~/.meetily-models is NOT touched
here — the artifact lands next to this script and the caller moves/registers
it; hash-pinned download into the app models dir happens at runtime via
model_download.
"""

import argparse
import hashlib
from pathlib import Path

import torch
from asteroid.models import ConvTasNet

CHECKPOINT = "JorisCos/ConvTasNet_Libri2Mix_sepnoisy_16k"
OPSET = 17


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--out",
        default="conv_tasnet_libri2mix_sepnoisy_16k.onnx",
        help="output .onnx path",
    )
    args = ap.parse_args()

    print(f"loading {CHECKPOINT} ...")
    model = ConvTasNet.from_pretrained(CHECKPOINT)
    model.eval()
    n_src = model.masker.n_src
    assert n_src == 2, f"expected a 2-source checkpoint, got {n_src}"

    # 4 s @ 16 kHz probe input; the exported graph accepts ANY time length
    # (dynamic axis) because the network is fully convolutional.
    dummy = torch.randn(1, 16000 * 4)

    # The legacy TorchScript exporter is used on purpose: it has the longest
    # track record with pure-conv audio nets; fall back to the dynamo one
    # (default in newer torch) only if the legacy path is removed.
    try:
        torch.onnx.export(
            model,
            (dummy,),
            args.out,
            input_names=["mix"],
            output_names=["est_source"],
            dynamic_axes={"mix": {1: "time"}, "est_source": {2: "time"}},
            opset_version=OPSET,
            dynamo=False,
        )
    except TypeError:
        torch.onnx.export(
            model,
            (dummy,),
            args.out,
            input_names=["mix"],
            output_names=["est_source"],
            dynamic_axes={"mix": {1: "time"}, "est_source": {2: "time"}},
            opset_version=OPSET,
        )

    out = Path(args.out)
    print(f"wrote {out} ({out.stat().st_size / 1e6:.1f} MB)")
    print(f"sha256 {sha256(out)}")

    import onnx

    g = onnx.load(str(out)).graph
    print(f"graph inputs : {[i.name for i in g.input]}")
    print(f"graph outputs: {[o.name for o in g.output]}")


if __name__ == "__main__":
    main()
