import { useId } from "react";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";
import { BatteryCells } from "./BatteryCells";

export type ClickyPose = "play" | "collapse" | "idle" | "offline" | "wake" | "hold" | "wave";
type Eyes = "open" | "closed" | "happy" | "cross";

export interface ClickyProps {
  /** Ground point between the feet in the parent SVG. */
  x: number;
  y: number;
  scale?: number;
  pose?: ClickyPose;
  /** Seconds since this pose began. Scenes derive this from timeline moments. */
  time?: number;
  batteryLevel?: number;
  charging?: boolean;
  /** One or two short lines. Only shown in the hold pose. */
  signText?: string;
  rotate?: number;
  flip?: boolean;
  opacity?: number;
  /** Fixed pencil seed. Do not change on every frame. */
  seed?: number;
  draw?: number;
}

const BODY = "M -110 -34 L -119 -139 Q -122 -184 -91 -191 L 91 -191 Q 122 -184 119 -139 L 110 -34 Z";
const outline = { stroke: PALETTE.graphite, strokeWidth: 3.2, roughness: 1.1, bowing: 0.8 };
const hatch = { stroke: "none", fill: PALETTE.pencil, fillStyle: "hachure", hachureAngle: -52, hachureGap: 9, fillWeight: 1.1, roughness: 1.6 } as const;
const clamp = (value: number) => Math.min(1, Math.max(0, value));
const ease = (value: number) => { const p = clamp(value); return p * p * (3 - 2 * p); };

interface Motion {
  tilt: number;
  dx: number;
  dy: number;
  squash: number;
  leftArm: number;
  rightArm: number;
  stride: number;
  eyes: Eyes;
  mouth: "smile" | "flat" | "yawn";
}

function poseMotion(pose: ClickyPose, time: number): Motion {
  const t = Math.max(0, time);
  const motion: Motion = { tilt: 0, dx: 0, dy: 0, squash: 1, leftArm: -25, rightArm: -25, stride: 0, eyes: "open", mouth: "smile" };
  if (pose === "play") {
    motion.tilt = Math.sin(t * 9) * 4;
    motion.dy = -Math.abs(Math.sin(t * 9)) * 6;
    motion.leftArm = -34 + Math.sin(t * 12) * 13;
    motion.rightArm = -34 - Math.sin(t * 12) * 13;
    motion.stride = Math.sin(t * 9) * 11;
  } else if (pose === "collapse") {
    const fall = ease(t / 0.7);
    motion.tilt = 78 * fall;
    motion.dx = -70 * fall;
    motion.dy = -80 * fall;
    motion.squash = 1 + 0.3 * fall;
    motion.leftArm = -65;
    motion.rightArm = -65;
    motion.eyes = "cross";
    motion.mouth = "flat";
  } else if (pose === "idle") {
    motion.tilt = 9 + Math.sin(t * 2) * 2;
    motion.squash = 0.91 + Math.sin(t * 2) * 0.018;
    motion.leftArm = -65;
    motion.rightArm = -65;
    motion.eyes = "closed";
    motion.mouth = "flat";
  } else if (pose === "offline") {
    motion.tilt = 86;
    motion.dx = -76;
    motion.dy = -90;
    motion.squash = 1.35 + Math.sin(t * 1.2) * 0.01;
    motion.leftArm = -72;
    motion.rightArm = -72;
    motion.eyes = "closed";
    motion.mouth = "flat";
  } else if (pose === "wake") {
    const stretch = ease((t - 0.45) / 0.45) * (1 - ease((t - 1.35) / 0.6));
    motion.leftArm = -35 + 111 * stretch;
    motion.rightArm = -35 + 111 * stretch;
    motion.squash = 0.91 + 0.23 * stretch + 0.09 * ease((t - 1.6) / 0.4);
    motion.eyes = t < 1.45 ? "closed" : "open";
    motion.mouth = t < 1.4 ? "yawn" : "smile";
  } else if (pose === "hold") {
    motion.leftArm = 70;
    motion.rightArm = 70;
    motion.eyes = "happy";
    motion.dy = Math.sin(t * 2.5) * 2;
  } else if (pose === "wave") {
    motion.leftArm = -35;
    motion.rightArm = 60 + Math.sin(t * 9) * 23;
    motion.tilt = Math.sin(t * 3) * 3;
    motion.eyes = "happy";
  }
  if (motion.eyes === "open" && t % 2.8 > 2.67) motion.eyes = "closed";
  return motion;
}

/** White wireless mouse, drawn with a fixed pencil sketch and animated poses. */
export const Clicky: React.FC<ClickyProps> = ({
  x, y, scale = 1, pose = "play", time = 0, batteryLevel = 78, charging = false,
  signText = "78%", rotate = 0, flip = false, opacity = 1, seed = 19, draw = 1,
}) => {
  const motion = poseMotion(pose, time);
  const grainId = `clicky-grain-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const reveal = clamp(draw);
  const sideScale = flip ? -1 : 1;
  const bodyScaleX = pose === "collapse" ? 1 - 0.42 * ease(time / 0.7) : pose === "offline" ? 0.58 : 1 + (1 - motion.squash) * 0.35;
  const arm = (side: -1 | 1, angle: number) => <g transform={`rotate(${side === -1 ? angle : -angle} ${side * 109} -91)`}>
    <PencilBlock x={side === -1 ? -158 : 106} y={-104} width={52} height={28} seed={seed + 20 + side} />
  </g>;
  const sleeping = pose === "idle" || pose === "offline";
  return <g transform={`translate(${x} ${y}) rotate(${rotate}) scale(${scale})`} opacity={opacity * reveal}>
    <g transform={`scale(${sideScale} 1)`}>
      <g transform={`translate(${motion.dx} ${motion.dy}) rotate(${motion.tilt}) scale(${bodyScaleX} ${motion.squash})`}>
        <defs>
          <filter id={grainId} x="-5%" y="-5%" width="110%" height="110%">
            <feTurbulence type="fractalNoise" baseFrequency="0.9 0.35" numOctaves={2} seed={seed} result="grain" />
            <feColorMatrix in="grain" type="matrix" values="0 0 0 0 0.42  0 0 0 0 0.36  0 0 0 0 0.30  0 0 0 -2.4 1.75" />
            <feComposite in2="SourceGraphic" operator="in" />
          </filter>
        </defs>
        {[-80, -46, 46, 80].map((center, index) => <g key={center} transform={`rotate(${motion.stride * (index % 2 ? -1 : 1)} ${center} -36)`}>
          <PencilBlock x={center - 10} y={-41} width={20} height={41} seed={seed + 30 + index} />
        </g>)}
        {arm(-1, motion.leftArm)}
        {arm(1, motion.rightArm)}
        <path d={BODY} fill={PALETTE.whitePaper} />
        <path d={BODY} fill={PALETTE.pencil} filter={`url(#${grainId})`} opacity={0.13} />
        <g opacity={0.23}><RoughDrawing seed={seed + 100} options={hatch} build={(g, o) => [g.path(BODY, o)]} /></g>
        <RoughDrawing seed={seed} progress={clamp(reveal * 1.5)} options={outline} build={(g, o) => [g.path(BODY, o)]} />
        <RoughDrawing seed={seed + 2} options={{ ...outline, stroke: PALETTE.pencil, strokeWidth: 1.6, roughness: 0.6 }}
          build={(g, o) => [g.line(-108, -145, 108, -145, o), g.line(0, -145, 0, -126, o)]} />
        {motion.leftArm > 0 && arm(-1, motion.leftArm)}
        {motion.rightArm > 0 && arm(1, motion.rightArm)}
        <BatteryCells x={-77} y={-179} level={batteryLevel} charging={charging} time={time} seed={seed + 200} />
        <Eye x={-49} shape={motion.eyes} seed={seed + 5} />
        <Eye x={49} shape={motion.eyes} seed={seed + 6} />
        <RoughDrawing seed={seed + 7} options={{ ...outline, strokeWidth: 2.5, fill: PALETTE.pencil, fillStyle: "solid", roughness: 0.6 }}
          build={(g, o) => [g.rectangle(-10, -124, 20, 37, o)]} />
        <RoughDrawing seed={seed + 8} options={{ stroke: PALETTE.whitePaper, strokeWidth: 1.5, roughness: 0.4 }}
          build={(g, o) => [-115, -106, -97].map((wheelY) => g.line(-5, wheelY, 5, wheelY, o))} />
        <Mouth kind={motion.mouth} seed={seed + 9} />
        {pose === "hold" && <Sign text={signText} flip={flip} seed={seed + 40} />}
      </g>
      {sleeping && <g transform={`translate(${pose === "offline" ? 155 : 118} ${pose === "offline" ? -145 : -203})`}>
        {[0, 1, 2].slice(0, pose === "offline" ? 3 : 1).map((index) => <g key={index}
          transform={`translate(${index * 25} ${-index * 26 - Math.sin(time * 2 - index) * 5})`}
          opacity={0.55 + 0.2 * Math.sin(time * 2 - index)}>
          <RoughDrawing seed={seed + 60 + index} options={{ stroke: PALETTE.pencil, strokeWidth: 2.4, roughness: 0.8 }}
            build={(g, o) => [g.linearPath([[0, 0], [15, 0], [0, 16], [16, 16]], o)]} />
        </g>)}
      </g>}
    </g>
  </g>;
};

const PencilBlock: React.FC<{ x: number; y: number; width: number; height: number; seed: number }> = ({ x, y, width, height, seed }) => <>
  <rect x={x} y={y} width={width} height={height} fill={PALETTE.whitePaper} />
  <g opacity={0.26}><RoughDrawing seed={seed + 100} deps={[x, y, width, height]} options={hatch}
    build={(g, o) => [g.rectangle(x, y, width, height, o)]} /></g>
  <RoughDrawing seed={seed} deps={[x, y, width, height]} options={outline}
    build={(g, o) => [g.rectangle(x, y, width, height, o)]} />
</>;

const Eye: React.FC<{ x: number; shape: Eyes; seed: number }> = ({ x, shape, seed }) => <RoughDrawing
  seed={seed} deps={[x, shape]} options={{ stroke: PALETTE.ink, strokeWidth: 3.8, roughness: 0.7, bowing: 0.6 }}
  build={(g, o) => {
    if (shape === "closed") return [g.path(`M ${x - 12} -112 Q ${x} -105 ${x + 12} -112`, o)];
    if (shape === "happy") return [g.linearPath([[x - 12, -108], [x, -122], [x + 12, -108]], o)];
    if (shape === "cross") return [g.line(x - 10, -125, x + 10, -105, o), g.line(x + 10, -125, x - 10, -105, o)];
    return [g.rectangle(x - 8, -127, 16, 25, { ...o, fill: PALETTE.ink, fillStyle: "solid", strokeWidth: 1.6 })];
  }} />;

const Mouth: React.FC<{ kind: Motion["mouth"]; seed: number }> = ({ kind, seed }) => <RoughDrawing
  seed={seed} deps={[kind]} options={{ stroke: PALETTE.ink, strokeWidth: 2.8, roughness: 0.7 }}
  build={(g, o) => {
    if (kind === "yawn") return [g.ellipse(0, -65, 26, 33, { ...o, fill: PALETTE.ink, fillStyle: "solid" })];
    if (kind === "flat") return [g.line(-12, -62, 12, -62, o)];
    return [g.path("M -17 -68 Q 0 -48 17 -68", o)];
  }} />;

const Sign: React.FC<{ text: string; flip: boolean; seed: number }> = ({ text, flip, seed }) => {
  const lines = text.split("\n").slice(0, 2);
  return <g>
    <RoughDrawing seed={seed} options={{ ...outline, strokeWidth: 5 }}
      build={(g, o) => [g.line(-126, -132, -97, -237, o), g.line(126, -132, 97, -237, o)]} />
    <rect x={-133} y={-335} width={266} height={100} fill={PALETTE.cream} />
    <RoughDrawing seed={seed + 1} options={outline} build={(g, o) => [g.rectangle(-133, -335, 266, 100, o)]} />
    <g transform={`scale(${flip ? -1 : 1} 1)`} fontFamily={HAND_FONT} fontSize={lines.length > 1 ? 32 : 45} fill={PALETTE.ink} textAnchor="middle">
      {lines.map((line, index) => <text key={index} x={0} y={lines.length > 1 ? -295 + index * 39 : -273}>{line}</text>)}
    </g>
  </g>;
};
