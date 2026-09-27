import type { Beat } from "../types";

export const oldWay: Beat = {
  id: "old-way", start: 8, end: 16,
  title: { en: "Keep it light", "zh-CN": "轻装上阵" },
  captions: [
    { id: "old-way-no-app", start: 8.5, end: 15.5, text: { en: "No G HUB. No background process.", "zh-CN": "无需 G HUB，也无需后台进程。" } },
  ],
  cues: [], reviewFrames: [12],
};
