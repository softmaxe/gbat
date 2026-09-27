import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

interface PenProps { progress?: number; seed?: number }
interface LineProps extends PenProps { x1: number; y1: number; x2: number; y2: number }
const pen = { stroke: PALETTE.accent, strokeWidth: 4.5, roughness: 0.9, bowing: 0.6 };

export const RedPenCircle: React.FC<PenProps & { cx: number; cy: number; rx: number; ry: number }> = ({ cx, cy, rx, ry, progress = 1, seed = 320 }) =>
  <RoughDrawing seed={seed} progress={progress} deps={[cx, cy, rx, ry]} options={pen}
    build={(g, o) => [g.ellipse(cx, cy, rx * 2, ry * 2, o)]} />;

export const RedPenStrike: React.FC<LineProps> = ({ x1, y1, x2, y2, progress = 1, seed = 321 }) =>
  <RoughDrawing seed={seed} progress={progress} deps={[x1, y1, x2, y2]} options={pen}
    build={(g, o) => [g.line(x1, y1, x2, y2, o), g.line(x1 + 3, y1 + 8, x2 - 3, y2 + 5, o)]} />;

export const RedPenTick: React.FC<PenProps & { x: number; y: number; size?: number }> = ({ x, y, size = 45, progress = 1, seed = 322 }) =>
  <RoughDrawing seed={seed} progress={progress} deps={[x, y, size]} options={pen}
    build={(g, o) => [g.linearPath([[x, y + size * 0.52], [x + size * 0.32, y + size * 0.84], [x + size, y]], o)]} />;

export const RedPenArrow: React.FC<LineProps & { bend?: number }> = ({ x1, y1, x2, y2, bend = 0, progress = 1, seed = 323 }) => {
  const controlX = (x1 + x2) / 2;
  const controlY = (y1 + y2) / 2 + bend;
  const angle = Math.atan2(y2 - controlY, x2 - controlX);
  const head = 22;
  return <RoughDrawing seed={seed} progress={progress} deps={[x1, y1, x2, y2, bend]} options={pen}
    build={(g, o) => [g.path(`M ${x1} ${y1} Q ${controlX} ${controlY} ${x2} ${y2}`, o),
      g.linearPath([[x2 - head * Math.cos(angle - 0.5), y2 - head * Math.sin(angle - 0.5)], [x2, y2],
        [x2 - head * Math.cos(angle + 0.5), y2 - head * Math.sin(angle + 0.5)]], o)]} />;
};
