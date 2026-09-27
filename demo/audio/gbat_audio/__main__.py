"""Render a shared timeline JSON file to a mono PCM16 WAV file."""

import argparse
import importlib
import json
import re
import wave
from pathlib import Path

import numpy as np

from . import SAMPLE_RATE
from .score import synthesize as synthesize_score


def render(timeline: dict, layer: str) -> np.ndarray:
    size = round(timeline["duration"] * SAMPLE_RATE)
    audio = synthesize_score(timeline, SAMPLE_RATE) if layer != "cues" else np.zeros(size)
    if len(audio) != size:
        raise ValueError("The score must match the timeline duration")
    if layer != "score":
        for beat in timeline["beats"]:
            for cue in beat["cues"]:
                kind = cue["kind"]
                if not re.fullmatch(r"[a-z][a-z0-9_]*", kind):
                    raise ValueError(f"Invalid cue kind: {kind}")
                if not beat["start"] <= cue["time"] < beat["end"]:
                    raise ValueError(f"Cue outside its Beat: {cue['id']}")
                module = importlib.import_module(f"gbat_audio.cues.{kind}")
                sound = module.synthesize(SAMPLE_RATE) * cue.get("gain", 1.0)
                start = round(cue["time"] * SAMPLE_RATE)
                end = min(size, start + len(sound))
                audio[start:end] += sound[:end - start]
    if not np.isfinite(audio).all():
        raise ValueError("Audio contains non-finite samples")
    peak = np.max(np.abs(audio), initial=0)
    return audio * min(1.0, 0.92 / max(peak, 0.001))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeline", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--layer", choices=["mix", "score", "cues"], default="mix")
    arguments = parser.parse_args()
    timeline = json.loads(arguments.timeline.read_text())
    audio = render(timeline, arguments.layer)
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(arguments.output), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(SAMPLE_RATE)
        output.writeframes(np.round(audio * 32767).astype("<i2").tobytes())
    print(f"Wrote {arguments.output}: {len(audio) / SAMPLE_RATE:.2f}s at {SAMPLE_RATE} Hz")


if __name__ == "__main__":
    main()
