"""A short mouse switch click generated from filtered noise and a pulse."""

import numpy as np

from ..dsp import envelope


def synthesize(sample_rate: int) -> np.ndarray:
    time = np.arange(round(0.07 * sample_rate)) / sample_rate
    noise = np.random.default_rng(17).normal(0, 0.12, len(time))
    sound = (noise + 0.35 * np.sin(2 * np.pi * 1900 * time)) * np.exp(-time * 90)
    return sound * envelope(len(sound), sample_rate, attack=0.001, release=0.015)
