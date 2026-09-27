# gbat explainer film

This directory contains the separate Node and Python toolchain for a 60-second film in English and Chinese. The Rust binary and its tests do not depend on it.

## Build

Use Node 26.10.0, npm, uv, and ffmpeg with ffprobe. On macOS, install the latter tools with `brew install uv ffmpeg`. The Node version is recorded in `.node-version`; use your Node version manager to select it.

```sh
cd demo
npm ci
npm run build
```

The build exports `build/timeline.json`, synthesises `build/audio.wav`, renders both Remotion compositions, muxes H.264 video with AAC audio, and extracts PNG review frames. Both movies are 1920 x 1080 at 30 fps. Remotion downloads its headless browser on first use. Python dependencies are isolated in `audio/.venv` and locked by `audio/uv.lock`.

- `build/gbat-en.mp4`
- `build/gbat-zh-CN.mp4`
- `build/review/`, named by language, Beat, and frame number

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

Timeline tests inspect exported JSON, caption durations and ordering, vocabulary, cue synthesis modules, font glyph coverage, and caption widths measured from the bundled font. Audio tests invoke the public Python CLI with JSON in and WAV out, checking length and each isolated cue onset. Film tests inspect both movies with ffprobe, check PNG files, and reject stale builds. The final-second audio check is opt-in with `CHECK_FINAL_SILENCE=1 npm run test:film` until the complete score is added in #26.

Use `npm run studio` for frame-by-frame review. The `GbatEnglish` and `GbatChinese` compositions share pictures and audio timing. Review the PNGs in both languages after changing a Beat; automated checks do not judge illustrations or text placement inside pictures.

`ClickyCharacterSheet` previews all seven animated poses over six seconds. The build exports its one-second frame to `build/review/clicky-character-sheet.png`. To render only this sheet:

```sh
npx remotion still video/index.ts ClickyCharacterSheet build/review/clicky-character-sheet.png --frame=30 --public-dir=video/public
```

Scenes mount `Clicky` from `video/characters/Clicky` inside an SVG. Its `x` and `y` mark the ground between the feet, and `time` is seconds since the current pose began. Choose `play`, `collapse`, `idle`, `offline`, `wake`, `hold`, or `wave`; pass `batteryLevel` as a percentage and `charging` for charging cells. The `hold` pose accepts one or two short lines in `signText`. `BatteryCells` is also available as a standalone SVG health bar.

## Editing the film

Each module in `timeline/beats/` owns its Beat's captions, sound cues, and review-frame times. All times are absolute seconds. The six Beats cover 0–8, 8–16, 16–30, 30–42, 42–52, and 52–60 seconds. Beat picture components run inside Remotion Sequences, so `useCurrentFrame()` starts at zero for each Beat. `useBeatTime()` converts that to absolute time. Use `cueTime(beat, id)` for actions that accompany sounds.

Add a cue synthesizer at `audio/gbat_audio/cues/<kind>.py` with `synthesize(sample_rate) -> numpy.ndarray`, then add its cue to the owning Beat. The public audio command is:

```sh
uv run --project audio python -m gbat_audio --timeline build/timeline.json --output build/audio.wav
```

The `--layer score` and `--layer cues` options isolate the score or effects for listening and tests. No downloaded pictures or audio samples are used.

## Font and reference source

LXGW WenKai is bundled as a WOFF2 web font with ASCII, common CJK punctuation, and the GB2312 Chinese character set. Tests read the actual font cmap and fail when a caption uses a missing glyph. To regenerate the subset from the upstream regular TTF:

```sh
uv run --project audio python scripts/subset_font.py /path/to/LXGWWenKai-Regular.ttf
npm run test:timeline
```

The font comes from [LXGW WenKai](https://github.com/lxgw/LxgwWenKai) under the SIL Open Font License. Its license and web-font subsetting permission are included in `video/public/fonts/OFL.txt`. The bundled subset was copied from the local Pelican Test Film at commit `965b4fe293eb891531c6258a13cf2c9ce76cacba`. The font is only used for web rendering, not distributed as an installable desktop font.

`RoughDrawing`, `Paper`, font loading, animation helpers, the palette, and the subset generator were copied and adapted from that same reference project. There are no imports or runtime dependencies on the reference directory. Paper grain uses fixed SVG seeds; only pencil strokes animate. Remotion has [its own license](https://www.remotion.dev/license).
