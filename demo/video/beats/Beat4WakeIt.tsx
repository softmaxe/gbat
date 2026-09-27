import type { Language } from "../../timeline";
import { wakeIt } from "../../timeline/beats/beat4-wake-it";
import { Placeholder } from "../components/Placeholder";

export const Beat4WakeIt: React.FC<{ language: Language }> = ({ language }) => (
  <Placeholder beat={wakeIt} language={language} number={4} />
);
