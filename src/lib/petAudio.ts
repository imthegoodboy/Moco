import pinkyMoo from "../assets/pinky-moo.wav";

export const PINKY_MOO_DURATION_MS = 2_864;

let activeMoo: HTMLAudioElement | undefined;

export function playCowMoo() {
  try {
    activeMoo?.pause();
    const audio = new Audio(pinkyMoo);
    activeMoo = audio;
    audio.volume = 0.78;
    audio.preload = "auto";

    const release = () => {
      if (activeMoo === audio) activeMoo = undefined;
    };
    audio.addEventListener("ended", release, { once: true });
    audio.addEventListener("error", release, { once: true });
    void audio.play().catch(release);
  } catch {
    // Pinky still animates on devices that cannot play audio.
  }
}
