import type { ReactNode } from "react";
import { AbsoluteFill } from "remotion";
import type { Language } from "../../timeline";
import { whereItFits } from "../../timeline/beats/beat5-where-it-fits";
import { ramp, useBeatTime } from "../anim";
import { HandDrawnPanel } from "../components/HandDrawnPanel";
import { HandDrawnTerminal, TERMINAL_FONT } from "../components/HandDrawnTerminal";
import { RedPenArrow } from "../components/RedPen";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

const m = whereItFits.moments;
const [reading] = whereItFits.readings;
const UI_FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif";
const pencil = { stroke: PALETTE.pencil, strokeWidth: 2.2, roughness: 1.2 };

export const Beat5WhereItFits: React.FC<{ language: Language }> = ({ language }) => {
  const { t } = useBeatTime(whereItFits);
  return <AbsoluteFill>
    <svg width={1920} height={1080} viewBox="0 0 1920 1080">
      <text x={155} y={162} fontFamily={HAND_FONT} fontSize={57} fill={PALETTE.ink}
        opacity={ramp(t, whereItFits.start, m.raycastSettle)}>{whereItFits.title[language]}</text>
      <StickyNote x={150} y={310} angle={-3} time={t} appear={m.raycast} settle={m.raycastSettle}
        title="Raycast" seed={500} tape={PALETTE.sunsetGold}>
        <HandDrawnPanel x={35} y={132} width={430} height={195} seed={510}>
          <RoughDrawing seed={511} options={pencil}
            build={(g, o) => [g.circle(37, 35, 23, o), g.line(45, 44, 56, 56, o), g.line(0, 75, 430, 75, o)]} />
          <text x={79} y={48} fontFamily={UI_FONT} fontSize={32} fill={PALETTE.ink}>gbat</text>
          <text x={28} y={120} fontFamily={UI_FONT} fontSize={31} fill={PALETTE.ink}>{reading.output}</text>
          <text x={28} y={161} fontFamily={UI_FONT} fontSize={23} fill={PALETTE.pencil}>
            {language === "en" ? "Mouse battery" : "鼠标电量"}
          </text>
          <RoughDrawing seed={512} options={{ ...pencil, stroke: PALETTE.accent }}
            build={(g, o) => [g.path("M 374 117 L 383 126 L 401 101", o)]} />
        </HandDrawnPanel>
      </StickyNote>
      <StickyNote x={710} y={327} angle={2} time={t} appear={m.shell} settle={m.shellSettle}
        title={language === "en" ? "Shell" : "终端"} seed={520} tape={PALETTE.sky}>
        <HandDrawnTerminal x={35} y={132} width={430} height={195} title="" seed={530}
          lines={["$ echo $(gbat)", reading.output]} fontSize={29} lineHeight={48} />
      </StickyNote>
      <StickyNote x={1270} y={303} angle={-1.5} time={t} appear={m.scripts} settle={m.scriptsSettle}
        title={language === "en" ? "Scripts + status bar" : "脚本与状态栏"} seed={540} tape={PALETTE.sunsetRose}>
        <HandDrawnPanel x={35} y={120} width={430} height={98} seed={550}>
          <text x={23} y={60} fontFamily={TERMINAL_FONT} fontSize={29} fill={PALETTE.ink}>battery=$(gbat)</text>
        </HandDrawnPanel>
        <RedPenArrow x1={250} y1={233} x2={250} y2={278} seed={552} />
        <HandDrawnPanel x={35} y={294} width={430} height={68} seed={553}>
          <RoughDrawing seed={554} options={pencil}
            build={(g, o) => [g.line(22, 22, 57, 22, o), g.line(22, 34, 57, 34, o), g.line(22, 46, 57, 46, o),
              g.line(88, 14, 88, 54, o)]} />
          <text x={408} y={44} textAnchor="end" fontFamily={UI_FONT} fontSize={28} fill={PALETTE.ink}>{reading.output}</text>
        </HandDrawnPanel>
      </StickyNote>
    </svg>
  </AbsoluteFill>;
};

interface StickyNoteProps {
  x: number;
  y: number;
  angle: number;
  time: number;
  appear: number;
  settle: number;
  title: string;
  seed: number;
  tape: string;
  children: ReactNode;
}

const StickyNote: React.FC<StickyNoteProps> = ({ x, y, angle, time, appear, settle, title, seed, tape, children }) => {
  if (time < appear) return null;
  const progress = ramp(time, appear, settle);
  const lift = (1 - progress) ** 3;
  return <g opacity={Math.min(1, progress * 6)}
    transform={`translate(${x + 250} ${y - lift * 34}) rotate(${angle + lift * 5}) scale(${1 + lift * 0.06}) translate(-250 0)`}>
    <HandDrawnPanel x={0} y={0} width={500} height={440} seed={seed} fill={PALETTE.cream}>
      <RoughDrawing seed={seed + 1} options={{ ...pencil, stroke: PALETTE.paperShade, strokeWidth: 1.5 }}
        build={(g, o) => [g.line(16, 419, 444, 419, o)]} />
      <path d="M 457 440 L 500 397 L 500 440 Z" fill={PALETTE.paperShade} />
      <RoughDrawing seed={seed + 2} options={{ ...pencil, strokeWidth: 1.8 }}
        build={(g, o) => [g.path("M 457 440 L 457 399 L 500 397", o)]} />
      <text x={250} y={77} textAnchor="middle" fontFamily={HAND_FONT} fontSize={42} fill={PALETTE.ink}>{title}</text>
      {children}
    </HandDrawnPanel>
    <g opacity={0.8}>
      <path d="M 187 -16 L 316 -13 L 312 22 L 184 18 Z" fill={tape} />
      <RoughDrawing seed={seed + 3} options={{ ...pencil, strokeWidth: 1.2, stroke: PALETTE.paperShade }}
        build={(g, o) => [g.polygon([[187, -16], [316, -13], [312, 22], [184, 18]], o)]} />
    </g>
  </g>;
};
