import type { Beat } from "../types";

export const whereItFits: Beat = {
  id: "where-it-fits", start: 42, end: 52,
  title: { en: "Where it fits", "zh-CN": "随处可用" },
  captions: [
    { id: "fits-raycast", start: 42.2, end: 45.2, text: { en: "A quick glance in Raycast.", "zh-CN": "在 Raycast 中，一眼查看电量。" } },
    { id: "fits-shell", start: 45.2, end: 48.2, text: { en: "Use it right in your shell.", "zh-CN": "也能直接用在终端里。" } },
    { id: "fits-scripts", start: 48.2, end: 51.5, text: { en: "Or add it to scripts and a status bar.", "zh-CN": "或接入脚本和状态栏。" } },
  ],
  cues: [], reviewFrames: [44, 47, 50],
};
