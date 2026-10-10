const WANTED = { width: 640, height: 480, facingMode: "user" };
const HELD = [
  ["exposureMode", "exposureTime"],
  ["whiteBalanceMode", "colorTemperature"],
  ["focusMode", "focusDistance"],
];

export async function openCamera(video) {
  video.srcObject = await navigator.mediaDevices.getUserMedia({ video: WANTED });
  await video.play();
}

export async function holdCamera(video, hold) {
  const track = video.srcObject?.getVideoTracks()[0];
  if (!track?.getCapabilities) return;
  const caps = track.getCapabilities();
  const now = track.getSettings();
  const mode = hold ? "manual" : "continuous";
  const advanced = HELD.filter(([m]) => caps[m]?.includes(mode)).map(([m, value]) =>
    hold && value in caps && value in now ? { [m]: mode, [value]: now[value] } : { [m]: mode },
  );
  if (advanced.length) {
    try {
      await track.applyConstraints({ ...WANTED, advanced });
    } catch (e) {
      console.warn("camera settings not applied", e);
    }
  }
  const settings = track.getSettings();
  console.info(`camera ${mode}:`, HELD.map(([m]) => `${m}=${settings[m] ?? "unsupported"}`).join(" "));
}
