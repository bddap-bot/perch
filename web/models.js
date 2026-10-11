const VISION = "https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@1.1.0";
const ORT = "https://cdn.jsdelivr.net/npm/onnxruntime-web@1.30.0/dist";
const FACE_MODEL =
  "https://storage.googleapis.com/mediapipe-models/face_detector/blaze_face_short_range/float16/1/blaze_face_short_range.tflite";
const PERSON_MODEL =
  "https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmenter/float16/1/selfie_segmenter.tflite";
const DEPTH_MODEL =
  "https://huggingface.co/onnx-community/depth-anything-v2-small/resolve/4472b7362082ad9968fee890ca0f1e5aca36b93d/onnx/model_quantized.onnx";
const FACE_PERIOD_MS = 100;
const FACE_STALE_MS = 300;
const DEPTH_PERIOD_MS = 200;
const DEPTH_BUDGET_MS = 1000;
const DEPTH_TRIALS = 3;

const DETECTOR = `
const { FilesetResolver, FaceDetector } = await import("${VISION}/vision_bundle.mjs");
const fileset = await FilesetResolver.forVisionTasks("${VISION}/wasm", true);
const detector = await FaceDetector.createFromOptions(fileset, {
  baseOptions: { modelAssetPath: "${FACE_MODEL}", delegate: "CPU" },
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

const SEGMENTER = `
const { FilesetResolver, ImageSegmenter } = await import("${VISION}/vision_bundle.mjs");
const fileset = await FilesetResolver.forVisionTasks("${VISION}/wasm", true);
const segmenter = await ImageSegmenter.createFromOptions(fileset, {
  baseOptions: { modelAssetPath: "${PERSON_MODEL}", delegate: "CPU" },
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

const ESTIMATOR = `
const ort = await import("${ORT}/ort.webgpu.min.mjs");
ort.env.wasm.wasmPaths = "${ORT}/";
const session = await ort.InferenceSession.create("${DEPTH_MODEL}", { executionProviders: ["webgpu", "wasm"] });
const PATCH = 14;
const PIXELS = 238 * 182;
const MEAN = [0.485, 0.456, 0.406];
const STD = [0.229, 0.224, 0.225];
const scaled = new OffscreenCanvas(PATCH, PATCH).getContext("2d", { willReadFrequently: true });
async function estimate(image) {
  const { width, height } = image;
  const side = (n) => Math.max(PATCH, Math.round(n / PATCH) * PATCH);
  const W = side(Math.sqrt((PIXELS * width) / height));
  const H = side((W * height) / width);
  scaled.canvas.width = W;
  scaled.canvas.height = H;
  const bitmap = await createImageBitmap(image);
  scaled.drawImage(bitmap, 0, 0, W, H);
  bitmap.close();
  const px = scaled.getImageData(0, 0, W, H).data;
  const input = new Float32Array(3 * W * H);
  for (let i = 0; i < W * H; i++)
    for (let c = 0; c < 3; c++) input[c * W * H + i] = (px[i * 4 + c] / 255 - MEAN[c]) / STD[c];
  const result = await session.run({ pixel_values: new ort.Tensor("float32", input, [1, 3, H, W]) });
  const predicted = result.predicted_depth.data;
  const out = new Float32Array(width * height);
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++)
      out[y * width + x] = predicted[Math.floor((y * H) / height) * W + Math.floor((x * W) / width)];
  return out;
}
let background = null;
let measured = null;
onmessage = async ({ data: { frame, empty, generation } }) => {
  try {
    if (empty) {
      measured = null;
      background = await estimate(empty);
      measured = generation;
    }
    if (measured !== generation) return postMessage({ generation, missing: true });
    const started = performance.now();
    const depth = await estimate(frame);
    const reply = { generation, ms: performance.now() - started, frame: depth, background: background.slice(), pixels: frame.data };
    postMessage(reply, [reply.frame.buffer, reply.background.buffer, reply.pixels.buffer]);
  } catch {
    postMessage(null);
  }
};
postMessage(null);
`;

export function spawn(source, label) {
  const worker = new Worker(URL.createObjectURL(new Blob([source], { type: "text/javascript" })), { type: "module" });
  const task = { worker, ready: false, busy: false, sent: -Infinity, arrived: -Infinity, result: null };
  worker.onmessage = ({ data }) => {
    task.result = data;
    task.arrived = performance.now();
    task.ready = true;
    task.busy = false;
  };
  worker.onerror = (e) => {
    console.warn(`no ${label}:`, e.message ?? e.type);
    task.result = null;
    task.busy = false;
  };
  return task;
}

export function due(task, period = 0) {
  return task.ready && !task.busy && performance.now() - task.sent >= period;
}

function post(task, message, transfer) {
  task.busy = true;
  task.sent = performance.now();
  task.worker.postMessage(message, transfer);
}

export function postBitmap(task, source, extra = {}) {
  task.busy = true;
  task.sent = performance.now();
  const at = task.sent;
  createImageBitmap(source)
    .then((frame) => task.worker.postMessage({ ...extra, frame, at }, [frame]))
    .catch(() => (task.busy = false));
}

let face = null;

export function eyes(video) {
  face ??= spawn(DETECTOR, "face detector");
  if (due(face, FACE_PERIOD_MS) && video.readyState >= 2) postBitmap(face, video);
  return (performance.now() - face.arrived < FACE_STALE_MS && face.result) || [];
}

let person = null;

export function personMask(canvas) {
  person ??= spawn(SEGMENTER, "person segmenter");
  if (due(person)) postBitmap(person, canvas);
  return person.result ?? new Uint8Array();
}

let depth = null;
let generation = 0;
let scene = null;
let resend = false;
const timings = [];

export function depthEstimate(frame, width, height, empty) {
  if (depth === false) return null;
  depth ??= spawn(ESTIMATOR, "depth estimator");
  if (empty.length) {
    generation++;
    depth.result = null;
    scene = new ImageData(new Uint8ClampedArray(empty), width, height);
    resend = true;
  }
  const reply = depth.result?.generation === generation ? depth.result : null;
  depth.result = null;
  if (reply?.missing) resend = true;
  if (reply?.ms && timings.push(reply.ms) === DEPTH_TRIALS) {
    const fastest = Math.min(...timings);
    if (fastest > DEPTH_BUDGET_MS) {
      console.info(`depth off: ${Math.round(fastest)} ms per estimate`);
      depth.worker.terminate();
      depth = false;
      return null;
    }
  }
  if (due(depth, resend ? 0 : DEPTH_PERIOD_MS)) {
    const image = new ImageData(new Uint8ClampedArray(frame), width, height);
    post(depth, { frame: image, empty: resend ? scene : null, generation }, [image.data.buffer]);
    resend = false;
  }
  return reply?.missing ? null : reply;
}
