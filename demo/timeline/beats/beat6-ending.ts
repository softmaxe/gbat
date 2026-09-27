import type { Beat } from "../types";

export const INSTALL_COMMAND = "brew install softmaxe/tap/gbat";
export const REPOSITORY_URL = "github.com/softmaxe/gbat";

const writingTimes = (text: string, start: number, end: number): number[] =>
  Array.from(text, (_, index) => start + (end - start) * index / (text.length - 1));

const moments = {
  entranceComplete: 52.4,
  installWriting: writingTimes(INSTALL_COMMAND, 52.55, 54.25),
  repositoryWriting: writingTimes(REPOSITORY_URL, 54.45, 55.75),
  character: 54.35,
  characterComplete: 54.75,
  wave: 55.85,
  underlineComplete: 56.2,
  fadeStart: 59,
  fadeComplete: 59.9,
};

export const ending = {
  id: "ending", start: 52, end: 60,
  title: { en: "Meet gbat", "zh-CN": "试试 gbat" },
  captions: [
    { id: "ending-install", start: 52.5, end: 60, text: { en: "One command. Your battery, at a glance.", "zh-CN": "一条命令，电量一目了然。" } },
  ],
  moments,
  cues: [],
  reviewFrames: [53.5, 55.8, 57, 59.45, 59.9, 59.96666666666667],
} satisfies Beat;
