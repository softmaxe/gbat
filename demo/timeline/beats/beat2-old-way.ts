import type { Beat } from "../types";

const moments = {
  windowEnter: 8.1,
  windowReady: 8.6,
  spinnerStart: 8.6,
  spinnerStop: 13.1,
  processArrival: [9, 9.5, 10, 10.5, 11, 11.5],
  processLanded: [9.35, 9.85, 10.35, 10.85, 11.35, 11.85],
  windowStrike: 12.7,
  windowStruck: 13.1,
  windowCross: 13.1,
  windowCrossed: 13.45,
  processStrike: 13.5,
  processStruck: 13.9,
  processCross: 13.9,
  processCrossed: 14.25,
};

export const oldWay = {
  id: "old-way", start: 8, end: 16,
  title: { en: "Keep it light", "zh-CN": "轻装上阵" },
  captions: [
    { id: "old-way-no-app", start: 8.5, end: 15.5, text: { en: "gbat needs no G HUB or background process.", "zh-CN": "gbat 无需 G HUB，也无需后台进程。" } },
  ],
  moments,
  cues: [],
  reviewFrames: [9.4, 11.9, 13.2, 14.7],
} satisfies Beat;
