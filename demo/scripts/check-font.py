"""Check actual bundled font coverage and caption advances for an exported timeline."""

import json
import sys
from pathlib import Path

from fontTools.ttLib import TTFont

root = Path(__file__).resolve().parent.parent
font = TTFont(root / "video/public/fonts/LXGWWenKai-Regular.subset.woff2")
timeline = json.loads(Path(sys.argv[1]).read_text())
mapping = font.getBestCmap()
units = font["head"].unitsPerEm
metrics = font["hmtx"].metrics
errors = []
for beat in timeline["beats"]:
    for item in [beat["title"], *[caption["text"] for caption in beat["captions"]]]:
        for language, text in item.items():
            missing = sorted({character for character in text if ord(character) not in mapping})
            if missing:
                errors.append(f"{beat['id']} {language}: missing glyphs {missing!r}")
            elif item != beat["title"]:
                width = sum(metrics[mapping[ord(character)]][0] for character in text) / units * 46
                if width > 1660:
                    errors.append(f"{beat['id']} {language}: caption width {width:.1f}px exceeds 1660px")
if errors:
    raise SystemExit("\n".join(errors))
print("All caption glyphs and widths fit the bundled font.")
