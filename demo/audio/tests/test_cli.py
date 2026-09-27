"""Exercise the public timeline-to-WAV entry point, including isolated cue onsets."""

import json
import subprocess
import sys
import wave
from pathlib import Path

import numpy as np
import pytest

ROOT = Path(__file__).resolve().parents[2]
TIMELINE = json.loads((ROOT / "build/timeline.json").read_text())
CUES = [cue for beat in TIMELINE["beats"] for cue in beat["cues"]]


def run_cli(tmp_path, timeline, layer="mix"):
    source = tmp_path / "timeline.json"
    output = tmp_path / "audio.wav"
    source.write_text(json.dumps(timeline))
    subprocess.run([sys.executable, "-m", "gbat_audio", "--timeline", str(source), "--output", str(output), "--layer", layer], check=True)
    with wave.open(str(output)) as wav:
        assert wav.getnchannels() == 1
        assert wav.getsampwidth() == 2
        assert wav.getframerate() == 48_000
        return np.frombuffer(wav.readframes(wav.getnframes()), dtype="<i2") / 32768


def test_public_cli_duration_and_headroom(tmp_path):
    audio = run_cli(tmp_path, TIMELINE)
    assert len(audio) == 48_000 * TIMELINE["duration"]
    assert 0.02 < np.max(np.abs(audio)) < 0.99


@pytest.mark.parametrize("cue", CUES, ids=lambda cue: cue["id"])
def test_each_cue_starts_at_its_exported_time(tmp_path, cue):
    timeline = json.loads(json.dumps(TIMELINE))
    for beat in timeline["beats"]:
        beat["cues"] = [item for item in beat["cues"] if item["id"] == cue["id"]]
    audio = run_cli(tmp_path, timeline, "cues")
    start = round(cue["time"] * 48_000)
    assert np.max(np.abs(audio[:start]), initial=0) == 0
    assert np.max(np.abs(audio[start:start + 480])) > 0.001
    assert np.sqrt(np.mean(audio[start:start + 4800] ** 2)) > 0.001
