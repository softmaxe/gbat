import type { Language } from "../../timeline";
import { oldWay } from "../../timeline/beats/beat2-old-way";
import { Placeholder } from "../components/Placeholder";

export const Beat2OldWay: React.FC<{ language: Language }> = ({ language }) => (
  <Placeholder beat={oldWay} language={language} number={2} />
);
