import { Composition } from "remotion";
import { FILM, toFrame } from "../timeline";
import { Film } from "./Film";

export const Root: React.FC = () => <>
  <Composition id="GbatEnglish" component={Film} defaultProps={{ language: "en" as const }} width={FILM.width} height={FILM.height} fps={FILM.fps} durationInFrames={toFrame(FILM.duration)} />
  <Composition id="GbatChinese" component={Film} defaultProps={{ language: "zh-CN" as const }} width={FILM.width} height={FILM.height} fps={FILM.fps} durationInFrames={toFrame(FILM.duration)} />
</>;
