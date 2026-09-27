"""A minimal music bed, replaced by the complete score in ticket #26."""

import numpy as np

from .dsp import tone


def synthesize(timeline: dict, sample_rate: int) -> np.ndarray:
    result = np.zeros(round(timeline["duration"] * sample_rate))
    beat_seconds = 60 / 110
    notes = [261.6256, 329.6276, 391.9954, 329.6276]
    for index, time in enumerate(np.arange(0, timeline["duration"], beat_seconds)):
        note = tone(notes[index % len(notes)], beat_seconds * 0.7, sample_rate) * 0.08
        start = round(time * sample_rate)
        stop = min(len(result), start + len(note))
        result[start:stop] += note[:stop - start]
    return result
