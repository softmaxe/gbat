import { useId } from "react";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";
import { HandDrawnPanel } from "./HandDrawnPanel";

export type TerminalLine = string | { text: string; color?: string };
export const TERMINAL_FONT = "'SFMono-Regular', Menlo, Consolas, 'Liberation Mono', monospace";

export interface HandDrawnTerminalProps {
  x: number;
  y: number;
  width: number;
  height: number;
  title?: string;
  lines: readonly TerminalLine[];
  fontSize?: number;
  lineHeight?: number;
  seed?: number;
  cursor?: boolean;
}

/** Scene timing decides which text is visible; the frame keeps the text crisp. */
export const HandDrawnTerminal: React.FC<HandDrawnTerminalProps> = ({
  x, y, width, height, title = "Terminal", lines, fontSize = 34, lineHeight = 54, seed = 310, cursor = false,
}) => {
  const clipId = `terminal-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  return <HandDrawnPanel x={x} y={y} width={width} height={height} seed={seed}>
    <defs><clipPath id={clipId}><rect x={20} y={70} width={width - 40} height={height - 85} /></clipPath></defs>
    <RoughDrawing seed={seed + 1} deps={[width]} options={{ stroke: PALETTE.pencil, strokeWidth: 2, roughness: 0.9 }}
      build={(g, o) => [g.line(0, 62, width, 62, o), ...[28, 53, 78].map((cx) => g.circle(cx, 30, 10, o))]} />
    <text x={width / 2} y={42} textAnchor="middle" fontFamily={HAND_FONT} fontSize={29} fill={PALETTE.pencil}>{title}</text>
    <g clipPath={`url(#${clipId})`} fontFamily={TERMINAL_FONT} fontSize={fontSize} fill={PALETTE.ink}>
      {lines.map((line, index) => <text key={index} x={30} y={112 + index * lineHeight} xmlSpace="preserve">
        <tspan fill={typeof line === "string" ? PALETTE.ink : line.color ?? PALETTE.ink}>{typeof line === "string" ? line : line.text}</tspan>
        {cursor && index === lines.length - 1 && <tspan fill={PALETTE.pencil}>▌</tspan>}
      </text>)}
    </g>
  </HandDrawnPanel>;
};
