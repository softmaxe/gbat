import type { Beat } from "../types";

export const openingMoments = {
  play: 0,
  sceneReady: 0.65,
  clickEnd: 1.55,
  drain: 2.4,
  red: 4.7,
  collapse: 5.3,
  collapsed: 6,
  question: 6.1,
  questionReady: 6.5,
} as const;

export const opening: Beat & { moments: typeof openingMoments } = {
  id: "opening", start: 0, end: 8,
  moments: openingMoments,
  title: { en: "How much charge is left?", "zh-CN": "鼠标还剩多少电？" },
  captions: [
    { id: "opening-charge", start: openingMoments.drain, end: 7.8, text: { en: "How much charge is left?", "zh-CN": "鼠标还剩多少电？" } },
  ],
  cues: [{ id: "opening-click", kind: "click", time: 1.2 }],
  reviewFrames: [1.3, 3.7, 5.1, 6.7],
};
