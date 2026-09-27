import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { FILM } from "../timeline";
import { TIMELINE_JSON } from "./paths";

export function exportTimeline(output = TIMELINE_JSON): string {
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, JSON.stringify(FILM, null, 2) + "\n");
  return output;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  console.log(exportTimeline(process.argv[2]));
}
