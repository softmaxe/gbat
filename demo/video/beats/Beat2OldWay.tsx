import { AbsoluteFill, interpolate } from "remotion";
import type { Language } from "../../timeline";
import { oldWay } from "../../timeline/beats/beat2-old-way";
import { ramp, useBeatTime } from "../anim";
import { HandDrawnPanel } from "../components/HandDrawnPanel";
import { RedPenStrike } from "../components/RedPen";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

const m = oldWay.moments;
const outline = { stroke: PALETTE.graphite, strokeWidth: 3, roughness: 1.1, bowing: 0.8 };
const figures = [
  { x: 1210, y: 700, tilt: -4 },
  { x: 1440, y: 700, tilt: 3 },
  { x: 1670, y: 700, tilt: -3 },
  { x: 1325, y: 549, tilt: 4 },
  { x: 1555, y: 549, tilt: -4 },
  { x: 1440, y: 398, tilt: -2 },
];

export const Beat2OldWay: React.FC<{ language: Language }> = ({ language }) => {
  const { t } = useBeatTime(oldWay);
  const windowProgress = ramp(t, m.windowEnter, m.windowReady);
  const spinnerAngle = Math.max(0, Math.min(t, m.spinnerStop) - m.spinnerStart) * 220;

  return <AbsoluteFill>
    <svg width={1920} height={1080} viewBox="0 0 1920 1080">
      <text x={155} y={162} fontFamily={HAND_FONT} fontSize={57} fill={PALETTE.ink}
        opacity={windowProgress}>{oldWay.title[language]}</text>
      <g opacity={windowProgress} transform={`translate(0 ${interpolate(windowProgress, [0, 1], [-45, 0])})`}>
        <HeavyWindow angle={spinnerAngle} />
        <text x={600} y={781} textAnchor="middle" fontFamily={HAND_FONT} fontSize={36} fill={PALETTE.pencil}>
          {language === "en" ? "Heavyweight app" : "笨重的应用"}
        </text>
      </g>
      {figures.map((figure, index) => {
        const progress = ramp(t, m.processArrival[index], m.processLanded[index]);
        if (progress === 0) return null;
        const drop = interpolate(progress, [0, 0.75, 1], [-65, 6, 0]);
        return <g key={index} opacity={Math.min(1, progress * 3)}
          transform={`translate(${figure.x} ${figure.y + drop}) rotate(${figure.tilt})`}>
          <ProcessFigure seed={220 + index} />
        </g>;
      })}
      <text x={1440} y={781} textAnchor="middle" fontFamily={HAND_FONT} fontSize={36} fill={PALETTE.pencil}
        opacity={ramp(t, m.processArrival[0], m.processLanded[0])}>
        {language === "en" ? "Background processes" : "后台进程"}
      </text>
      <RedPenStrike x1={164} y1={274} x2={1030} y2={719} seed={231}
        progress={ramp(t, m.windowStrike, m.windowStruck)} />
      <RedPenStrike x1={1018} y1={282} x2={172} y2={726} seed={232}
        progress={ramp(t, m.windowCross, m.windowCrossed)} />
      <RedPenStrike x1={1134} y1={236} x2={1784} y2={721} seed={233}
        progress={ramp(t, m.processStrike, m.processStruck)} />
      <RedPenStrike x1={1769} y1={244} x2={1118} y2={715} seed={234}
        progress={ramp(t, m.processCross, m.processCrossed)} />
    </svg>
  </AbsoluteFill>;
};

const HeavyWindow: React.FC<{ angle: number }> = ({ angle }) => <g>
  <HandDrawnPanel x={217} y={280} width={790} height={405} seed={201} fill={PALETTE.paperShade} />
  <HandDrawnPanel x={200} y={297} width={790} height={405} seed={202} fill={PALETTE.cream} />
  <HandDrawnPanel x={183} y={314} width={790} height={405} seed={203}>
    <RoughDrawing seed={204} options={{ ...outline, fill: PALETTE.paperShade, fillStyle: "hachure", hachureGap: 8, fillWeight: 1.2 }}
      build={(g, o) => [g.rectangle(0, 0, 790, 54, o), g.rectangle(32, 87, 145, 283, { ...o, strokeWidth: 2 })]} />
    <RoughDrawing seed={205} options={{ ...outline, stroke: PALETTE.pencil, strokeWidth: 2.4 }}
      build={(g, o) => [g.line(31, 27, 166, 27, o), g.line(52, 119, 151, 119, o),
        g.line(52, 162, 125, 162, o), g.line(52, 205, 143, 205, o), g.line(52, 248, 119, 248, o),
        g.line(52, 291, 144, 291, o), g.line(52, 334, 132, 334, o)]} />
    <g transform={`translate(472 222) rotate(${angle})`}>
      {Array.from({ length: 10 }, (_, index) => <g key={index} transform={`rotate(${index * 36})`} opacity={0.2 + index * 0.08}>
        <RoughDrawing seed={207 + index} options={{ ...outline, strokeWidth: 7, stroke: PALETTE.pencil }}
          build={(g, o) => [g.line(0, -48, 0, -78, o)]} />
      </g>)}
    </g>
    <RoughDrawing seed={217} options={{ ...outline, stroke: PALETTE.pencil, strokeWidth: 2 }}
      build={(g, o) => [g.rectangle(294, 336, 356, 19, o),
        g.rectangle(294, 336, 53, 19, { ...o, fill: PALETTE.paperShade, fillStyle: "hachure", hachureGap: 5 })]} />
  </HandDrawnPanel>
</g>;

const ProcessFigure: React.FC<{ seed: number }> = ({ seed }) => <g>
  <RoughDrawing seed={seed} options={outline}
    build={(g, o) => [g.linearPath([[-45, -30], [-53, 0], [-77, 0]], o),
      g.linearPath([[45, -30], [53, 0], [77, 0]], o),
      g.linearPath([[-78, -88], [-105, -67], [-109, -41]], o),
      g.linearPath([[78, -88], [105, -67], [109, -41]], o)]} />
  <path d="M -80 -145 L 55 -145 L 80 -120 L 80 -30 L -80 -30 Z" fill={PALETTE.cream} />
  <RoughDrawing seed={seed + 40} options={{ ...outline, fill: PALETTE.paperShade, fillStyle: "hachure", hachureGap: 9, fillWeight: 0.7 }}
    build={(g, o) => [g.polygon([[-80, -145], [55, -145], [80, -120], [80, -30], [-80, -30]], o),
      g.linearPath([[55, -145], [55, -120], [80, -120]], { ...o, fill: undefined })]} />
  <ellipse cx={-28} cy={-106} rx={4.5} ry={7} fill={PALETTE.ink} />
  <ellipse cx={20} cy={-106} rx={4.5} ry={7} fill={PALETTE.ink} />
  <RoughDrawing seed={seed + 60} options={{ ...outline, strokeWidth: 2.4 }}
    build={(g, o) => [g.path("M -14 -86 Q -4 -92 6 -86", o), g.line(-44, -56, 39, -56, o)]} />
</g>;
