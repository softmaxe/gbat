import type { Beat } from "../types";
import { EXAMPLE_READINGS } from "../readings";

const moments = {
  typing: [16.8, 17.05, 17.3, 17.55],
  request: 18,
  receiver: 19.2,
  mouse: 20.8,
  sign: 21.15,
  reading: 22.2,
  charging: 25.5,
  chargingTyping: [25.5, 25.75, 26, 26.25],
  chargingRequest: 26.45,
  chargingSign: 27.3,
  chargingReading: 27.65,
};

export const oneCommand = {
  id: "one-command", start: 16, end: 30,
  title: { en: "One command", "zh-CN": "只需一条命令" },
  captions: [
    { id: "command-route", start: 16.5, end: 21, text: { en: "One command, through the Receiver.", "zh-CN": "一条命令，经由接收器读取电量。" } },
    { id: "command-index", start: 21, end: 25.5, text: { en: "The Device index identifies the mouse.", "zh-CN": "Device index 指定要读取的鼠标。" } },
    { id: "command-charge", start: 25.5, end: 29.5, text: { en: "Charging? The same command tells you.", "zh-CN": "正在充电？同一条命令就能告诉你。" } },
  ],
  moments,
  readings: [EXAMPLE_READINGS.wireless, EXAMPLE_READINGS.charging],
  cues: [
    ...moments.typing.map((time, index) => ({ id: `command-key-${index}`, kind: "click", time, gain: 0.22 })),
    { id: "command-request", kind: "request_beep", time: moments.request, gain: 0.8 },
    { id: "command-reading", kind: "reading_ding", time: moments.reading, gain: 0.8 },
    ...moments.chargingTyping.map((time, index) => ({ id: `command-charge-key-${index}`, kind: "click", time, gain: 0.22 })),
    { id: "command-charge-request", kind: "request_beep", time: moments.chargingRequest, gain: 0.8 },
    { id: "command-charge-reading", kind: "reading_ding", time: moments.chargingReading, gain: 0.8 },
  ],
  reviewFrames: [17.6, 18.65, 20.1, 21.7, 23.5, 26.9, 28.5],
} satisfies Beat;
