"""A 110 BPM square, triangle, and noise score driven by the film timeline."""

import numpy as np

from .dsp import envelope, tone

BPM = 110
QUARTER = 60 / BPM
EIGHTH = QUARTER / 2


def _add(result: np.ndarray, sound: np.ndarray, time: float, sample_rate: int) -> None:
    start = round(time * sample_rate)
    stop = min(len(result), start + len(sound))
    result[start:stop] += sound[:stop - start]


def _note(midi: int, time: float, duration: float, sample_rate: int, wave: str,
          sag: tuple[float, float] | None = None) -> np.ndarray:
    frequency = 440 * 2 ** ((midi - 69) / 12)
    if sag is None:
        return tone(frequency, duration, sample_rate, wave)
    seconds = time + np.arange(round(duration * sample_rate)) / sample_rate
    progress = np.clip((seconds - sag[0]) / (sag[1] - sag[0]), 0, 1)
    phase = np.cumsum(frequency * 2 ** -progress) / sample_rate
    signal = np.where(phase % 1 < 0.5, 1.0, -1.0) if wave == "square" else 4 * np.abs(phase % 1 - 0.5) - 1
    return signal * envelope(len(signal), sample_rate)


def _drum(sample_rate: int, step: int, rng: np.random.Generator) -> np.ndarray:
    # Short high-passed noise hats alternate with a longer noise snare.
    snare = step % 4 == 2
    duration = 0.13 if snare else 0.045
    size = round(duration * sample_rate)
    noise = rng.uniform(-1, 1, size + 1)
    sound = np.diff(noise) * np.exp(-np.arange(size) / (sample_rate * duration / 5))
    sound *= envelope(size, sample_rate, attack=0.001, release=0.01)
    return sound * (0.065 if snare else 0.028)


def _section(result: np.ndarray, start: float, end: float, sample_rate: int,
             style: str, rng: np.random.Generator,
             sag: tuple[float, float] | None = None) -> None:
    level = {"opening": 0.85, "old-way": 0.65, "wake-it": 0.5, "ending": 0.85}.get(style, 1.0)
    for step, time in enumerate(np.arange(start, end, EIGHTH)):
        chord = (step // 8) % 4
        root = 48 if style == "opening" else (48, 45, 41, 43)[chord]
        third = 3 if chord == 1 and style != "opening" else 4
        melody = (24, 24 + third, 31, 24 + third, 26, 24 + third, 31, 36)
        if style == "opening":
            melody = (24, 28, 31, 28, 24, 28, 31, 28)
        # Quieter, sparse phrases leave space for the vendor-window gag and yawn.
        if style not in {"old-way", "wake-it"} or step % 2 == 0:
            duration = min(EIGHTH * 0.78, end - time)
            lead = _note(root + melody[step % 8], time, duration, sample_rate, "square", sag)
            _add(result, lead * 0.075 * level, time, sample_rate)
        if step % 2 == 0:
            duration = min(QUARTER * 0.78, end - time)
            bass = _note(root + (7 if step % 4 == 2 else 0), time, duration, sample_rate, "triangle", sag)
            _add(result, bass * 0.08 * level, time, sample_rate)
        drum = _drum(sample_rate, step, rng)[:round((end - time) * sample_rate)]
        _add(result, drum * level, time, sample_rate)


def _charging(result: np.ndarray, start: float, end: float, sample_rate: int,
              rng: np.random.Generator) -> None:
    # Each quarter note climbs through a C major arpeggio, then holds the top C.
    arpeggio = (60, 64, 67, 72, 76, 79, 84)
    for step, time in enumerate(np.arange(start, end, EIGHTH)):
        note = arpeggio[min(step // 2, len(arpeggio) - 1)]
        if step % 2 == 0:
            duration = min(QUARTER * 0.88, end - time)
            _add(result, tone(440 * 2 ** ((note - 69) / 12), duration, sample_rate, "square") * 0.082, time, sample_rate)
            _add(result, tone(440 * 2 ** ((note - 81) / 12), duration, sample_rate, "triangle") * 0.055, time, sample_rate)
        _add(result, _drum(sample_rate, step, rng)[:round((end - time) * sample_rate)], time, sample_rate)


def synthesize(timeline: dict, sample_rate: int) -> np.ndarray:
    result = np.zeros(round(timeline["duration"] * sample_rate))
    beats = {beat["id"]: beat for beat in timeline["beats"]}
    captions = {caption["id"]: caption for beat in beats.values() for caption in beat["captions"]}
    drain = captions["opening-charge"]
    charging = captions["command-charge"]
    ending = beats["ending"]
    farewell = captions["ending-install"]
    # Finish slightly before the last second so AAC overlap cannot leak the fade.
    silence = ending["end"] - 1.08
    resolve = max(ending["start"], min(farewell["end"], silence) - 4 * QUARTER)
    fade_start = max(resolve, min(farewell["end"] - QUARTER, silence - QUARTER))
    rng = np.random.default_rng(110)

    for beat in beats.values():
        start, end, style = beat["start"], beat["end"], beat["id"]
        if style == "one-command":
            _section(result, start, charging["start"], sample_rate, style, rng)
            _charging(result, charging["start"], charging["end"], sample_rate, rng)
            _section(result, charging["end"], end, sample_rate, style, rng)
        elif style == "ending":
            _section(result, start, max(start, resolve - QUARTER), sample_rate, style, rng)
            # A short dominant chord settles into a sustained C major tonic.
            for note in (55, 71, 74):
                _add(result, _note(note, resolve - QUARTER, QUARTER, sample_rate, "triangle") * 0.055, resolve - QUARTER, sample_rate)
            for note, wave, gain in ((48, "triangle", 0.10), (60, "square", 0.065), (64, "triangle", 0.04), (67, "triangle", 0.04)):
                _add(result, _note(note, resolve, silence - resolve, sample_rate, wave) * gain, resolve, sample_rate)
        else:
            sag = (drain["start"], drain["end"]) if style == "opening" else None
            _section(result, start, end, sample_rate, style, rng, sag)

    seconds = np.arange(len(result)) / sample_rate
    # A squared fade is smooth at silence and keeps the preceding tail gentle.
    result *= np.clip((silence - seconds) / (silence - fade_start), 0, 1) ** 2
    result *= np.clip(seconds / 0.025, 0, 1)
    return result
