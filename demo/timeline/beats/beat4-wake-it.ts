import type { Beat } from "../types";
import { EXAMPLE_READINGS } from "../readings";

const moments = {
  appear: 30.3,
  idleTyping: [30.55, 30.65, 30.75, 30.85],
  idleRequest: 31.05,
  idleWake: 31.65,
  idleAnswer: 34.05,
  idleTick: 34.4,
  offline: 35.5,
  offlineTyping: [35.65, 35.75, 35.85, 35.95],
  offlineRequest: 36.05,
  error: 36.5,
  remedy: 36.8,
  circle: 37.1,
  circleEnd: 37.55,
  handEnter: 37.9,
  handTouch: 38.45,
  nudge: 38.8,
  nudgeEnd: 39.1,
  offlineWake: 39.1,
  handLeave: 39.5,
  handGone: 39.9,
  retryTyping: [39.6, 39.7, 39.8, 39.9],
  retryRequest: 40,
  retryArrive: 40.2,
  retryAnswer: 40.75,
  retryTick: 41.05,
};

export const wakeIt = {
  id: "wake-it", start: 30, end: 42,
  title: { en: "Wake it", "zh-CN": "唤醒鼠标" },
  captions: [
    { id: "wake-idle", start: 30.5, end: 35.5, text: { en: "Idle? A Cold read takes a little longer.", "zh-CN": "待机时，冷读取需要多等一会儿。" } },
    { id: "wake-offline", start: 35.5, end: 41.5, text: { en: "Offline? Move the mouse, then try again.", "zh-CN": "离线时，移动鼠标后再试一次。" } },
  ],
  moments,
  readings: [EXAMPLE_READINGS.wireless],
  cues: [
    { id: "wake-idle-request", kind: "request_beep", time: moments.idleRequest, gain: 0.8 },
    { id: "wake-idle-yawn", kind: "wake_yawn", time: moments.idleWake, gain: 0.8 },
    { id: "wake-idle-answer", kind: "reading_ding", time: moments.idleAnswer, gain: 0.8 },
    { id: "wake-offline-request", kind: "request_beep", time: moments.offlineRequest, gain: 0.8 },
    { id: "wake-nudge", kind: "click", time: moments.nudge, gain: 0.6 },
    { id: "wake-offline-yawn", kind: "wake_yawn", time: moments.offlineWake, gain: 0.65 },
    { id: "wake-retry-request", kind: "request_beep", time: moments.retryRequest, gain: 0.8 },
    { id: "wake-retry-answer", kind: "reading_ding", time: moments.retryAnswer, gain: 0.8 },
  ],
  reviewFrames: [30.9, 31.35, 32.55, 34.65, 36.65, 37.7, 38.95, 39.35, 40.1, 41.2],
} satisfies Beat;
