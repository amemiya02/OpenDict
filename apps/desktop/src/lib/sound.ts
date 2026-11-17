// The app's one audio player: Dictionary audio plays here, in the app, never inside an
// Entry frame. A new sound stops the one before it.

let current: HTMLAudioElement | null = null;

/** Plays the sound at `url` (an Entry scheme URL; the app's CSP allows media from nowhere else). */
export function playSound(url: string): void {
  current?.pause();
  const audio = new Audio(url);
  current = audio;
  // A missing or undecodable file just stays silent.
  audio.play().catch(() => {});
}

/**
 * Whether the reader has just clicked or pressed a key (transient user activation). A
 * click inside an Entry frame counts for the app too. Where the browser cannot tell
 * (no `navigator.userActivation`), this says yes.
 */
export function readerJustActed(): boolean {
  const activation = (navigator as { userActivation?: { isActive: boolean } }).userActivation;
  return activation ? activation.isActive : true;
}
