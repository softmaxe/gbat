import { cueTime, type Language } from "../../timeline";
import { opening, openingMoments } from "../../timeline/beats/beat1-opening";
import { ramp, useBeatTime } from "../anim";
import { Clicky } from "../characters/Clicky";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

const pencil = { stroke: PALETTE.graphite, strokeWidth: 3.5, roughness: 1.3, bowing: 0.8 };
const pad = [[894, 511], [1585, 496], [1727, 752], [794, 768]] as [number, number][];

export const Beat1Opening: React.FC<{ language: Language }> = () => {
  const { t } = useBeatTime(opening);
  const click = cueTime(opening, "opening-click");
  const reveal = ramp(t, openingMoments.play, openingMoments.sceneReady);
  const clickProgress = ramp(t, click, openingMoments.clickEnd);
  const clickFlash = t >= click ? 1 - clickProgress : 0;
  const drain = ramp(t, openingMoments.drain, openingMoments.red);
  const batteryLevel = 92 - 80 * drain;
  const collapsed = t >= openingMoments.collapse;
  const fall = ramp(t, openingMoments.collapse, openingMoments.collapsed);
  const playTime = Math.min(t, openingMoments.collapse) - openingMoments.play;
  const x = 1270 + Math.sin(playTime * 2.6) * 36 * (1 - fall);
  const question = ramp(t, openingMoments.question, openingMoments.questionReady);
  const gameTime = Math.min(t, openingMoments.collapse);

  return <svg width={1920} height={1080} viewBox="0 0 1920 1080" style={{ position: "absolute", inset: 0 }}>
    <g opacity={reveal}>
      <polygon points={pad.map((point) => point.join(",")).join(" ")} fill={PALETTE.cream} />
      <RoughDrawing seed={211} options={{ ...pencil, fill: PALETTE.sky, fillStyle: "hachure", hachureAngle: -18, hachureGap: 17, fillWeight: 0.9 }}
        build={(g, o) => [g.polygon(pad, o)]} />
      <RoughDrawing seed={212} options={{ stroke: PALETTE.pencil, strokeWidth: 2, roughness: 1.4 }}
        build={(g, o) => [g.line(799, 780, 1734, 765), g.line(1734, 765, 1593, 504)]} />

      <RoughDrawing seed={214} options={{ ...pencil, fill: PALETTE.paperShade, fillStyle: "hachure", hachureGap: 8 }}
        build={(g, o) => [g.polygon([[510, 596], [575, 596], [584, 678], [494, 678]], o), g.ellipse(541, 686, 247, 32, o)]} />
      <rect x={223} y={219} width={639} height={385} rx={12} fill={PALETTE.whitePaper} />
      <RoughDrawing seed={215} options={pencil}
        build={(g, o) => [g.rectangle(223, 219, 639, 385, o), g.rectangle(245, 242, 595, 331, o)]} />
      <RoughDrawing seed={216} options={{ stroke: PALETTE.pencil, strokeWidth: 1.6, roughness: 0.9 }}
        build={(g, o) => [g.line(262, 519, 823, 519), g.line(287, 542, 351, 542), g.line(753, 542, 812, 542)]} />

      <g opacity={1 - fall * 0.6}>
        <g transform={`translate(${320 + Math.sin(gameTime * 2.6) * 34} 490)`}>
          <RoughDrawing seed={218} options={{ ...pencil, fill: PALETTE.sky, fillStyle: "hachure", hachureGap: 5 }}
            build={(g, o) => [g.polygon([[-33, 0], [-20, -36], [8, -48], [23, -20], [43, 0]], o)]} />
        </g>
        <RoughDrawing seed={219} options={{ stroke: PALETTE.pencil, strokeWidth: 2.2, roughness: 1.1 }}
          build={(g, o) => [g.line(404, 359, 448, 359), g.line(495, 301, 573, 301), g.line(658, 472, 716, 472)]} />
        <g transform={`translate(${669 + Math.sin(gameTime * 1.8) * 37} ${367 + Math.cos(gameTime * 2) * 16})`}>
          <g transform={`scale(${1 + clickFlash * 0.24})`}>
            <RoughDrawing seed={220} options={{ ...pencil, stroke: PALETTE.accent, fill: PALETTE.sunsetGold, fillStyle: "hachure", hachureGap: 5 }}
              build={(g, o) => [g.polygon([[0, -41], [13, -13], [43, -10], [21, 12], [27, 42], [0, 27], [-27, 42], [-21, 12], [-43, -10], [-13, -13]], o)]} />
          </g>
          <g opacity={clickFlash}>
            <RoughDrawing seed={221} options={{ stroke: PALETTE.accent, strokeWidth: 4, roughness: 1 }}
              build={(g, o) => [g.line(-65, -31, -89, -43, o), g.line(60, -41, 80, -61, o), g.line(3, -61, 5, -85, o), g.line(66, 26, 93, 38, o)]} />
          </g>
        </g>
      </g>

      <g transform={`translate(${x} 697)`}>
        <g opacity={clickFlash}>
          <RoughDrawing seed={224} options={{ stroke: PALETTE.accent, strokeWidth: 4, roughness: 1.1 }}
            build={(g, o) => [g.line(-267, -299, -305, -315, o), g.line(-261, -329, -278, -363, o), g.line(-276, -267, -315, -266, o)]} />
        </g>
      </g>
      <Clicky x={x} y={697 + clickFlash * 8} scale={2.15} pose={collapsed ? "collapse" : "play"}
        time={collapsed ? t - openingMoments.collapse : playTime} batteryLevel={batteryLevel} />

      <g opacity={question} transform={`translate(1471 ${264 + (1 - question) * 16})`}>
        <RoughDrawing seed={227} options={{ ...pencil, stroke: PALETTE.accent, fill: PALETTE.cream, fillStyle: "solid", strokeWidth: 3 }}
          build={(g, o) => [g.ellipse(0, 0, 134, 126, o), g.ellipse(-45, 88, 22, 19, o), g.ellipse(-70, 116, 11, 10, o)]} />
        <text x={0} y={23} textAnchor="middle" fontFamily={HAND_FONT} fontSize={82} fill={PALETTE.accent}>?</text>
      </g>
    </g>
  </svg>;
};
