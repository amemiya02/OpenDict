import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";

// jsdom has <dialog> but not its methods: enough of them for the command palette. Esc
// cancels and closes a modal dialog in browsers; here a keydown on it does.
if (typeof HTMLDialogElement !== "undefined" && !HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function (this: HTMLDialogElement) {
    this.setAttribute("open", "");
    const restore = document.activeElement as HTMLElement | null;
    this.querySelector<HTMLElement>("[autofocus], input")?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented && this.dispatchEvent(new Event("cancel", { cancelable: true }))) this.close();
    };
    this.addEventListener("keydown", onKey);
    this.addEventListener(
      "close",
      () => {
        this.removeEventListener("keydown", onKey);
        restore?.focus();
      },
      { once: true },
    );
  };
  HTMLDialogElement.prototype.close = function (this: HTMLDialogElement) {
    if (!this.hasAttribute("open")) return;
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
}

afterEach(async () => {
  cleanup();
  // Let listeners removed on unmount finish unregistering before the mocks go away.
  await new Promise((resolve) => setTimeout(resolve, 0));
  clearMocks();
});
