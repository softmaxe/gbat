import { AbsoluteFill } from "remotion";
import type { Language } from "../../timeline";
import { ending, INSTALL_COMMAND, REPOSITORY_URL } from "../../timeline/beats/beat6-ending";
import { EXAMPLE_READINGS } from "../../timeline/readings";
import { ramp, useBeatTime } from "../anim";
import { Clicky } from "../characters/Clicky";
import { HandDrawnTerminal, TERMINAL_FONT } from "../components/HandDrawnTerminal";
import { HAND_FONT } from "../fonts";
import { RoughDrawing } from "../rough/RoughDrawing";
import { PALETTE } from "../theme";

const m = ending.moments;
const written = (text: string, times: number[], t: number): string =>
  text.slice(0, times.filter((time) => t >= time).length);

export const Beat6Ending: React.FC<{ language: Language }> = ({ language }) => {
  const { t } = useBeatTime(ending);
  const waving = t >= m.wave;
  const repository = written(REPOSITORY_URL, m.repositoryWriting, t);
  return <AbsoluteFill>
    <svg width={1920} height={1080} viewBox="0 0 1920 1080">
      <g opacity={ramp(t, ending.start, m.entranceComplete)}>
        <text x={200} y={190} fontFamily={HAND_FONT} fontSize={64} fill={PALETTE.ink}>{ending.title[language]}</text>
        <HandDrawnTerminal x={200} y={280} width={1510} height={235} seed={600}
          lines={[`$ ${written(INSTALL_COMMAND, m.installWriting, t)}`]} fontSize={56}
          cursor={t >= m.installWriting[0] && t < m.installWriting.at(-1)!} />
        <text x={235} y={690} fontFamily={TERMINAL_FONT} fontSize={46} fill={PALETTE.ink}>{repository}</text>
        <RoughDrawing seed={603} progress={ramp(t, m.wave, m.underlineComplete)}
          options={{ stroke: PALETTE.accent, strokeWidth: 4, roughness: 1, bowing: 0.7 }}
          build={(g, o) => [g.path("M 236 710 Q 562 723 870 710", o)]} />
        <Clicky x={1590} y={800} scale={1.15} pose={waving ? "wave" : "play"}
          time={waving ? t - m.wave : 0} batteryLevel={EXAMPLE_READINGS.wireless.level}
          draw={ramp(t, m.character, m.characterComplete)} />
      </g>
    </svg>
  </AbsoluteFill>;
};
