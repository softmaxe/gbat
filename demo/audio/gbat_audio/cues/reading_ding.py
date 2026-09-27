"""A bright decaying chime when the terminal prints a reading."""

import numpy as np

from ..dsp import envelope


def synthesize(sample_rate: int) -> np.ndarray:
    time = np.arange(round(0.38 * sample_rate)) / sample_rate
    signal = 0.3 * np.sin(2 * np.pi * 1568 * time) + 0.12 * np.sin(2 * np.pi * 2352 * time)
    return signal * np.exp(-time * 10) * envelope(len(time), sample_rate, attack=0.002, release=0.06)
