import type { Language } from "../../timeline";
import { ending } from "../../timeline/beats/beat6-ending";
import { Placeholder } from "../components/Placeholder";

export const Beat6Ending: React.FC<{ language: Language }> = ({ language }) => (
  <Placeholder beat={ending} language={language} number={6} />
);
