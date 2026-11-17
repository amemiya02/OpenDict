import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Channel } from "@tauri-apps/api/core";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { DownloadEvent } from "@tauri-apps/plugin-updater";
import { App } from "./App";
import { I18nProvider } from "./i18n";

const newRelease = {
  rid: 7,
  currentVersion: "0.1.0",
  version: "0.2.0",
  date: "2026-12-01T00:00:00Z",
  body: "Release notes",
  rawJson: {},
};

/** A fake shell whose update endpoint announces `release` (or nothing). */
function fakeShell(release: typeof newRelease | null) {
  const calls: string[] = [];
  mockIPC((command) => {
    calls.push(command);
    if (command === "library_status") return { kind: "ready", folder: "/library", dictionaryCount: 1 };
    if (command === "plugin:updater|check") return release;
    return null;
  });
  return calls;
}

function renderApp() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
}

describe("updates", () => {
  it("shows the available version in the status line when a new release is out", async () => {
    fakeShell(newRelease);
    renderApp();

    expect(await screen.findByText("OpenDict 0.2.0 is available.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeTruthy();
  });

  it("downloads, installs and restarts when the reader confirms", async () => {
    const calls = fakeShell(newRelease);
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Install and restart" }));

    await waitFor(() => expect(calls).toContain("plugin:process|restart"));
    expect(calls.indexOf("plugin:updater|download_and_install")).toBeGreaterThan(-1);
    expect(calls.indexOf("plugin:updater|download_and_install")).toBeLessThan(calls.indexOf("plugin:process|restart"));
  });

  it("shows how much of the update has downloaded", async () => {
    mockIPC((command, args) => {
      if (command === "plugin:updater|check") return newRelease;
      if (command === "plugin:updater|download_and_install") {
        const progress = (args as { onEvent: Channel<DownloadEvent> }).onEvent;
        progress.onmessage({ event: "Started", data: { contentLength: 200 } });
        progress.onmessage({ event: "Progress", data: { chunkLength: 50 } });
        return new Promise(() => {}); // still downloading
      }
      return null;
    });
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Install and restart" }));

    expect(await screen.findByText("Downloading OpenDict 0.2.0… 25%")).toBeTruthy();
  });

  it("says why an update failed and offers it again", async () => {
    mockIPC((command) => {
      if (command === "plugin:updater|check") return newRelease;
      if (command === "plugin:updater|download_and_install") throw "signature verification failed";
      if (command === "plugin:process|restart") throw new Error("must not restart after a failed install");
      return null;
    });
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Install and restart" }));

    expect(await screen.findByText("OpenDict 0.2.0 could not be installed: signature verification failed")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeTruthy();
  });

  it("stays silent when there is no newer release or the check fails", async () => {
    fakeShell(null);
    renderApp();
    await screen.findByText("1 dictionary");
    expect(screen.queryByRole("button", { name: "Install and restart" })).toBeNull();

    mockIPC((command) => {
      if (command === "plugin:updater|check") throw "network unreachable";
      return { kind: "ready", folder: "/library", dictionaryCount: 1 };
    });
    renderApp();
    await screen.findAllByText("1 dictionary");
    expect(screen.queryByText(/network unreachable/)).toBeNull();
  });
});
