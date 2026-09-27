import type { Language } from "../../timeline";
import { whereItFits } from "../../timeline/beats/beat5-where-it-fits";
import { Placeholder } from "../components/Placeholder";

export const Beat5WhereItFits: React.FC<{ language: Language }> = ({ language }) => (
  <Placeholder beat={whereItFits} language={language} number={5} />
);
