import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import type { DataDirectory, Library } from "./lib/ipc";
import { reloadInterface } from "./lib/reload";

vi.mock("./lib/reload", () => ({ reloadInterface: vi.fn() }));

const library: Library = { folders: [], dictionaries: [], failures: [], indexing: null, groups: [], currentGroup: null, problem: null };
const HOME = "C:\\Users\\reader\\AppData\\Roaming\\io.github.amemiya02.opendict";
const SYNCED = "D:\\Sync\\OpenDict";

interface Shell {
  directory: DataDirectory;
  /** What the folder dialog answers; null is a cancelled dialog. */
  picked: string | null;
  /** How a move into `SYNCED` goes. */
  move: "moves" | "holdsData" | "fails";
}

/** A fake shell that keeps the data directory like the real one and records the commands about it. */
function fakeShell(shell: Shell) {
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    switch (command) {
      case "library":
        return library;
      case "suggest":
        return { items: [], lemma: null, near: false, miss: false, browse: true };
      case "history":
        return [];
      case "word_books":
        return { books: [], current: null };
      case "appearance":
        return { theme: "system", allAsAuthored: false, textSize: 16, zoom: 100 };
      case "window_chrome":
        return { platform: "windows", material: null };
      case "data_directory":
        return shell.directory;
      case "choose_data_folder":
        calls.push({ command, args });
        return shell.picked;
      case "move_data_directory":
        calls.push({ command, args });
        if (shell.move === "holdsData") throw { kind: "holdsData", detail: args.path };
        if (shell.move === "fails") throw { kind: "failed", detail: "the disk is full" };
        shell.directory = { path: args.path as string, isDefault: false, problem: null };
        return shell.directory;
      case "use_data_directory":
        calls.push({ command, args });
        shell.directory = { path: args.path as string, isDefault: false, problem: null };
        return shell.directory;
    }
    if (command.startsWith("plugin:")) return null; // the updater, which has nothing to offer
    throw new Error(`not answered here: ${command}`);
  });
  return calls;
}

async function openDataSettings() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  await userEvent.click(screen.getByRole("button", { name: "Manage" }));
  return screen.findByRole("region", { name: "Your data" });
}

beforeEach(() => {
  vi.mocked(reloadInterface).mockClear();
});

describe("the data directory", () => {
  it("shows where the reader's data is kept", async () => {
    fakeShell({ directory: { path: HOME, isDefault: true, problem: null }, picked: null, move: "moves" });

    const settings = await openDataSettings();

    expect(await within(settings).findByText(HOME)).toBeTruthy();
    expect(within(settings).getByText(/History, Word Books, settings and indexes/)).toBeTruthy();
  });

  it("moves into the folder the reader chooses, and says where the data is now", async () => {
    const calls = fakeShell({ directory: { path: HOME, isDefault: true, problem: null }, picked: SYNCED, move: "moves" });
    const settings = await openDataSettings();

    await userEvent.click(await within(settings).findByRole("button", { name: "Move…" }));

    expect(await within(settings).findByText(SYNCED)).toBeTruthy();
    expect(within(settings).queryByText(HOME)).toBeNull();
    expect(within(settings).getByRole("status").textContent).toBe(`Your data is now in ${SYNCED}.`);
    expect(calls.map((call) => call.command)).toEqual(["choose_data_folder", "move_data_directory"]);
    expect(calls[1]?.args).toEqual({ path: SYNCED });
    expect(reloadInterface).not.toHaveBeenCalled();
  });

  it("moves nothing when the reader closes the folder dialog", async () => {
    const calls = fakeShell({ directory: { path: HOME, isDefault: true, problem: null }, picked: null, move: "moves" });
    const settings = await openDataSettings();

    await userEvent.click(await within(settings).findByRole("button", { name: "Move…" }));

    expect(calls.map((call) => call.command)).toEqual(["choose_data_folder"]);
    expect(within(settings).getByText(HOME)).toBeTruthy();
  });

  it("says the data stayed where it was when a move fails", async () => {
    fakeShell({ directory: { path: HOME, isDefault: true, problem: null }, picked: SYNCED, move: "fails" });
    const settings = await openDataSettings();

    await userEvent.click(await within(settings).findByRole("button", { name: "Move…" }));

    expect((await within(settings).findByRole("alert")).textContent).toBe(
      `Your data could not be moved and is still in ${HOME}: the disk is full`,
    );
    expect(within(settings).getByText(HOME)).toBeTruthy();
  });

  it("offers the data already in a folder, as on a second computer, and uses it only when asked", async () => {
    const calls = fakeShell({ directory: { path: HOME, isDefault: true, problem: null }, picked: SYNCED, move: "holdsData" });
    const settings = await openDataSettings();

    await userEvent.click(await within(settings).findByRole("button", { name: "Move…" }));
    expect(await within(settings).findByText(`${SYNCED} already holds OpenDict data. Use that data instead? Your data here stays where it is.`)).toBeTruthy();
    await userEvent.click(within(settings).getByRole("button", { name: "Cancel" }));
    expect(within(settings).queryByRole("button", { name: "Use that data" })).toBeNull();
    expect(calls.map((call) => call.command)).not.toContain("use_data_directory");

    await userEvent.click(within(settings).getByRole("button", { name: "Move…" }));
    await userEvent.click(await within(settings).findByRole("button", { name: "Use that data" }));

    await vi.waitFor(() => expect(reloadInterface).toHaveBeenCalledOnce());
    expect(calls.at(-1)).toEqual({ command: "use_data_directory", args: { path: SYNCED } });
  });

  it("says when the data directory cannot be found, and lets the reader choose another", async () => {
    const calls = fakeShell({
      directory: { path: "E:\\OpenDict", isDefault: false, problem: "the data directory E:\\OpenDict cannot be found" },
      picked: SYNCED,
      move: "moves",
    });
    const settings = await openDataSettings();

    expect(await within(settings).findByText("Your data directory cannot be found. It may be on a drive that is not connected.")).toBeTruthy();
    expect(within(settings).getByText("E:\\OpenDict")).toBeTruthy();
    expect(within(settings).queryByRole("button", { name: "Move…" })).toBeNull();
    await userEvent.click(within(settings).getByRole("button", { name: "Choose a folder…" }));

    await vi.waitFor(() => expect(reloadInterface).toHaveBeenCalledOnce());
    expect(calls.map((call) => call.command)).toEqual(["choose_data_folder", "use_data_directory"]);
    expect(calls[1]?.args).toEqual({ path: SYNCED });
  });
});
