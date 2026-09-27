import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

export interface BatteryCellsProps {
  /** Top-left corner in the parent SVG. */
  x: number;
  y: number;
  /** Percentage. Each cell can be partly filled. */
  level: number;
  width?: number;
  height?: number;
  charging?: boolean;
  time?: number;
  seed?: number;
}

const HEALTHY = "#7f9d7b";

/** Five pencil cells form Clicky's visible health bar. */
export const BatteryCells: React.FC<BatteryCellsProps> = ({
  x, y, level, width = 154, height = 24, charging = false, time = 0, seed = 90,
}) => {
  const percentage = Number.isFinite(level) ? Math.min(100, Math.max(0, level)) : 0;
  const gap = width * 0.035;
  const cellWidth = (width - gap * 4) / 5;
  const color = charging ? PALETTE.sky : percentage <= 20 ? PALETTE.accent : percentage <= 40 ? PALETTE.sunsetGold : HEALTHY;
  return <g transform={`translate(${x} ${y})`}>
    {Array.from({ length: 5 }, (_, index) => {
      const fill = Math.min(1, Math.max(0, percentage / 20 - index));
      const left = index * (cellWidth + gap);
      const fillWidth = Math.max(0, (cellWidth - 4) * fill);
      return <g key={index}>
        <rect x={left} width={cellWidth} height={height} fill={PALETTE.whitePaper} />
        {fillWidth > 0 && <g opacity={charging ? 0.75 + 0.2 * Math.sin(time * 6 - index) : 1}>
          <rect x={left + 2} y={2} width={fillWidth} height={height - 4} fill={color} opacity={0.46} />
          <RoughDrawing seed={seed + index} deps={[left, fillWidth, height, color]}
            options={{ stroke: "none", fill: color, fillStyle: "hachure", hachureAngle: -52, hachureGap: 3, fillWeight: 1.5, roughness: 0.8 }}
            build={(g, o) => [g.rectangle(left + 2, 2, fillWidth, height - 4, o)]} />
        </g>}
        <RoughDrawing seed={seed + 10 + index} deps={[left, cellWidth, height]}
          options={{ stroke: PALETTE.graphite, strokeWidth: 1.6, roughness: 0.7, bowing: 0.5 }}
          build={(g, o) => [g.rectangle(left, 0, cellWidth, height, o)]} />
      </g>;
    })}
    {charging && <g transform={`translate(${width + 12} ${height / 2})`}>
      <RoughDrawing seed={seed + 20} options={{ stroke: PALETTE.accent, strokeWidth: 1.7, fill: PALETTE.sunsetGold, fillStyle: "solid", roughness: 0.6 }}
        build={(g, o) => [g.polygon([[4, -17], [-8, 1], [1, 1], [-3, 17], [13, -4], [4, -4]], o)]} />
    </g>}
  </g>;
};
