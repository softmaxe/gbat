import type { Language } from "../../timeline";
import { oneCommand } from "../../timeline/beats/beat3-one-command";
import { Placeholder } from "../components/Placeholder";

export const Beat3OneCommand: React.FC<{ language: Language }> = ({ language }) => (
  <Placeholder beat={oneCommand} language={language} number={3} />
);
