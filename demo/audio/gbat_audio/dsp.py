"""Small deterministic synthesis helpers."""

import numpy as np


def envelope(length: int, sample_rate: int, attack: float = 0.003, release: float = 0.03) -> np.ndarray:
    result = np.ones(length)
    attack_samples = min(length, round(attack * sample_rate))
    release_samples = min(length - attack_samples, round(release * sample_rate))
    if attack_samples:
        result[:attack_samples] = np.linspace(0, 1, attack_samples, endpoint=False)
    if release_samples:
        result[-release_samples:] = np.linspace(1, 0, release_samples)
    return result


def tone(frequency: float, duration: float, sample_rate: int, wave: str = "triangle") -> np.ndarray:
    phase = np.arange(round(duration * sample_rate)) * frequency / sample_rate
    if wave == "square":
        signal = np.where(phase % 1 < 0.5, 1.0, -1.0)
    elif wave == "triangle":
        signal = 4 * np.abs(phase % 1 - 0.5) - 1
    elif wave == "sine":
        signal = np.sin(2 * np.pi * phase)
    else:
        raise ValueError(f"Unknown waveform: {wave}")
    return signal * envelope(len(signal), sample_rate)
