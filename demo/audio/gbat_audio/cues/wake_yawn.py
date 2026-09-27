"""A rising, then falling voiced yawn with a quiet synthetic breath."""

import numpy as np

from ..dsp import envelope


def synthesize(sample_rate: int) -> np.ndarray:
    time = np.arange(round(1.35 * sample_rate)) / sample_rate
    pitch = np.interp(time, [0, 0.22, 0.6, 0.95, 1.35], [165, 185, 270, 210, 140])
    phase = 2 * np.pi * np.cumsum(pitch) / sample_rate
    voice = 0.22 * np.sin(phase) + 0.065 * np.sin(phase * 2) + 0.035 * np.sin(phase * 3)
    breath = np.random.default_rng(23).normal(0, 1, len(time))
    breath = np.convolve(breath, np.ones(15) / 15, mode="same") * 0.06
    shape = np.interp(time, [0, 0.06, 0.4, 0.8, 1.15, 1.35], [0.3, 0.55, 1, 0.9, 0.4, 0])
    return (voice + breath) * shape * envelope(len(time), sample_rate, attack=0.005, release=0.12)
