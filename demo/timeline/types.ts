/** All times are absolute seconds. Intervals include start and exclude end. */
import type { ExampleReading } from "./readings";

export type Seconds = number;
export type Language = "en" | "zh-CN";
export interface Caption {
  id: string;
  start: Seconds;
  end: Seconds;
  text: Record<Language, string>;
}
export interface Cue {
  id: string;
  kind: string;
  time: Seconds;
  gain?: number;
}
export interface Beat {
  id: string;
  start: Seconds;
  end: Seconds;
  title: Record<Language, string>;
  captions: Caption[];
  cues: Cue[];
  reviewFrames: Seconds[];
  /** Named action times and typing timestamps, in absolute film seconds. */
  moments?: Record<string, Seconds | Seconds[]>;
  /** Actual on-screen readings, shared with scene components and other Beats. */
  readings?: ExampleReading[];
}
export interface Film {
  fps: number;
  width: number;
  height: number;
  duration: Seconds;
  beats: Beat[];
}
