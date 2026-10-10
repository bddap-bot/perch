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
| Jump | Space (or ↑ W Z) | A |
| Recapture the empty scene | B | B or Select |

Anything highlighted counts as ground, and its top edges are drawn brighter. A plain wall behind you helps but is not required. Keep the camera still: moving it makes the whole room look new, so recapture after bumping it. Lighting changes and soft shadows are tolerated; a sudden lamp switch-on may need a recapture.

## Build

```sh
nix-shell --run ./web/build.sh
python3 -m http.server -d web/dist
```

Camera access needs `localhost` or HTTPS.

MIT licensed.
