const WANTED = { width: 640, height: 480, facingMode: "user" };
const LOCKABLE = ["exposureMode", "whiteBalanceMode", "focusMode"];
let saved = null;

export async function openCamera(video) {
  video.srcObject = await navigator.mediaDevices.getUserMedia({ video: WANTED });
  await video.play();
  addEventListener("pagehide", () => releaseCamera(video));
}

export async function lockCamera(video) {
  const track = video.srcObject?.getVideoTracks()[0];
  if (!track?.getCapabilities || saved) return;
  const caps = track.getCapabilities();
  const now = track.getSettings();
  const lockable = LOCKABLE.filter((m) => caps[m]?.includes("manual") && m in now);
  saved = lockable.map((m) => ({ [m]: now[m] }));
  await apply(track, lockable.map((m) => ({ [m]: "manual" })));
  const after = track.getSettings();
  console.info("camera locked:", LOCKABLE.map((m) => `${m}=${after[m] ?? "unsupported"}`).join(" "));
}

export async function releaseCamera(video) {
  const track = video.srcObject?.getVideoTracks()[0];
  if (!track || !saved) return;
  const modes = saved;
  saved = null;
  await apply(track, modes);
}

async function apply(track, advanced) {
  if (!advanced.length) return;
  try {
    // Chromium's getConstraints() forgets the basic constraints once image-capture ones apply.
    await track.applyConstraints({ ...WANTED, advanced });
  } catch (e) {
    console.warn("camera settings not applied", e);
  }
}
