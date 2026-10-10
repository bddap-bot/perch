# perch

A tiny bird lives on whatever your webcam sees, and little monsters want to catch it. Anything that was not there when the empty scene was captured becomes solid: the bird perches and hops on it, and the monsters cannot pass through it.

Play: **https://bddap-bot.github.io/perch/**

## How to play

1. Allow the camera.
2. Step out of view during the countdown so the game can learn the empty scene.
3. Come back and protect the bird. It moves on its own and likes to stay near your eyes when it feels safe; you play with your body and whatever you hold.

- Monsters creep in from the edges of the screen and fly through open air toward the bird. Swat one with a moving hand or object to knock it out.
- Wall the bird in and the monsters cannot reach it, but it needs air: when no open path leads from the bird to the edge of the screen, its air pocket turns red and its bubbles run out. Open a gap before they do.
- The bird is caught or runs out of air: the round ends, your time is shown, and a new round starts.
- Click or tap to recapture the empty scene.

Highlighted areas count as solid, and their top edges are drawn brighter. Keep the camera still: moving it makes the whole room look new, so recapture after bumping it. Soft shadows and whole-room brightness or colour changes are ignored; a lamp switched on near one part of the scene needs a recapture.

## Build

`shell.nix` provides the toolchain, including the exact `wasm-bindgen` version the crate pins.

```sh
nix-shell --run ./web/build.sh
python3 -m http.server -d web/dist
```

Camera access needs `localhost` or HTTPS.

MIT licensed.
