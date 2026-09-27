import type { Beat } from "../types";

export const oneCommand: Beat = {
  id: "one-command", start: 16, end: 30,
  title: { en: "One command", "zh-CN": "只需一条命令" },
  captions: [
    { id: "command-route", start: 16.5, end: 21, text: { en: "One command, through the Receiver.", "zh-CN": "一条命令，经由接收器读取电量。" } },
    { id: "command-index", start: 21, end: 25.5, text: { en: "The Device index identifies the mouse.", "zh-CN": "Device index 指定要读取的鼠标。" } },
    { id: "command-charge", start: 25.5, end: 29.5, text: { en: "Charging? The same command tells you.", "zh-CN": "正在充电？同一条命令就能告诉你。" } },
  ],
  cues: [], reviewFrames: [20, 24, 28],
};
