import { AbsoluteFill, useCurrentFrame } from "remotion";
import type { Beat, Language } from "../../timeline";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

/** A replaceable picture for the first complete pipeline build. */
export const Placeholder: React.FC<{ beat: Beat; language: Language; number: number }> = ({ beat, language, number }) => {
  const frame = useCurrentFrame();
  return (
    <AbsoluteFill style={{ alignItems: "center", justifyContent: "center", paddingBottom: 180 }}>
      <svg width={1000} height={500} viewBox="0 0 1000 500">
        <RoughDrawing seed={number + Math.floor(frame / 15) % 2} options={{ stroke: PALETTE.graphite, strokeWidth: 3, fill: PALETTE.sky, fillStyle: "hachure", hachureGap: 15 }}
          build={(g, o) => [g.rectangle(100, 100, 800, 300, o)]} />
        <text x={500} y={240} textAnchor="middle" fontFamily={HAND_FONT} fontSize={68} fill={PALETTE.graphite}>gbat</text>
        <text x={500} y={325} textAnchor="middle" fontFamily={HAND_FONT} fontSize={44} fill={PALETTE.graphite}>{beat.title[language]}</text>
      </svg>
    </AbsoluteFill>
  );
};
