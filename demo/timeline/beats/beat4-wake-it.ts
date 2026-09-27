import type { Beat } from "../types";

export const wakeIt: Beat = {
  id: "wake-it", start: 30, end: 42,
  title: { en: "Wake it", "zh-CN": "唤醒鼠标" },
  captions: [
    { id: "wake-idle", start: 30.5, end: 35.5, text: { en: "Idle? A Cold read takes a little longer.", "zh-CN": "待机时，冷读取需要多等一会儿。" } },
    { id: "wake-offline", start: 35.5, end: 41.5, text: { en: "Offline? Move the mouse, then try again.", "zh-CN": "离线时，移动鼠标后再试一次。" } },
  ],
  cues: [], reviewFrames: [33, 38, 41],
};
