import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { bundle } from "@remotion/bundler";
import { renderMedia, selectComposition } from "@remotion/renderer";
import { FILM } from "../timeline";
import { exportTimeline } from "./export-timeline";
import { exportReviewFrames } from "./review-frames";
import { sourceHash } from "./source-hash";
import { AUDIO_WAV, BUILD, COMPOSITIONS, LANGUAGES, ROOT, TIMELINE_JSON, filmPath } from "./paths";

async function main(): Promise<void> {
  const requested = process.argv.find((argument) => argument.startsWith("--concurrency="))?.split("=")[1];
  if (requested !== undefined && !/^[1-9]\d*$/.test(requested)) throw new Error("--concurrency must be a positive integer");
  const concurrency = requested ? Number(requested) : Math.min(8, Math.max(1, Math.floor(os.availableParallelism() / 2)));
  fs.mkdirSync(BUILD, { recursive: true });
  fs.rmSync(path.join(BUILD, "manifest.json"), { force: true });
  console.log("Exporting timeline and synthesising audio");
  exportTimeline();
  execFileSync("uv", ["run", "--project", "audio", "python", "-m", "gbat_audio", "--timeline", TIMELINE_JSON, "--output", AUDIO_WAV], { cwd: ROOT, stdio: "inherit" });
  const digest = sourceHash();
  const serveUrl = await bundle({ entryPoint: path.join(ROOT, "video/index.ts"), publicDir: path.join(ROOT, "video/public") });
  for (const language of LANGUAGES) {
    const composition = await selectComposition({ serveUrl, id: COMPOSITIONS[language] });
    const video = path.join(BUILD, `${language}-video.mp4`);
    console.log(`Rendering ${language}, ${FILM.duration} seconds at ${FILM.width}x${FILM.height}`);
    let reported = -1;
    await renderMedia({ serveUrl, composition, codec: "h264", crf: 18, pixelFormat: "yuv420p", muted: true,
      imageFormat: "jpeg", jpegQuality: 92, concurrency, outputLocation: video,
      onProgress: ({ progress }) => {
        const percentage = Math.floor(progress * 10) * 10;
        if (percentage !== reported) { reported = percentage; console.log(`${language}: ${percentage}%`); }
      },
    });
    execFileSync("ffmpeg", ["-y", "-v", "error", "-i", video, "-i", AUDIO_WAV, "-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-t", String(FILM.duration), "-movflags", "+faststart", filmPath(language)], { stdio: "inherit" });
    fs.rmSync(video);
  }
  await exportReviewFrames(serveUrl);
  if (sourceHash() !== digest) throw new Error("Source changed during the build. Rebuild before testing the film.");
  fs.writeFileSync(path.join(BUILD, "manifest.json"), JSON.stringify({ sourceHash: digest, builtAt: new Date().toISOString() }, null, 2) + "\n");
  console.log(`Films and review frames are ready in ${BUILD}`);
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
