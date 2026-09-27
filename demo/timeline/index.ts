import { opening } from "./beats/beat1-opening";
import { oldWay } from "./beats/beat2-old-way";
import { oneCommand } from "./beats/beat3-one-command";
import { wakeIt } from "./beats/beat4-wake-it";
import { whereItFits } from "./beats/beat5-where-it-fits";
import { ending } from "./beats/beat6-ending";
import type { Beat, Film } from "./types";

export * from "./types";
export const FPS = 30;
export const DURATION = 60;
export const BEATS = [opening, oldWay, oneCommand, wakeIt, whereItFits, ending];
export const FILM: Film = { fps: FPS, width: 1920, height: 1080, duration: DURATION, beats: BEATS };
export const toFrame = (seconds: number): number => Math.round(seconds * FPS);

/** Look up the shared cue time when synchronising an action with its sound. */
export function cueTime(beat: Beat, id: string): number {
  const cue = beat.cues.find((item) => item.id === id);
  if (!cue) throw new Error(`Unknown cue ${id} in ${beat.id}`);
  return cue.time;
}
