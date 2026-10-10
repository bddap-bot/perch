# Agent notes

## Rules

- No code comments. A survivor states a why the code cannot show.
- Changes land directly on `main`.
- `test-map.json` maps every touched path to the commands that must be green before landing.
- Sprites are drawn in code (`src/sprite.rs`). Any added asset is self-made or CC0 with its attribution recorded here, and stored lossless.

## Layout

- `src/seg.rs`: background capture (per-pixel median of a few frames plus a per-pixel chroma tolerance from their spread), then per-frame classification: a pixel is solid on a chroma shift beyond its tolerance or a luma ratio outside the shadow/highlight band, after dividing out a global gain (the median luma ratio). Pixels judged background slowly update the model. Open then close with a 3x3 box. Pure Rust, no Bevy.
- `src/body.rs`: the character against the mask, in mask cells. Moves at most one cell per sub-step on each axis, so it cannot tunnel. A solid that overlaps it lifts it to the nearest free row above. Small steps are climbed. Falling off the bottom respawns it at the top.
- `src/sprite.rs`: pixel-art frames.
- `src/game.rs`, `src/webcam.rs`: the wasm32-only Bevy front end. `getUserMedia` feeds the page's `<video>`, which CSS shows mirrored behind a transparent Bevy canvas; the game draws only the mask tint and the character. An offscreen 2D canvas reads the video mirrored and centre-cropped to 4:3 at mask resolution (240x180), and segmentation runs only when the video has a new frame. The `#stage` box in `web/index.html` and the camera's `ScalingMode::AutoMin` must frame the same 4:3 rectangle.

Native `cargo test` covers segmentation (busy background, lighting shift, cast shadow, moving and still objects), collision and sprites. Bevy is a wasm32-only dependency, so native builds stay small.

## Landing contract

A landing on `main` is complete only when the `pages` workflow (native tests, then the wasm build and deploy) is green on that sha. The deployed page is whatever `main` last built: https://bddap-bot.github.io/perch/

## End-to-end check without a webcam

Chromium can play a synthetic `.y4m` as the camera: `--use-fake-ui-for-media-stream --use-fake-device-for-media-stream --use-file-for-fake-video-capture=<file>.y4m`. Make the clip open on an empty scene that lasts past the countdown, then bring objects in. Headless WebGL2 through SwiftShader (`--use-angle=swiftshader-webgl --enable-unsafe-swiftshader`) renders it.
