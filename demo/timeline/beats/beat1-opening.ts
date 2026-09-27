import type { Beat } from "../types";

export const opening: Beat = {
  id: "opening", start: 0, end: 8,
  title: { en: "How much charge is left?", "zh-CN": "鼠标还剩多少电？" },
  captions: [
    { id: "opening-charge", start: 0.5, end: 7.5, text: { en: "How much charge is left?", "zh-CN": "鼠标还剩多少电？" } },
  ],
  cues: [{ id: "opening-click", kind: "click", time: 1.2 }],
  reviewFrames: [2, 6.5],
};
