import { VISION } from "./face.js";

const MODEL =
  "https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmenter/float16/1/selfie_segmenter.tflite";

const SEGMENTER = `
const { FilesetResolver, ImageSegmenter } = await import("${VISION}/vision_bundle.mjs");
const fileset = await FilesetResolver.forVisionTasks("${VISION}/wasm", true);
const segmenter = await ImageSegmenter.createFromOptions(fileset, {
  baseOptions: { modelAssetPath: "${MODEL}", delegate: "CPU" },
  runningMode: "VIDEO",
  outputConfidenceMasks: true,
});
onmessage = ({ data: { frame, at } }) => {
  try {
    segmenter.segmentForVideo(frame, at, (result) => {
      const confidence = result.confidenceMasks[0].getAsFloat32Array();
      const mask = new Uint8Array(confidence.length);
      for (let i = 0; i < mask.length; i++) mask[i] = confidence[i] > 0.5;
      postMessage(mask, [mask.buffer]);
    });
  } catch {
    postMessage(null);
  } finally {
    frame.close();
  }
};
postMessage(null);
`;

const worker = new Worker(URL.createObjectURL(new Blob([SEGMENTER], { type: "text/javascript" })), { type: "module" });
let ready = false;
let busy = false;
let found = null;

worker.onmessage = ({ data }) => {
  found = data;
  ready = true;
  busy = false;
};
worker.onerror = (e) => {
  console.warn("no person segmenter:", e.message ?? e.type);
  found = null;
  busy = false;
};

export function person(canvas) {
  if (ready && !busy) {
    busy = true;
    const at = performance.now();
    createImageBitmap(canvas)
      .then((frame) => worker.postMessage({ frame, at }, [frame]))
      .catch(() => (busy = false));
  }
  return found ?? new Uint8Array();
}
