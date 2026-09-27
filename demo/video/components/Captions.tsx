import { AbsoluteFill, Sequence } from "remotion";
import { BEATS, toFrame } from "../../timeline";
import type { Language } from "../../timeline";
import { HAND_FONT } from "../fonts";
import { PALETTE } from "../theme";

export const CAPTION_SIZE = 46;
export const CAPTION_WIDTH = 1660;

export const Captions: React.FC<{ language: Language }> = ({ language }) => (
  <AbsoluteFill style={{ pointerEvents: "none" }}>
    {BEATS.flatMap((beat) => beat.captions).map((caption) => (
      <Sequence key={caption.id} from={toFrame(caption.start)} durationInFrames={toFrame(caption.end - caption.start)} layout="none">
        <div style={{ position: "absolute", left: 130, bottom: 100, width: CAPTION_WIDTH,
          fontFamily: HAND_FONT, fontSize: CAPTION_SIZE, lineHeight: 1.3, textAlign: "center",
          whiteSpace: "nowrap", color: PALETTE.ink }}>
          {caption.text[language]}
        </div>
      </Sequence>
    ))}
  </AbsoluteFill>
);
