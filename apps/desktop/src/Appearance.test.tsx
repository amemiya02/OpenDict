import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { EntryFrame } from "./components/EntryFrame";
import { I18nProvider } from "./i18n";
import type { Appearance, Entry, Library } from "./lib/ipc";

const library: Library = {
  folders: [{ path: "D:\\Dictionaries", dictionaryCount: 2, problem: null }],
  dictionaries: [
    { id: "learner", title: "Sample Learner's Dictionary", path: "D:\\Dictionaries\\learner.mdx", entryCount: 100, size: 1024, enabled: true, index: { state: "ready" } },
    { id: "atlas", title: "Sample Atlas", path: "D:\\Dictionaries\\atlas.mdx", entryCount: 100, size: 1024, enabled: true, index: { state: "ready" } },
  ],
  failures: [],
  indexing: null,
  problem: null,
};

function entry(id: string, title: string, asAuthored = false): Entry {
  return { dictionaryId: id, dictionaryTitle: title, src: `http://odentry.localhost/${id}/?headword=thumb`, allowNetwork: false, sound: null, pronunciation: null, asAuthored };
}

const defaults: Appearance = { theme: "system", allAsAuthored: false, textSize: 16, zoom: 100 };

/** A fake shell that keeps the reading preferences like the real one and records every change to them. */
function fakeShell(stored: Partial<Appearance> = {}, authored: string[] = []) {
  const appearance: Appearance = { ...defaults, ...stored };
  /** Choices for single Dictionaries; the rest follow the global switch. */
  const chosen = new Map(authored.map((id) => [id, true]));
  const asAuthored = { has: (id: string) => chosen.get(id) ?? appearance.allAsAuthored };
  const changes: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    switch (command) {
      case "library":
        return library;
      case "suggest":
        return { items: [{ headword: "thumb", count: 2 }], lemma: null, near: false, miss: false, browse: false };
      case "headwords_after":
        return [];
      case "look_up":
        return [entry("learner", "Sample Learner's Dictionary", asAuthored.has("learner")), entry("atlas", "Sample Atlas", asAuthored.has("atlas"))];
      case "appearance":
        return appearance;
      case "history":
      case "record_lookup":
        return [];
    }
    if (command.startsWith("plugin:")) return null; // the updater, which has nothing to offer
    changes.push({ command, args });
    switch (command) {
      case "set_theme":
        appearance.theme = args.theme as Appearance["theme"];
        return null;
      case "set_as_authored":
        chosen.set(args.dictionaryId as string, args.asAuthored as boolean);
        return null;
      case "set_all_as_authored":
        appearance.allAsAuthored = args.asAuthored as boolean;
        chosen.clear();
        return null;
      case "set_text_size":
        appearance.textSize = Math.min(Math.max(args.size as number, 10), 32);
        return appearance.textSize;
      case "set_zoom":
        appearance.zoom = Math.min(Math.max(args.percent as number, 50), 200);
        return appearance.zoom;
    }
    throw new Error(`unexpected command ${command}`);
  });
  return changes;
}

/** jsdom has no media queries: the operating system's colour scheme, as the page would see it. */
function systemScheme(scheme: "light" | "dark") {
  const listeners = new Set<() => void>();
  let dark = scheme === "dark";
  window.matchMedia = vi.fn((query: string) => ({
    get matches() {
      return query.includes("dark") ? dark : !dark;
    },
    media: query,
    addEventListener: (_: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_: string, listener: () => void) => listeners.delete(listener),
  })) as unknown as typeof window.matchMedia;
  return (next: "light" | "dark") => {
    dark = next === "dark";
    listeners.forEach((listener) => listener());
  };
}

afterEach(() => {
  delete document.documentElement.dataset.theme;
});

function renderApp() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
}

async function lookUp(word: string) {
  renderApp();
  await userEvent.type(screen.getByRole("combobox", { name: "Type a word" }), `${word}{Enter}`);
  await screen.findByRole("heading", { level: 1, name: `【${word}】` });
}

async function openReadingSettings() {
  await userEvent.click(screen.getByRole("button", { name: "Library" }));
  return screen.findByRole("region", { name: "Reading" });
}

function frameOf(title: string) {
  return screen.getByTitle(`Entry from ${title}`) as HTMLIFrameElement;
}

/** Recoloured Entries are inverted as pictures by the app; the Entry document itself is never touched. */
function recoloured(title: string) {
  return frameOf(title).style.filter !== "";
}

describe("the theme", () => {
  it("follows the operating system until the reader picks one", async () => {
    systemScheme("light");
    fakeShell();
    renderApp();
    await vi.waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));

    const settings = await openReadingSettings();
    expect((within(settings).getByRole("radio", { name: "Follow system" }) as HTMLInputElement).checked).toBe(true);
  });

  it("turns dark when the operating system does, while it follows the system", async () => {
    const setSystem = systemScheme("light");
    fakeShell();
    renderApp();
    await vi.waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));

    setSystem("dark");

    await vi.waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
  });

  it("opens in the theme the reader chose last time", async () => {
    systemScheme("light");
    fakeShell({ theme: "dark" });
    renderApp();

    await vi.waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
  });

  it("switches when the reader picks another, and the choice is saved", async () => {
    systemScheme("light");
    const changes = fakeShell();
    renderApp();
    const settings = await openReadingSettings();

    await userEvent.click(within(settings).getByRole("radio", { name: "Dark" }));

    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(changes).toEqual([{ command: "set_theme", args: { theme: "dark" } }]);
  });
});

describe("Entries in the dark theme", () => {
  it("are recoloured by default, each with an \"as authored\" switch", async () => {
    systemScheme("dark");
    fakeShell();
    await lookUp("thumb");

    expect(recoloured("Sample Learner's Dictionary")).toBe(true);
    expect(recoloured("Sample Atlas")).toBe(true);
    const section = screen.getByRole("region", { name: "Sample Atlas" });
    expect(within(section).getByRole("button", { name: "Show as authored" }).getAttribute("aria-pressed")).toBe("false");
  });

  it("show one Dictionary as authored on its white page, and remember it", async () => {
    systemScheme("dark");
    const changes = fakeShell();
    await lookUp("thumb");
    const section = screen.getByRole("region", { name: "Sample Atlas" });

    await userEvent.click(within(section).getByRole("button", { name: "Show as authored" }));

    expect(recoloured("Sample Atlas")).toBe(false);
    expect(recoloured("Sample Learner's Dictionary")).toBe(true);
    expect(within(section).getByRole("button", { name: "Show as authored" }).getAttribute("aria-pressed")).toBe("true");
    expect(changes).toEqual([{ command: "set_as_authored", args: { dictionaryId: "atlas", asAuthored: true } }]);
  });

  it("show a Dictionary as authored where the reader left it so", async () => {
    systemScheme("dark");
    fakeShell({}, ["atlas"]);
    await lookUp("thumb");

    expect(recoloured("Sample Atlas")).toBe(false);
    expect(recoloured("Sample Learner's Dictionary")).toBe(true);
  });

  it("can all be shown as authored at once", async () => {
    systemScheme("dark");
    const changes = fakeShell();
    await lookUp("thumb");
    const settings = await openReadingSettings();

    await userEvent.click(within(settings).getByRole("checkbox", { name: "Show every dictionary as authored in the dark theme" }));
    expect(changes).toEqual([{ command: "set_all_as_authored", args: { asAuthored: true } }]);
    await userEvent.click(screen.getByRole("button", { name: "Suggestions" }));
    await userEvent.click(await screen.findByRole("option", { name: /thumb/ }));
    await screen.findByRole("heading", { level: 1, name: "【thumb】" });

    expect(recoloured("Sample Atlas")).toBe(false);
    expect(recoloured("Sample Learner's Dictionary")).toBe(false);
  });
});

describe("Entries in the light theme", () => {
  it("are shown as authored, with no switch", async () => {
    systemScheme("light");
    fakeShell();
    await lookUp("thumb");

    expect(recoloured("Sample Atlas")).toBe(false);
    expect(screen.queryByRole("button", { name: "Show as authored" })).toBeNull();
  });
});

describe("text size and page zoom", () => {
  it("grow and shrink from the Reading settings, and are saved", async () => {
    systemScheme("light");
    const changes = fakeShell();
    renderApp();
    const settings = await openReadingSettings();

    await userEvent.click(within(settings).getByRole("button", { name: "Larger text" }));
    await userEvent.click(within(settings).getByRole("button", { name: "Larger text" }));
    await userEvent.click(within(settings).getByRole("button", { name: "Zoom in" }));
    await userEvent.click(within(settings).getByRole("button", { name: "Zoom out" }));
    await userEvent.click(within(settings).getByRole("button", { name: "Zoom out" }));

    expect(changes).toEqual([
      { command: "set_text_size", args: { size: 17 } },
      { command: "set_text_size", args: { size: 18 } },
      { command: "set_zoom", args: { percent: 110 } },
      { command: "set_zoom", args: { percent: 100 } },
      { command: "set_zoom", args: { percent: 90 } },
    ]);
    expect(within(settings).getByText("18 px")).toBeTruthy();
    expect(within(settings).getByText("90%")).toBeTruthy();
  });

  it("zoom with Ctrl and plus, minus and zero", async () => {
    systemScheme("light");
    const changes = fakeShell({ zoom: 125 });
    renderApp();
    await within(await openReadingSettings()).findByText("125%");

    await userEvent.keyboard("{Control>}={/Control}");
    await userEvent.keyboard("{Control>}-{/Control}");
    await userEvent.keyboard("{Control>}-{/Control}");
    await userEvent.keyboard("{Control>}0{/Control}");

    expect(changes.map((change) => change.args.percent)).toEqual([150, 125, 110, 100]);
  });

  it("give an open Entry a new text size without reloading it", () => {
    const { rerender } = render(<EntryFrame dictionaryTitle="Sample Atlas" src="about:blank" textSize={16} />);
    const frame = frameOf("Sample Atlas");
    const post = vi.spyOn(frame.contentWindow!, "postMessage");

    rerender(<EntryFrame dictionaryTitle="Sample Atlas" src="about:blank" textSize={18} />);

    expect(post).toHaveBeenCalledWith({ opendict: "textSize", size: 18 }, "*");
    expect(frameOf("Sample Atlas")).toBe(frame);
  });
});
