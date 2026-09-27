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
    assert np.max(np.abs(audio[-48_000:])) < 0.01


@pytest.fixture(scope="module")
def score(tmp_path_factory):
    return run_cli(tmp_path_factory.mktemp("score"), TIMELINE, "score")


def rms(audio):
    return np.sqrt(np.mean(audio ** 2))


def dominant_pitch(audio, start):
    """Measure a short lead-note window after the drum transient."""
    samples = audio[round(start * 48_000):round((start + 0.1) * 48_000)]
    frequencies = np.fft.rfftfreq(len(samples), 1 / 48_000)
    spectrum = np.abs(np.fft.rfft(samples * np.hanning(len(samples))))
    spectrum[(frequencies < 180) | (frequencies > 2500)] = 0
    return frequencies[np.argmax(spectrum)]


def test_score_duration_music_in_each_beat_and_final_fade(score):
    assert len(score) == 48_000 * TIMELINE["duration"]
    for beat in TIMELINE["beats"]:
        assert rms(score[round(beat["start"] * 48_000):round(beat["end"] * 48_000)]) > 0.01
    assert rms(score[-2 * 48_000:-48_000]) < 0.5 * rms(score[-3 * 48_000:-2 * 48_000])
    # The entire last second must be quiet, including any isolated transient.
    assert np.max(np.abs(score[-48_000:])) < 0.01


def test_score_noise_is_reproducible(tmp_path, score):
    assert np.array_equal(run_cli(tmp_path, TIMELINE, "score"), score)


def test_battery_drain_lowers_the_pitch_and_ending_returns_to_tonic(score):
    opening = next(beat for beat in TIMELINE["beats"] if beat["id"] == "opening")
    eighth = 60 / 110 / 2
    times = np.arange(opening["start"] + 0.07, opening["end"] - 0.2, eighth)
    pitches = np.array([dominant_pitch(score, time) for time in times])
    third = len(pitches) // 3
    assert np.median(pitches[-third:]) < 0.8 * np.median(pitches[:third])
    # The final sustained root is one octave below the opening's first note.
    tonic = dominant_pitch(score, TIMELINE["duration"] - 3)
    assert abs(2 * tonic - pitches[0]) < 20


def test_pitch_sag_stops_at_the_exported_collapse_before_the_caption_ends(tmp_path):
    timeline = json.loads(json.dumps(TIMELINE))
    opening = next(beat for beat in timeline["beats"] if beat["id"] == "opening")
    caption = next(caption for caption in opening["captions"] if caption["id"] == "opening-charge")
    caption.update(start=2.4, end=7.8)
    opening["moments"] = {"drain": 2.4, "red": 4.7, "collapse": 5.3}
    audio = run_cli(tmp_path, timeline, "score")
    # Compare repetitions of the same root note before drain and after collapse.
    repeat = 2 * 60 / 110
    times = np.arange(opening["start"] + 0.07, opening["end"] - 0.1, repeat)
    before = dominant_pitch(audio, times[times < opening["moments"]["drain"]][-1])
    after = [dominant_pitch(audio, time) for time in times if time > opening["moments"]["collapse"]]
    assert len(after) >= 2
    assert max(after) - min(after) < 20
    assert abs(2 * after[0] - before) < 20


@pytest.mark.parametrize("shift", [0, -0.8])
def test_charging_arpeggio_follows_the_caption_window(tmp_path, shift):
    timeline = json.loads(json.dumps(TIMELINE))
    command = next(beat for beat in timeline["beats"] if beat["id"] == "one-command")
    charging = next(caption for caption in command["captions"] if caption["id"] == "command-charge")
    previous = next(caption for caption in command["captions"] if caption["end"] == charging["start"])
    charging["start"] += shift
    charging["end"] += shift
    previous["end"] = charging["start"]
    audio = run_cli(tmp_path, timeline, "score")
    times = np.arange(charging["start"] + 0.07, charging["end"] - 0.12, 60 / 110)
    pitches = [dominant_pitch(audio, time) for time in times]
    assert all(after >= before - 20 for before, after in zip(pitches, pitches[1:]))
    assert pitches[-1] > 2 * pitches[0]


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
