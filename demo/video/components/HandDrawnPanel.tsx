import type { ReactNode } from "react";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

export interface HandDrawnPanelProps {
  x: number;
  y: number;
  width: number;
  height: number;
  seed?: number;
  fill?: string;
  /** Children use panel-local coordinates. */
  children?: ReactNode;
}

export const HandDrawnPanel: React.FC<HandDrawnPanelProps> = ({
  x, y, width, height, seed = 300, fill = PALETTE.whitePaper, children,
}) => <g transform={`translate(${x} ${y})`}>
  <rect width={width} height={height} fill={fill} />
  <RoughDrawing seed={seed} deps={[width, height]} options={{ stroke: PALETTE.graphite, strokeWidth: 3, roughness: 1.15, bowing: 0.8 }}
    build={(g, o) => [g.rectangle(0, 0, width, height, o)]} />
  {children}
</g>;
