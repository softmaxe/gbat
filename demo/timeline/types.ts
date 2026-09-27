/** All times are absolute seconds. Intervals include start and exclude end. */
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
}
export interface Film {
  fps: number;
  width: number;
  height: number;
  duration: Seconds;
  beats: Beat[];
}
