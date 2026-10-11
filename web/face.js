export const VISION = "https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@1.1.0";
const MODEL =
  "https://storage.googleapis.com/mediapipe-models/face_detector/blaze_face_short_range/float16/1/blaze_face_short_range.tflite";
const PERIOD_MS = 100;
const STALE_MS = 300;

const DETECTOR = `
const { FilesetResolver, FaceDetector } = await import("${VISION}/vision_bundle.mjs");
const fileset = await FilesetResolver.forVisionTasks("${VISION}/wasm", true);
const detector = await FaceDetector.createFromOptions(fileset, {
  baseOptions: { modelAssetPath: "${MODEL}", delegate: "CPU" },
  runningMode: "VIDEO",
});
onmessage = ({ data: { frame, at } }) => {
  try {
    const [a, b] = detector.detectForVideo(frame, at).detections[0]?.keypoints ?? [];
    postMessage(a && b ? [((a.x + b.x) / 2) * frame.width, ((a.y + b.y) / 2) * frame.height] : []);
  } catch {
    postMessage([]);
  } finally {
    frame.close();
  }
};
postMessage([]);
`;

let worker = null;
let ready = false;
let busy = false;
let last = -Infinity;
let found = [];
let arrived = -Infinity;

function start() {
  worker = new Worker(URL.createObjectURL(new Blob([DETECTOR], { type: "text/javascript" })), { type: "module" });
  worker.onmessage = ({ data }) => {
    found = data;
    arrived = performance.now();
    ready = true;
    busy = false;
  };
  worker.onerror = (e) => {
    console.warn("no face detector:", e.message ?? e.type);
    busy = false;
  };
}

export function eyes(video) {
  if (!worker) start();
  const now = performance.now();
  if (ready && !busy && video.readyState >= 2 && now - last >= PERIOD_MS) {
    busy = true;
    last = now;
    createImageBitmap(video)
      .then((frame) => worker.postMessage({ frame, at: now }, [frame]))
      .catch(() => (busy = false));
  }
  return now - arrived < STALE_MS ? found : [];
}
