import path from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const BUILD = path.join(ROOT, "build");
export const TIMELINE_JSON = path.join(BUILD, "timeline.json");
export const AUDIO_WAV = path.join(BUILD, "audio.wav");
export const REVIEW = path.join(BUILD, "review");
export const LANGUAGES = ["en", "zh-CN"] as const;
export const COMPOSITIONS = { en: "GbatEnglish", "zh-CN": "GbatChinese" } as const;
export const filmPath = (language: string): string => path.join(BUILD, `gbat-${language}.mp4`);
