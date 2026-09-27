import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import type { Film } from "../timeline/types";
import { ROOT } from "../scripts/paths";

const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "gbat-timeline-"));
after(() => fs.rmSync(temporary, { recursive: true, force: true }));
const exported = path.join(temporary, "timeline.json");
execFileSync(process.execPath, ["--import", "tsx", "scripts/export-timeline.ts", exported], { cwd: ROOT });
const film: Film = JSON.parse(fs.readFileSync(exported, "utf8"));

test("the exported Beats tile the complete 60-second film", () => {
  assert.equal(film.beats.length, 6);
  assert.equal(film.fps, 30);
  assert.equal(film.width, 1920);
  assert.equal(film.height, 1080);
  assert.equal(film.duration, 60);
  let end = 0;
  for (const beat of film.beats) {
    assert.equal(beat.start, end, `${beat.id} has a gap or overlap`);
    assert.ok(beat.end > beat.start);
    end = beat.end;
  }
  assert.equal(end, film.duration);
});

test("captions are readable, ordered, non-overlapping and stay in their Beat", () => {
  const ids = new Set<string>();
  let previousEnd = 0;
  for (const beat of film.beats) {
    assert.ok(beat.captions.length > 0);
    for (const caption of beat.captions) {
      assert.ok(!ids.has(caption.id), `duplicate caption ${caption.id}`);
      ids.add(caption.id);
      assert.ok(caption.end - caption.start >= 2.5, caption.id);
      assert.ok(caption.start >= previousEnd, caption.id);
      assert.ok(caption.start >= beat.start && caption.end <= beat.end, caption.id);
      assert.deepEqual(Object.keys(caption.text).sort(), ["en", "zh-CN"]);
      assert.ok(caption.text.en.trim() && caption.text["zh-CN"].trim());
      previousEnd = caption.end;
    }
  }
});

test("cues are in narrative order, inside their Beat, and have synthesis modules", () => {
  const ids = new Set<string>();
  for (const beat of film.beats) {
    let previous = beat.start;
    for (const cue of beat.cues) {
      assert.ok(!ids.has(cue.id), `duplicate cue ${cue.id}`);
      ids.add(cue.id);
      assert.ok(cue.time >= previous && cue.time < beat.end, cue.id);
      assert.match(cue.kind, /^[a-z][a-z0-9_]*$/);
      assert.ok(fs.existsSync(path.join(ROOT, "audio/gbat_audio/cues", `${cue.kind}.py`)), cue.kind);
      assert.ok(cue.gain === undefined || Number.isFinite(cue.gain) && cue.gain > 0);
      previous = cue.time;
    }
    assert.ok(beat.reviewFrames.length > 0);
    for (const time of beat.reviewFrames) assert.ok(time >= beat.start && time < beat.end);
  }
});

test("captions avoid the repository glossary's forbidden synonyms", () => {
  const context = fs.readFileSync(path.join(ROOT, "../CONTEXT.md"), "utf8");
  const terms = [...context.matchAll(/^_Avoid_:\s*(.+)$/gm)]
    .flatMap((match) => match[1].replace(/\([^)]*\)/g, "").split(","))
    .map((term) => term.trim()).filter(Boolean);
  assert.ok(terms.length > 0);
  for (const beat of film.beats) for (const caption of beat.captions) for (const text of Object.values(caption.text)) {
    for (const term of terms) {
      const escaped = term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      assert.ok(!new RegExp(`\\b${escaped}\\b`, "i").test(text), `${caption.id}: ${term}`);
    }
  }
});

test("named visual moments stay inside their owning Beat", () => {
  for (const beat of film.beats) for (const [name, moments] of Object.entries(beat.moments ?? {})) {
    const times = Array.isArray(moments) ? moments : [moments];
    assert.ok(times.length > 0, `${beat.id}.${name}`);
    for (const [index, time] of times.entries()) {
      assert.ok(Number.isFinite(time) && time >= beat.start && time < beat.end, `${beat.id}.${name}`);
      if (index > 0) assert.ok(time >= times[index - 1], `${beat.id}.${name} is out of order`);
    }
  }
});

test("the film's actual reading data matches both README examples", () => {
  const readings = film.beats.flatMap((beat) => beat.readings ?? []);
  assert.ok(readings.length >= 2, "the exported timeline must include the displayed readings");
  for (const file of ["README.md", "README.zh-CN.md"]) {
    const markdown = fs.readFileSync(path.join(ROOT, "..", file), "utf8");
    const examples = markdown.match(/```(?:text)?\n(Battery: [^`]+)\n```/)?.[1].split("\n");
    assert.ok(examples?.length, `${file} must contain the battery example block`);
    assert.deepEqual([...new Set(readings.map((reading) => reading.output))].sort(), [...examples].sort(), file);
    for (const reading of readings) {
      const match = reading.output.match(/^Battery: (\d+)%((?: \(charging\))?)$/);
      assert.ok(match, reading.output);
      assert.equal(reading.level, Number(match[1]), "Clicky's battery level must match its output");
      assert.equal(reading.charging, Boolean(match[2]), "Clicky's charging state must match its output");
    }
  }
});

test("the bundled WOFF2 contains every caption glyph and fits the caption frame", () => {
  execFileSync("uv", ["run", "--project", "audio", "python", "scripts/check-font.py", exported], { cwd: ROOT, stdio: "pipe" });
});
