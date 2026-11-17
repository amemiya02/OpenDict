/**
 * Loads the interface afresh, so every view reads the reader's data again: after the app
 * switches to the data in another folder. The shell keeps running.
 */
export function reloadInterface(): void {
  window.location.reload();
}
