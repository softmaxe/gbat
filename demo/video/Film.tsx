import { AbsoluteFill, Sequence } from "remotion";
import { BEATS, toFrame } from "../timeline";
import type { Language } from "../timeline";
import { Beat1Opening } from "./beats/Beat1Opening";
import { Beat2OldWay } from "./beats/Beat2OldWay";
import { Beat3OneCommand } from "./beats/Beat3OneCommand";
import { Beat4WakeIt } from "./beats/Beat4WakeIt";
import { Beat5WhereItFits } from "./beats/Beat5WhereItFits";
import { Beat6Ending } from "./beats/Beat6Ending";
import { Paper } from "./components/Paper";
import { Captions } from "./components/Captions";
import { loadFonts } from "./fonts";

loadFonts();
const pictures = [Beat1Opening, Beat2OldWay, Beat3OneCommand, Beat4WakeIt, Beat5WhereItFits, Beat6Ending];

export const Film: React.FC<{ language: Language }> = ({ language }) => (
  <AbsoluteFill>
    <Paper />
    {BEATS.map((beat, index) => {
      const Picture = pictures[index];
      return <Sequence key={beat.id} from={toFrame(beat.start)} durationInFrames={toFrame(beat.end - beat.start)}><Picture language={language} /></Sequence>;
    })}
    <Captions language={language} />
  </AbsoluteFill>
);
