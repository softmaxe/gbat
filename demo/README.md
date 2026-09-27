# gbat explainer film

This directory builds a 60-second explainer film in English and Chinese. It is an animation with example readings, `Battery: 78%` and `Battery: 42% (charging)`, not a live capture. Building it does not query a mouse.

The Node and Python toolchain is separate from the Rust binary, tests, and release. The [existing recordings](../docs/recording.md) remain unchanged. Compression and embedding the film in the root READMEs are follow-up work.

## Build

Use Node 26.10.0, npm, uv, and FFmpeg with ffprobe. On macOS, install uv and FFmpeg with `brew install uv ffmpeg`. The Node version is recorded in `.node-version`; use your Node version manager to select it. The audio package requires Python 3.13, which uv can provision.

Install the locked dependencies once:

```sh
cd demo
npm ci
uv sync --project audio --locked
```

Then build both languages with one command. All remaining commands on this page run from `demo/` unless stated otherwise.

```sh
npm run build
```

The build exports `build/timeline.json`, synthesises `build/audio.wav`, renders both Remotion compositions, muxes H.264 video with AAC audio, and extracts PNG review frames. Both movies are 1920 x 1080 at 30 fps. Remotion downloads its headless browser on first use. Python dependencies are isolated in `audio/.venv` and locked by `audio/uv.lock`.

- `build/gbat-en.mp4`
- `build/gbat-zh-CN.mp4`
- `build/timeline.json` and `build/audio.wav`
- `build/review/<language>-<beat-id>-<frame>.png`
- `build/review/clicky-character-sheet.png`
- `build/manifest.json`, the completed build's source hash

Use `npm run build -- --concurrency=4` to set rendering concurrency. Generated outputs and dependencies are ignored by Git. Remove them with `rm -rf demo/build demo/node_modules demo/audio/.venv` from the repository root.

## Test and review

```sh
npm run typecheck
npm run test:timeline
npm run test:audio
npm run test:film
# Run every check after building:
npm test
```

These checks run locally; Rust CI does not run the demo tests. Timeline tests export their own JSON and inspect caption durations and ordering, vocabulary, example readings, cue synthesis modules, font glyph coverage, and caption widths measured from the bundled font. Audio tests export the timeline, then invoke the public Python CLI with JSON in and WAV out to check length and each isolated cue onset. Neither check needs a rendered movie.

Film tests require a completed `npm run build`. They inspect both movies with ffprobe, check PNG files, and require both RMS and peak audio levels in the complete final second to stay below -40 dB. They also compare `build/manifest.json` with the current files under `demo/`, including this README and the lockfiles. After an edit, rebuild before running `test:film` or `npm test`; the tests do not rebuild automatically. Dependencies, caches, and generated outputs are excluded from the hash.

Use `npm run studio` for frame-by-frame picture review. The `GbatEnglish` and `GbatChinese` compositions share action and cue timing. Studio previews the pictures; listen to `build/audio.wav` or the muxed MP4 for the soundtrack. Review the PNGs in both languages after changing a Beat; automated checks do not judge illustrations or text placement inside pictures.

For a quick picture check without a full build, render a still at an absolute frame number. This previews the Chinese charging example at 28.5 seconds, frame 855 at 30 fps. Substitute `GbatEnglish` for the English version.

```sh
npx --no-install remotion still video/index.ts GbatChinese build/review/zh-CN-preview.png --frame=855 --public-dir=video/public
```

The `--public-dir` option makes the bundled font available. A still preview does not refresh the movies or the build manifest.

`ClickyCharacterSheet` previews all seven animated poses over six seconds. The build exports its one-second frame to `build/review/clicky-character-sheet.png`. To render only this sheet:

```sh
npx --no-install remotion still video/index.ts ClickyCharacterSheet build/review/clicky-character-sheet.png --frame=30 --public-dir=video/public
```

Scenes mount `Clicky` from `video/characters/Clicky` inside an SVG. Its `x` and `y` mark the ground between the feet, and `time` is seconds since the current pose began. Choose `play`, `collapse`, `idle`, `offline`, `wake`, `hold`, or `wave`; pass `batteryLevel` as a percentage and `charging` for charging cells. The `hold` pose accepts one or two short lines in `signText`. `BatteryCells` is also available as a standalone SVG health bar.

## Editing the film

Each module in `timeline/beats/` owns its Beat's captions, sound cues, and review-frame times. All times are absolute seconds. The six Beats cover 0–8, 8–16, 16–30, 30–42, 42–52, and 52–60 seconds. Keep each caption visible for at least 2.5 seconds, with no overlaps, and reserve the bottom caption band when drawing scenes. Beat picture components run inside Remotion Sequences, so `useCurrentFrame()` starts at zero for each Beat. `useBeatTime()` converts that to absolute time. Use `cueTime(beat, id)` from `timeline/index.ts` for actions that accompany sounds.

Optional Beat `moments` map action names to absolute seconds or arrays of typing timestamps. Define a sound and its visual action from the same moment value. The timeline test checks every moment is inside its Beat and every timestamp array is ordered.

`timeline/readings.ts` supplies `EXAMPLE_READINGS.wireless` and `EXAMPLE_READINGS.charging`. Reuse their `output`, `level`, and `charging` fields for terminals and Clicky. Include displayed examples in the owning Beat's `readings` array so the exported-timeline test compares them with both READMEs.

Shared SVG components live in `video/components/`. `HandDrawnPanel` positions children in panel-local coordinates. `HandDrawnTerminal` accepts a `lines` array of strings or `{text, color}` objects and clips crisp monospace text within its pencil frame. Scenes control typing and output by passing the currently visible strings. `RedPenCircle`, `RedPenStrike`, `RedPenTick`, and `RedPenArrow` accept canvas geometry and a `progress` reveal value from 0 to 1. Use fixed seeds to keep lines stable across parallel renders.

Add a cue synthesizer at `audio/gbat_audio/cues/<kind>.py` with `synthesize(sample_rate) -> numpy.ndarray`, then add its cue to the owning Beat. Use an identifier such as `reading_ding` for `kind`. The mixer imports that module, places its mono samples at the cue's absolute `time`, and applies its optional `gain`. Put story timing in the Beat, not in the synthesizer. Keep the score's story windows in sync with the timeline when changing moments or caption IDs used by `audio/gbat_audio/score.py`.

To synthesize audio without rendering video:

```sh
npm run timeline:export
uv run --project audio python -m gbat_audio --timeline build/timeline.json --output build/audio.wav
```

The `--layer score` and `--layer cues` options isolate the score or effects for listening and tests; use a separate `--output` path for these previews. The default `mix` writes a 48 kHz mono PCM16 WAV. No downloaded pictures or audio samples are used.

## Font and reference source

LXGW WenKai is bundled as a WOFF2 web font with ASCII, common CJK punctuation, and the GB2312 Chinese character set. Tests read the actual font cmap and fail when a title or caption uses a missing glyph. Check other handwritten labels in the review frames too. To add characters outside that set, extend the text selected in `scripts/subset_font.py`, then regenerate the subset from the upstream regular TTF:

```sh
uv run --project audio python scripts/subset_font.py /path/to/LXGWWenKai-Regular.ttf
npm run test:timeline
```

Commit the generated `.woff2` and `.charset.txt` together. Preserve [OFL.txt](video/public/fonts/OFL.txt), which contains the font's SIL Open Font License and additional web-font subsetting permission. The font comes from [LXGW WenKai](https://github.com/lxgw/LxgwWenKai); the bundled subset was copied from the local Pelican Test Film at commit `965b4fe293eb891531c6258a13cf2c9ce76cacba`. The font is only used for web rendering, not distributed as an installable desktop font.

`RoughDrawing`, `Paper`, font loading, animation helpers, the palette, and the subset generator were copied and adapted from that same reference project. There are no imports or runtime dependencies on the reference directory. Paper grain uses fixed SVG seeds; only pencil strokes animate. Remotion has [its own license](https://www.remotion.dev/license).
