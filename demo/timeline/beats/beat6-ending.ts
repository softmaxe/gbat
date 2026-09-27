import type { Beat } from "../types";

export const ending: Beat = {
  id: "ending", start: 52, end: 60,
  title: { en: "Meet gbat", "zh-CN": "试试 gbat" },
  captions: [
    { id: "ending-install", start: 52.5, end: 58.5, text: { en: "One command. Your battery, at a glance.", "zh-CN": "一条命令，电量一目了然。" } },
  ],
  cues: [], reviewFrames: [56, 59.9],
};
