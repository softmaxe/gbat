import type { Beat } from "../types";
import { EXAMPLE_READINGS } from "../readings";

const moments = {
  raycast: 42.2,
  raycastSettle: 42.65,
  shell: 45.2,
  shellSettle: 45.65,
  scripts: 48.2,
  scriptsSettle: 48.65,
};

export const whereItFits = {
  id: "where-it-fits", start: 42, end: 52,
  title: { en: "Where it fits", "zh-CN": "随处可用" },
  captions: [
    { id: "fits-raycast", start: moments.raycast, end: moments.shell, text: { en: "A quick glance in Raycast.", "zh-CN": "在 Raycast 中，一眼查看电量。" } },
    { id: "fits-shell", start: moments.shell, end: moments.scripts, text: { en: "Use it right in your shell.", "zh-CN": "也能直接用在终端里。" } },
    { id: "fits-scripts", start: moments.scripts, end: 51.5, text: { en: "Or add it to scripts and a status bar.", "zh-CN": "或接入脚本和状态栏。" } },
  ],
  moments,
  readings: [EXAMPLE_READINGS.wireless],
  cues: [], reviewFrames: [44, 47, 50],
} satisfies Beat;
