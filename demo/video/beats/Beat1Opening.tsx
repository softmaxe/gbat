import type { Language } from "../../timeline";
import { opening } from "../../timeline/beats/beat1-opening";
import { Placeholder } from "../components/Placeholder";

export const Beat1Opening: React.FC<{ language: Language }> = ({ language }) => (
  <Placeholder beat={opening} language={language} number={1} />
);
