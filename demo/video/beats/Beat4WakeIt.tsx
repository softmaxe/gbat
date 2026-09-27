import { AbsoluteFill, interpolate } from "remotion";
import type { Language } from "../../timeline";
import { wakeIt } from "../../timeline/beats/beat4-wake-it";
import { EXAMPLE_READINGS } from "../../timeline/readings";
import { clamp, ramp, useBeatTime } from "../anim";
import { Clicky, type ClickyPose } from "../characters/Clicky";
import { HandDrawnTerminal, type TerminalLine } from "../components/HandDrawnTerminal";
import { RedPenArrow, RedPenCircle, RedPenTick } from "../components/RedPen";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

const m = wakeIt.moments;
const reading = EXAMPLE_READINGS.wireless;
const idleYawn = wakeIt.cues.find((cue) => cue.id === "wake-idle-yawn")!;
const offlineYawn = wakeIt.cues.find((cue) => cue.id === "wake-offline-yawn")!;
const outline = { stroke: PALETTE.graphite, strokeWidth: 3, roughness: 1.1 };
const typed = (t: number, times: number[]) => "gbat".slice(0, times.filter((time) => t >= time).length);

export const Beat4WakeIt: React.FC<{ language: Language }> = ({ language }) => {
  const { t } = useBeatTime(wakeIt);
  const offline = t >= m.offline;
  const yawn = offline ? offlineYawn : idleYawn;
  const answerAt = offline ? m.retryAnswer : m.idleAnswer;
  const answered = t >= answerAt;
  const waking = t >= yawn.time;
  const pose: ClickyPose = answered ? "hold" : waking ? "wake" : offline ? "offline" : "idle";
  const poseStart = answered ? answerAt : waking ? yawn.time : offline ? m.offline : wakeIt.start;
  const lines: TerminalLine[] = [`$ ${typed(t, offline ? m.offlineTyping : m.idleTyping)}`];
  if (offline && t >= m.error) lines.push(
    { text: "The receiver reports no connected mouse.", color: PALETTE.accent },
    "Turn the mouse on or wake it, and retry.",
  );
  if (offline && t >= m.retryTyping[0]) lines.push(`$ ${typed(t, m.retryTyping)}`);
  if (answered) lines.push(reading.output);
  const requestAt = !offline ? m.idleRequest : t < m.retryRequest ? m.offlineRequest : m.retryRequest;
  const requestEnd = !offline ? idleYawn.time : t < m.retryRequest ? m.error : m.retryArrive;
  const requestVisible = t >= requestAt && t < requestEnd;
  const requestProgress = ramp(t, requestAt, requestEnd);
  const shift = 42 * ramp(t, m.nudge, m.nudgeEnd);
  const handOpacity = ramp(t, m.handEnter, m.handTouch) * (1 - ramp(t, m.handLeave, m.handGone));
  const handX = interpolate(t, [m.handEnter, m.handTouch, m.nudge, m.nudgeEnd, m.handLeave, m.handGone],
    [1310, 1476, 1476, 1518, 1518, 1300], clamp);
  return <AbsoluteFill>
    <svg width={1920} height={1080} viewBox="0 0 1920 1080">
      <g opacity={ramp(t, wakeIt.start, m.appear)}>
        <text x={155} y={162} fontFamily={HAND_FONT} fontSize={57} fill={PALETTE.ink}>{wakeIt.title[language]}</text>
        {!offline && <ColdReadClock language={language} progress={ramp(t, m.idleRequest, m.idleAnswer)} />}
        <text x={1550} y={192} textAnchor="middle" fontFamily={HAND_FONT} fontSize={48} fill={PALETTE.ink}>
          {answered ? language === "en" ? "Ready" : "已唤醒" : waking ? language === "en" ? "Waking" : "唤醒中" : offline ? language === "en" ? "Offline" : "离线" : language === "en" ? "Idle" : "待机"}
        </text>
        <HandDrawnTerminal x={155} y={305} width={1000} height={440} fontSize={31} lineHeight={55}
          lines={lines} cursor={!offline ? t < m.idleRequest : t < m.offlineRequest || t >= m.retryTyping[0] && t < m.retryRequest} seed={410} />
        <RoughDrawing seed={411} options={{ ...outline, stroke: PALETTE.pencil, strokeWidth: 1.7 }}
          build={(g, o) => [g.line(1355, 704, 1810, 704, o)]} />
        <Clicky x={1515 + shift} y={690} scale={1.3} pose={pose} time={t - poseStart}
          batteryLevel={reading.level} signText={`${reading.level}%`} />
        {requestVisible && <RequestPlane x={1168 + 260 * requestProgress} y={451 + 40 * requestProgress} />}
        {offline && t >= m.error && t < offlineYawn.time && <g>
          <RoughDrawing seed={415} options={{ ...outline, stroke: PALETTE.accent, strokeWidth: 4 }}
            build={(g, o) => [g.line(1282, 456, 1310, 484, o), g.line(1310, 456, 1282, 484, o)]} />
        </g>}
        {offline && t >= m.remedy && <g opacity={ramp(t, m.remedy, m.circle)}>
          <text x={405} y={814} textAnchor="middle" fontFamily={HAND_FONT} fontSize={40} fill={PALETTE.accent}>
            {language === "en" ? "move the mouse" : "移动鼠标"}
          </text>
          <RedPenCircle cx={405} cy={801} rx={language === "en" ? 190 : 125} ry={37}
            progress={ramp(t, m.circle, m.circleEnd)} seed={416} />
          <RedPenArrow x1={620} y1={799} x2={1280} y2={735} bend={35}
            progress={ramp(t, m.circleEnd, m.handTouch)} seed={417} />
        </g>}
        {handOpacity > 0 && <NudgingHand x={handX} y={585} opacity={handOpacity} />}
        {t >= m.nudge && t < m.handLeave && <RedPenArrow x1={1370} y1={750} x2={1440} y2={750}
          progress={ramp(t, m.nudge, m.nudgeEnd)} seed={418} />}
        {answered && <RedPenTick x={1090} y={offline ? 609 : 444} size={38}
          progress={ramp(t, answerAt, offline ? m.retryTick : m.idleTick)} seed={419} />}
      </g>
    </svg>
  </AbsoluteFill>;
};

const ColdReadClock: React.FC<{ language: Language; progress: number }> = ({ language, progress }) => <g>
  <text x={920} y={189} textAnchor="middle" fontFamily={HAND_FONT} fontSize={40} fill={PALETTE.ink}>
    {language === "en" ? "Cold read" : "冷读取"}
  </text>
  <text x={920} y={237} textAnchor="middle" fontFamily={HAND_FONT} fontSize={31} fill={PALETTE.pencil}>
    {language === "en" ? "takes longer" : "需要多等一会儿"}
  </text>
  <g transform="translate(1140 201)">
    <circle r={45} fill={PALETTE.cream} />
    <RoughDrawing seed={420} options={outline} build={(g, o) => [g.circle(0, 0, 90, o),
      ...[[0, -36, 0, -29], [36, 0, 29, 0], [0, 36, 0, 29], [-36, 0, -29, 0]].map(([x1, y1, x2, y2]) => g.line(x1, y1, x2, y2, o))]} />
    <RoughDrawing seed={421} options={outline} build={(g, o) => [g.line(0, 0, -18, -10, o)]} />
    <g transform={`rotate(${progress * 300})`}>
      <RoughDrawing seed={422} options={{ ...outline, stroke: PALETTE.accent, strokeWidth: 3.5 }}
        build={(g, o) => [g.line(0, 5, 0, -32, o)]} />
    </g>
    <circle r={3} fill={PALETTE.graphite} />
  </g>
</g>;

const RequestPlane: React.FC<{ x: number; y: number }> = ({ x, y }) => <g transform={`translate(${x} ${y}) rotate(9)`}>
  <path d="M -44 -23 L 42 0 L -44 25 L -21 0 Z" fill={PALETTE.whitePaper} />
  <RoughDrawing seed={430} options={{ ...outline, strokeWidth: 2.5, fill: PALETTE.sky, fillStyle: "hachure", hachureGap: 8, fillWeight: 0.6 }}
    build={(g, o) => [g.polygon([[-44, -23], [42, 0], [-44, 25], [-21, 0]], o), g.line(-21, 0, 42, 0, o)]} />
  <text x={-8} y={-41} textAnchor="middle" fontFamily={HAND_FONT} fontSize={29} fill={PALETTE.pencil}>HID++</text>
</g>;

const HAND = "M -214 151 L -159 151 L -114 112 Q -97 101 -92 81 L -84 53 L -15 15 Q 8 5 0 -9 Q -7 -21 -24 -12 L -114 27 L -94 -9 Q -89 -20 -99 -25 Q -110 -30 -118 -18 L -153 31 L -174 69 L -215 99 Z";

/** The index finger contacts Clicky's left edge before the mouse moves. */
const NudgingHand: React.FC<{ x: number; y: number; opacity: number }> = ({ x, y, opacity }) => <g transform={`translate(${x} ${y})`} opacity={opacity}>
  <path d={HAND} fill={PALETTE.cream} />
  <RoughDrawing seed={440} options={{ ...outline, fill: PALETTE.paperShade, fillStyle: "hachure", hachureAngle: -35, hachureGap: 8, fillWeight: 1 }}
    build={(g, o) => [g.path(HAND, o), g.path("M -146 66 Q -125 60 -110 72 M -161 83 Q -140 80 -122 91 M -186 118 L -161 140", { ...o, fill: undefined })]} />
</g>;
