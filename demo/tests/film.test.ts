import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { BUILD, LANGUAGES, REVIEW, filmPath } from "../scripts/paths";
import { reviewFrameTimes } from "../scripts/review-frames";
import { sourceHash } from "../scripts/source-hash";

test("movies were built from the current source", () => {
  const manifest = path.join(BUILD, "manifest.json");
  assert.ok(fs.existsSync(manifest), "Run npm run build before film tests.");
  assert.equal(JSON.parse(fs.readFileSync(manifest, "utf8")).sourceHash, sourceHash(), "Sources changed. Run npm run build before film tests.");
});

for (const language of LANGUAGES) {
  test(`${language}: 60 seconds of 1080p H.264 at 30 fps with AAC`, () => {
    const probe = JSON.parse(execFileSync("ffprobe", ["-v", "error", "-show_streams", "-show_format", "-of", "json", filmPath(language)], { encoding: "utf8" }));
    assert.equal(probe.streams.length, 2);
    const videos = probe.streams.filter((stream: { codec_type: string }) => stream.codec_type === "video");
    const audios = probe.streams.filter((stream: { codec_type: string }) => stream.codec_type === "audio");
    assert.equal(videos.length, 1);
    assert.equal(audios.length, 1);
    const video = videos[0];
    assert.equal(video.codec_name, "h264");
    assert.equal(video.width, 1920);
    assert.equal(video.height, 1080);
    assert.equal(video.r_frame_rate, "30/1");
    // ffprobe names full-range JPEG-derived 4:2:0 output yuvj420p.
    assert.ok(["yuv420p", "yuvj420p"].includes(video.pix_fmt), video.pix_fmt);
    assert.equal(audios[0].codec_name, "aac");
    for (const duration of [probe.format.duration, video.duration, audios[0].duration]) assert.ok(Math.abs(Number(duration) - 60) < 0.1);
  });

  test(`${language}: every review frame is a nonempty PNG`, () => {
    for (const { name } of reviewFrameTimes()) {
      const png = fs.readFileSync(path.join(REVIEW, `${language}-${name}.png`));
      assert.equal(png.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
      assert.ok(png.length > 1000);
      assert.equal(png.readUInt32BE(16), 1920);
      assert.equal(png.readUInt32BE(20), 1080);
    }
  });

  // Ticket #26 enables this check after the final score replaces the music bed.
  test(`${language}: final second is below -40 dB`, { skip: process.env.CHECK_FINAL_SILENCE !== "1" }, () => {
    const pcm = execFileSync("ffmpeg", ["-v", "error", "-sseof", "-1", "-i", filmPath(language), "-vn", "-ac", "1", "-ar", "48000", "-f", "f32le", "-"], { maxBuffer: 1024 * 1024 });
    assert.ok(pcm.length >= 47000 * 4, "ffmpeg must decode the final audio second");
    let energy = 0;
    for (let offset = 0; offset < pcm.length; offset += 4) energy += pcm.readFloatLE(offset) ** 2;
    const rms = Math.sqrt(energy / (pcm.length / 4));
    assert.ok(20 * Math.log10(rms) < -40, `Final RMS: ${20 * Math.log10(rms)} dB`);
  });
}
