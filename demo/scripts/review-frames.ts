import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { renderStill, selectComposition } from "@remotion/renderer";
import { BEATS, toFrame } from "../timeline";
import { LANGUAGES, REVIEW, filmPath } from "./paths";

export function reviewFrameTimes(): { name: string; time: number }[] {
  return BEATS.flatMap((beat) => beat.reviewFrames.map((time) => ({ name: `${beat.id}-${toFrame(time)}`, time })));
}

export async function exportReviewFrames(serveUrl: string): Promise<void> {
  fs.mkdirSync(REVIEW, { recursive: true });
  for (const language of LANGUAGES) {
    for (const { name, time } of reviewFrameTimes()) {
      execFileSync("ffmpeg", ["-y", "-v", "error", "-ss", String(time), "-i", filmPath(language), "-frames:v", "1", path.join(REVIEW, `${language}-${name}.png`)]);
    }
  }
  await exportCharacterSheet(serveUrl);
}

export async function exportCharacterSheet(serveUrl: string): Promise<void> {
  fs.mkdirSync(REVIEW, { recursive: true });
  const composition = await selectComposition({ serveUrl, id: "ClickyCharacterSheet" });
  await renderStill({ serveUrl, composition, frame: toFrame(1), imageFormat: "png",
    output: path.join(REVIEW, "clicky-character-sheet.png") });
}
