import { AbsoluteFill, interpolate } from "remotion";
import type { Language } from "../../timeline";
import { oneCommand } from "../../timeline/beats/beat3-one-command";
import { clamp, ramp, useBeatTime } from "../anim";
import { Clicky } from "../characters/Clicky";
import { HandDrawnTerminal, type TerminalLine } from "../components/HandDrawnTerminal";
import { RedPenArrow, RedPenCircle, RedPenTick } from "../components/RedPen";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

const m = oneCommand.moments;
const [wireless, plugged] = oneCommand.readings;
const typed = (t: number, times: number[]) => "gbat".slice(0, times.filter((time) => t >= time).length);
const outline = { stroke: PALETTE.graphite, strokeWidth: 3, roughness: 1.1 };

export const Beat3OneCommand: React.FC<{ language: Language }> = ({ language }) => {
  const { t } = useBeatTime(oneCommand);
  const charging = t >= m.charging;
  const reading = charging ? plugged : wireless;
  const signAt = charging ? m.chargingSign : m.sign;
  const holding = t >= signAt;
  const lines: TerminalLine[] = [`$ ${typed(t, m.typing)}`];
  if (t >= m.reading) lines.push(wireless.output);
  if (charging) lines.push(`$ ${typed(t, m.chargingTyping)}`);
  if (t >= m.chargingReading) lines.push(plugged.output);
  const cursor = t < m.request || charging && t < m.chargingRequest;
  const planeVisible = t >= m.request && t < m.mouse;
  const planeX = interpolate(t, [m.request, m.receiver, m.mouse], [810, 1100, 1430], clamp);
  const planeY = interpolate(t, [m.request, m.receiver, m.mouse], [430, 350, 460], clamp);
  const planeAngle = t < m.receiver ? -15 : 19;
  const response = ramp(t, m.sign, m.reading);
  const cable = ramp(t, m.charging, m.chargingRequest);
  const chargingPulse = ramp(t, m.chargingRequest, m.chargingSign);
  return <AbsoluteFill>
    <svg width={1920} height={1080} viewBox="0 0 1920 1080">
      <g opacity={ramp(t, oneCommand.start, oneCommand.start + 0.35)}>
        <text x={155} y={162} fontFamily={HAND_FONT} fontSize={57} fill={PALETTE.ink}>{oneCommand.title[language]}</text>
        <HandDrawnTerminal x={155} y={305} width={715} height={340} lines={lines} cursor={cursor} fontSize={34} />
        <text x={510} y={704} textAnchor="middle" fontFamily={HAND_FONT} fontSize={36} fill={PALETTE.ink}>Mac</text>
        <g opacity={charging ? 0.28 : 1}>
          <RoughDrawing seed={340} options={{ stroke: PALETTE.pencil, strokeWidth: 2, roughness: 0.8 }}
            progress={ramp(t, m.typing[0], m.request)}
            build={(g, o) => [g.path("M 880 476 Q 942 404 1050 458", o), g.path("M 1154 458 Q 1274 403 1406 478", o)]} />
          <Receiver />
          <text x={1100} y={638} textAnchor="middle" fontFamily={HAND_FONT} fontSize={36} fill={PALETTE.ink}>
            {language === "en" ? "Receiver" : "接收器"}
          </text>
        </g>
        <g opacity={ramp(t, m.request, m.request + 0.2)}>
          <text x={charging ? 1230 : 1110} y={236} textAnchor="middle" fontFamily={HAND_FONT} fontSize={35} fill={PALETTE.accent}>
            {charging ? "Device index 0xFF" : "Device index 1"}
          </text>
          <RedPenArrow x1={charging ? 1400 : 1190} y1={253} x2={charging ? 1455 : 1150} y2={charging ? 437 : 418}
            bend={-20} progress={ramp(t, m.request + 0.2, m.receiver)} />
        </g>
        {!charging && <text x={980} y={321} fontFamily={HAND_FONT} fontSize={33} fill={PALETTE.pencil}
          opacity={ramp(t, m.request, m.request + 0.25)}>HID++</text>}
        {planeVisible && <PaperPlane x={planeX} y={planeY} angle={planeAngle} />}
        <Clicky x={1600} y={672} scale={1.18} pose={holding ? "hold" : "play"}
          time={t - (holding ? signAt : charging ? m.charging : oneCommand.start)} batteryLevel={reading.level}
          charging={charging} signText={charging ? `${reading.level}%\n${language === "en" ? "charging" : "充电中"}` : `${reading.level}%`} />
        <text x={1600} y={734} textAnchor="middle" fontFamily={HAND_FONT} fontSize={36} fill={PALETTE.ink}>Clicky</text>
        {t >= m.sign && t < m.reading && <g transform={`translate(${interpolate(response, [0, 0.55, 1], [1410, 1100, 855])} ${interpolate(response, [0, 0.55, 1], [520, 520, 485])})`}>
          <rect x={-62} y={-31} width={124} height={62} fill={PALETTE.cream} />
          <RoughDrawing seed={350} options={outline} build={(g, o) => [g.rectangle(-62, -31, 124, 62, o)]} />
          <text textAnchor="middle" y={12} fontFamily={HAND_FONT} fontSize={35} fill={PALETTE.ink}>{wireless.level}%</text>
        </g>}
        {t >= m.reading && <RedPenTick x={787} y={437} size={38} progress={ramp(t, m.reading, m.reading + 0.3)} />}
        {charging && <>
          <RoughDrawing seed={360} progress={cable} options={{ ...outline, strokeWidth: 5 }}
            build={(g, o) => [g.path("M 839 646 L 839 712 Q 839 783 925 783 L 1370 783 Q 1430 783 1430 715 L 1430 618 Q 1430 579 1459 579", o)]} />
          <RoughDrawing seed={361} progress={cable} options={{ ...outline, fill: PALETTE.whitePaper, fillStyle: "solid" }}
            build={(g, o) => [g.rectangle(1450, 565, 25, 27, o)]} />
          <text x={1130} y={831} textAnchor="middle" fontFamily={HAND_FONT} fontSize={30} fill={PALETTE.pencil} opacity={cable}>USB</text>
          {t >= m.chargingRequest && t < m.chargingSign && <g transform={`translate(${interpolate(chargingPulse, [0, 0.2, 0.65, 0.93, 1], [839, 925, 1370, 1430, 1459])} ${interpolate(chargingPulse, [0, 0.2, 0.65, 0.93, 1], [646, 783, 783, 618, 579])})`}>
            <circle r={13} fill={PALETTE.sky} stroke={PALETTE.graphite} strokeWidth={2} />
            <text x={0} y={-28} textAnchor="middle" fontFamily={HAND_FONT} fontSize={28} fill={PALETTE.pencil}>HID++</text>
          </g>}
        </>}
        {t >= m.chargingReading && <>
          <RedPenCircle cx={553} cy={570} rx={111} ry={26} progress={ramp(t, m.chargingReading, m.chargingReading + 0.4)} />
          <RedPenTick x={787} y={543} size={38} progress={ramp(t, m.chargingReading + 0.15, m.chargingReading + 0.5)} seed={363} />
        </>}
      </g>
    </svg>
  </AbsoluteFill>;
};

const Receiver: React.FC = () => <g>
  <rect x={1056} y={472} width={88} height={107} fill={PALETTE.whitePaper} />
  <RoughDrawing seed={341} options={{ ...outline, fill: PALETTE.sky, fillStyle: "hachure", hachureGap: 8, fillWeight: 1 }}
    build={(g, o) => [g.rectangle(1056, 472, 88, 107, o), g.rectangle(1075, 423, 50, 49, { ...o, fill: PALETTE.paperShade })]} />
  <RoughDrawing seed={342} options={{ ...outline, strokeWidth: 2 }}
    build={(g, o) => [g.rectangle(1085, 432, 8, 15, o), g.rectangle(1108, 432, 8, 15, o), g.line(1081, 553, 1119, 553, o)]} />
</g>;

const PaperPlane: React.FC<{ x: number; y: number; angle: number }> = ({ x, y, angle }) => <g transform={`translate(${x} ${y}) rotate(${angle})`}>
  <path d="M -58 -28 L 50 0 L -58 30 L -29 0 Z" fill={PALETTE.whitePaper} />
  <RoughDrawing seed={345} options={{ ...outline, strokeWidth: 2.6, fill: PALETTE.sky, fillStyle: "hachure", hachureGap: 9, fillWeight: 0.7 }}
    build={(g, o) => [g.polygon([[-58, -28], [50, 0], [-58, 30], [-29, 0]], o), g.line(-29, 0, 50, 0, { ...o, fill: undefined })]} />
</g>;
