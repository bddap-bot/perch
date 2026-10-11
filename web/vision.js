import { due, postBitmap, spawn } from "./models.js";

const WORKER = `
import init, { WorkerSegmenter } from "${new URL("segmenter.js", document.baseURI)}";
import { personMask, depthEstimate } from "${new URL("./models.js", import.meta.url)}";
await init();
let segmenter = null;
let ctx = null;
let epoch = null;
onmessage = ({ data: { frame, crop, width, height, ...scene } }) => {
  try {
    if (ctx?.canvas.width !== width || ctx?.canvas.height !== height) {
      segmenter?.free();
      segmenter = new WorkerSegmenter(width, height);
      ctx = new OffscreenCanvas(width, height).getContext("2d", { willReadFrequently: true });
      ctx.setTransform(-1, 0, 0, 1, width, 0);
      // Low smoothing aliases the downscale, which changes the mask.
      ctx.imageSmoothingQuality = "medium";
      epoch = null;
    }
    if (scene.epoch !== epoch) {
      epoch = scene.epoch;
      if (scene.capturing) segmenter.capture();
      else segmenter.forget();
    }
    ctx.drawImage(frame, ...crop, 0, 0, width, height);
    const rgba = ctx.getImageData(0, 0, width, height).data;
    const person = personMask(ctx.canvas);
    if (segmenter.has_background()) {
      const depth = depthEstimate(rgba, width, height, segmenter.depth_background() ?? new Uint8Array());
      if (depth) segmenter.observe_depth(depth.frame, depth.background, depth.pixels);
    }
    const captured = segmenter.frame(rgba, person.length === width * height ? person : undefined);
    const mask = segmenter.mask();
    postMessage({ mask, width, height, epoch, captured }, [mask.buffer]);
  } catch (e) {
    console.warn("vision frame dropped:", e);
    postMessage(null);
  } finally {
    frame.close();
  }
};
postMessage(null);
`;

const scene = { width: 0, height: 0, epoch: 0, capturing: false };
let worker = null;
let counting = null;
let shown = 0;
let taken = 0;

export function visionSize(width, height) {
  Object.assign(scene, { width, height, capturing: false });
  scene.epoch++;
}

export function visionReset() {
  scene.epoch++;
  scene.capturing = false;
}

export function visionCapture() {
  scene.epoch++;
  scene.capturing = true;
}

export function visionFeed(video, crop) {
  worker ??= spawn(WORKER, "vision");
  if (counting !== video) {
    counting = video;
    const count = () => {
      shown++;
      video.requestVideoFrameCallback(count);
    };
    video.requestVideoFrameCallback(count);
  }
  if (!due(worker) || taken === shown) return;
  taken = shown;
  postBitmap(worker, video, { ...scene, crop: Array.from(crop) });
}

export function visionResult() {
  const reply = worker?.result;
  if (worker) worker.result = null;
  const current = reply && ["width", "height", "epoch"].every((k) => reply[k] === scene[k]);
  return current ? reply : null;
}
