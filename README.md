# perch

A tiny bird hops around on whatever your webcam sees. Put a book on the desk, hold out an arm, lean into frame: anything that was not there when the empty scene was captured becomes ground.

Play: **https://bddap-bot.github.io/perch/**

## How to play

1. Allow the camera.
2. Step out of view during the countdown so the game can learn the empty scene.
3. Come back, place things, and move the bird across them.

| | Keyboard | Gamepad |
|---|---|---|
| Run | ← → (or A D) | left stick / d-pad |
| Jump | Space (or ↑ W Z) | bottom face button |
| Recapture the empty scene | B | right face button or Select |

Anything highlighted counts as ground, and its top edges are drawn brighter. Keep the camera still: moving it makes the whole room look new, so recapture after bumping it. Soft shadows and whole-room brightness changes are ignored; a lamp switched on near one part of the scene needs a recapture.

## Build

`shell.nix` provides the toolchain, including the exact `wasm-bindgen` version the crate pins.

```sh
nix-shell --run ./web/build.sh
python3 -m http.server -d web/dist
```

Camera access needs `localhost` or HTTPS.

MIT licensed.
