"""A short two-step square pulse for the outgoing HID++ request."""

import numpy as np

from ..dsp import tone


def synthesize(sample_rate: int) -> np.ndarray:
    return np.concatenate([
        0.17 * tone(880, 0.055, sample_rate, "square"),
        0.15 * tone(1320, 0.07, sample_rate, "square"),
    ])
