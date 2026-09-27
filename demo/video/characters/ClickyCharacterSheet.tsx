import { AbsoluteFill, useCurrentFrame, useVideoConfig } from "remotion";
import { Paper } from "../components/Paper";
import { HAND_FONT, loadFonts } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";
import { BatteryCells } from "./BatteryCells";
import { Clicky, type ClickyPose } from "./Clicky";

loadFonts();

const poses: { pose: ClickyPose; label: string; x: number; y: number; level: number }[] = [
  { pose: "play", label: "Play", x: 300, y: 438, level: 78 },
  { pose: "collapse", label: "Collapse", x: 745, y: 438, level: 0 },
  { pose: "idle", label: "Idle / 待机", x: 1190, y: 438, level: 78 },
  { pose: "offline", label: "Offline / 离线", x: 1610, y: 438, level: 78 },
  { pose: "wake", label: "Yawn + stretch / 醒来", x: 355, y: 887, level: 78 },
  { pose: "hold", label: "Hold a sign", x: 960, y: 887, level: 78 },
  { pose: "wave", label: "Wave", x: 1545, y: 887, level: 42 },
];

/** A six-second looping pose review, separate from the final film. */
export const ClickyCharacterSheet: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const time = frame / fps;
  return <AbsoluteFill>
    <Paper />
    <svg width={1920} height={1080} style={{ position: "absolute", inset: 0 }}>
      <g fontFamily={HAND_FONT} fill={PALETTE.ink}>
        <text x={115} y={123} fontSize={64}>Clicky</text>
        <text x={330} y={119} fontSize={31} fill={PALETTE.pencil}>A wireless mouse with a visible battery</text>
        <BatteryCells x={1345} y={88} width={290} height={31} level={42} charging time={time} />
        <text x={1695} y={115} fontSize={30}>42%</text>
        <RoughDrawing seed={714} options={{ stroke: PALETTE.pencil, strokeWidth: 1.5, roughness: 1 }}
          build={(g, o) => [g.line(115, 157, 1805, 157, o)]} />
        {poses.map(({ pose, label, x, y, level }, index) => <g key={pose}>
          <Clicky x={x} y={y} pose={pose} time={pose === "wake" ? time % 2.6 : time} batteryLevel={level}
            charging={pose === "wave"} seed={19 + index * 100} signText="78%" />
          <text x={x} y={y + 59} fontSize={35} textAnchor="middle">{label}</text>
        </g>)}
        <text x={115} y={1020} fontSize={26} fill={PALETTE.pencil}>White shell, pencil hatching, short legs, scroll-wheel nose.</text>
      </g>
    </svg>
  </AbsoluteFill>;
};
