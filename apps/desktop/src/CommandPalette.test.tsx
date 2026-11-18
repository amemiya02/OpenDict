import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider, type Locale } from "./i18n";
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
  groups: [{ id: "g1", name: "Reading", dictionaries: ["learner"] }],
  currentGroup: null,
};

function entry(id: string, title: string, headword: string, sound: string | null = null): Entry {
  return { dictionaryId: id, dictionaryTitle: title, src: `http://odentry.localhost/${id}/?headword=${headword}`, allowNetwork: false, sound, pronunciation: null, asAuthored: false };
}

const words = ["apple", "cat", "thumb"];

interface Shell {
  /** Every command that changes something, in order. */
  changes: { command: string; args: Record<string, unknown> }[];
  lookedUp: string[];
  /** While set, Lookups wait for this before answering. */
  hold: Promise<void> | null;
  /** While set, suggestions wait for this before answering. */
  holdSuggestions: Promise<void> | null;
  recorded: string[];
}

/** A fake shell with a two-Dictionary Library, one Group, and three headwords. */
function fakeShell(stored: Partial<Appearance> & { language?: Locale | null } = {}): Shell {
  const appearance = { theme: "system", allAsAuthored: false, textSize: 16, zoom: 100, language: null, ...stored };
  let current: Library = { ...library };
  const history: string[] = [];
  const shell: Shell = { changes: [], lookedUp: [], hold: null, holdSuggestions: null, recorded: [] };
  mockIPC(
    async (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      switch (command) {
        case "library":
          return current;
        case "suggest": {
          if (shell.holdSuggestions) await shell.holdSuggestions;
          const query = String(args.query).trim();
          const items = words.filter((w) => w.startsWith(query)).map((headword) => ({ headword, count: 2 }));
          return { items, lemma: null, near: false, miss: items.length === 0 && query !== "", browse: query === "" };
        }
        case "headwords_after":
          return [];
        case "look_up": {
          const headword = String(args.headword);
          shell.lookedUp.push(headword);
          if (shell.hold) await shell.hold;
          return words.includes(headword)
            ? [entry("learner", "Sample Learner's Dictionary", headword, "a.mp3"), entry("atlas", "Sample Atlas", headword)]
            : [];
        }
        case "appearance":
          return appearance;
        case "history":
          return history;
        case "record_lookup":
          history.unshift(String(args.headword));
          shell.recorded.push(String(args.headword));
          return history;
        case "word_books":
          return { books: [], current: null };
        case "window_chrome":
          return { platform: "windows", material: null };
        case "elsewhere":
          return null;
        case "mini_settings":
          return { lookUpSelection: "Control+Shift+F9", showMainWindow: null, watchClipboard: false, unavailable: [], problem: null };
        case "data_directory":
          return { path: "D:\\Data\\OpenDict", isDefault: true, problem: null };
      }
      if (command.startsWith("plugin:")) return null;
      shell.changes.push({ command, args });
      switch (command) {
        case "set_theme":
          appearance.theme = args.theme as Appearance["theme"];
          return null;
        case "set_language":
          appearance.language = args.language as Locale | null;
          return null;
        case "set_all_as_authored":
          appearance.allAsAuthored = args.asAuthored as boolean;
          return null;
        case "set_current_group":
          current = { ...current, currentGroup: args.group as string | null };
          return current;
        case "set_zoom":
          return args.percent;
        case "set_text_size":
          return args.size;
        case "toggle_word":
          return { books: [{ id: 1, name: "Word Book", words: [{ headword: args.headword, dictionaryId: "learner", dictionaryTitle: "x", note: null, added: 0 }] }], current: 1 };
        case "rescan_library":
          return current;
      }
      throw new Error(`unexpected command ${command}`);
    },
    { shouldMockEvents: true },
  );
  return shell;
}

function renderApp(locale: Locale = "en") {
  render(
    <I18nProvider locale={locale}>
      <App />
    </I18nProvider>,
  );
}

const field = () => screen.getByRole<HTMLInputElement>("combobox", { name: "Type a word" });

async function lookUp(word: string) {
  await userEvent.clear(field());
  await userEvent.type(field(), `${word}{Enter}`);
  await screen.findByRole("heading", { level: 1, name: `【${word}】` });
}

async function openPalette(name = "Command palette") {
  await userEvent.keyboard("{Control>}k{/Control}");
  return screen.findByRole("dialog", { name });
}

/** The palette's row for a command, found by the start of its label. */
function command(palette: HTMLElement, label: string) {
  return within(palette)
    .getAllByRole("option")
    .find((option) => option.querySelector("[data-label]")?.textContent === label)!;
}

afterEach(() => {
  vi.restoreAllMocks();
  delete document.documentElement.dataset.theme;
});

describe("the command palette", () => {
  it("opens from a keyboard-accessible button beside the search field", async () => {
    fakeShell();
    renderApp();

    const commands = screen.getByRole("button", { name: "Commands" });
    expect(commands.getAttribute("aria-haspopup")).toBe("dialog");
    expect(screen.getByRole("search").contains(commands)).toBe(false);
    field().focus();
    await userEvent.tab();
    expect(document.activeElement).toBe(commands);
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeTruthy();
  });

  it("opens with Ctrl+K and lists every action with its shortcut", async () => {
    fakeShell();
    renderApp();
    await lookUp("thumb");

    const palette = await openPalette();
    expect(document.activeElement).toBe(within(palette).getByRole("combobox", { name: "Type a command" }));
    const shortcut = (label: string) => command(palette, label)?.querySelector("kbd")?.textContent ?? null;
    expect(shortcut("Focus the search field")).toBe("/");
    expect(shortcut("Add this word to the Word Book")).toBe("Ctrl D");
    expect(shortcut("Pronounce this word")).toBe("Alt P");
    expect(shortcut("Back")).toBe(null); // nowhere to go back to yet: not listed
    expect(shortcut("Go to Sample Atlas")).toBe("Alt 2");
    expect(shortcut("Zoom in")).toBe("Ctrl =");
    for (const label of [
      "Open Library and Settings",
      "Show History",
      "Show Word Book",
      "Collapse all dictionaries",
      "Expand all dictionaries",
      "Dark theme: show every dictionary as authored",
      "Theme: light",
      "Theme: dark",
      "Interface language: 日本語",
      "Switch to Group “Reading”",
      "Add a Library Folder…",
      "Rescan the Library Folders",
    ]) {
      expect(command(palette, label), label).toBeTruthy();
    }
  });

  it("ticks the Current Group, the theme and the interface language", async () => {
    fakeShell({ theme: "dark" });
    renderApp();

    const palette = await openPalette();
    await vi.waitFor(() => expect(command(palette, "Theme: dark").getAttribute("aria-checked")).toBe("true"));
    expect(command(palette, "Theme: light").getAttribute("aria-checked")).toBe("false");
    // No language chosen yet: the interface follows the system's.
    expect(command(palette, "Interface language: Follow system").getAttribute("aria-checked")).toBe("true");
    expect(command(palette, "Interface language: English").getAttribute("aria-checked")).toBe("false");
    expect(command(palette, "Interface language: 简体中文").getAttribute("aria-checked")).toBe("false");
    expect(command(palette, "Use all dictionaries").getAttribute("aria-checked")).toBe("true");
    expect(command(palette, "Switch to Group “Reading”").getAttribute("aria-checked")).toBe("false");
    // Commands that are not a choice carry no tick at all.
    expect(command(palette, "Show History").hasAttribute("aria-checked")).toBe(false);
  });

  it("narrows to what is typed; arrows move and Enter runs the command, which switches the theme", async () => {
    const shell = fakeShell();
    renderApp();

    const palette = await openPalette();
    const input = within(palette).getByRole("combobox", { name: "Type a command" });
    await userEvent.type(input, "theme");
    const labels = within(palette)
      .getAllByRole("option")
      .map((option) => option.querySelector("[data-label]")?.textContent);
    expect(labels).toEqual(["Theme: follow system (currently light)", "Theme: light", "Theme: dark", "Dark theme: show every dictionary as authored"]);
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    expect(input.getAttribute("aria-activedescendant")).toBe(command(palette, "Theme: dark").id);
    await userEvent.keyboard("{Enter}");

    expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
    expect(shell.changes).toEqual([{ command: "set_theme", args: { theme: "dark" } }]);
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("says when no command matches", async () => {
    fakeShell();
    renderApp();
    const palette = await openPalette();
    await userEvent.type(within(palette).getByRole("combobox"), "xyzzy");
    expect(within(palette).getByText("No matching command")).toBeTruthy();
  });

  it("never runs a command on the Enter that ends an IME composition", async () => {
    const shell = fakeShell();
    renderApp();
    const palette = await openPalette();
    const input = within(palette).getByRole("combobox", { name: "Type a command" });
    await userEvent.type(input, "theme: dark");

    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    fireEvent.keyDown(input, { key: "Enter", keyCode: 229 });
    fireEvent.keyDown(input, { key: "ArrowDown", isComposing: true });

    expect(screen.getByRole("dialog", { name: "Command palette" })).toBeTruthy();
    expect(shell.changes).toEqual([]);
  });

  it("closes on Esc and gives the focus back", async () => {
    fakeShell();
    renderApp();
    await lookUp("thumb");
    field().focus();

    await openPalette();
    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
    expect(document.activeElement).toBe(field());
  });

  it("switches the Current Group", async () => {
    const shell = fakeShell();
    renderApp();
    const palette = await openPalette();
    await vi.waitFor(() => expect(command(palette, "Switch to Group “Reading”")).toBeTruthy());

    await userEvent.click(command(palette, "Switch to Group “Reading”"));

    expect(shell.changes).toEqual([{ command: "set_current_group", args: { group: "g1" } }]);
    expect(await screen.findByText("Current Group: Reading")).toBeTruthy();
  });

  it("leads to the Mini Window's settings and to moving the data directory", async () => {
    fakeShell();
    renderApp();

    await userEvent.click(command(await openPalette(), "Mini Window settings…"));
    const mini = await screen.findByRole("region", { name: "Mini Window" });
    await vi.waitFor(() => expect(mini.contains(document.activeElement)).toBe(true));

    await userEvent.click(command(await openPalette(), "Move the data directory…"));
    await vi.waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Move…" })));
  });

  it("collapses and expands every Dictionary on the page", async () => {
    fakeShell();
    renderApp();
    await lookUp("thumb");

    await userEvent.click(command(await openPalette(), "Collapse all dictionaries"));
    expect(screen.getAllByRole("button", { name: "Expand" })).toHaveLength(2);

    await userEvent.click(command(await openPalette(), "Expand all dictionaries"));
    expect(screen.getAllByRole("button", { name: "Collapse" })).toHaveLength(2);
  });
});

describe("the interface language", () => {
  it("follows the system language until the reader picks one", async () => {
    fakeShell();
    renderApp("ja");
    expect(await screen.findByRole("combobox", { name: "調べたい語を入力" })).toBeTruthy();
    expect(document.documentElement.lang).toBe("ja");
  });

  it("switches from the palette at once, without a restart, and is kept", async () => {
    const shell = fakeShell();
    renderApp("en");
    await lookUp("thumb");

    await userEvent.click(command(await openPalette(), "Interface language: 简体中文"));

    expect(await screen.findByRole("combobox", { name: "输入要查的词" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "【thumb】" })).toBeTruthy(); // the page stays
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(shell.changes).toEqual([{ command: "set_language", args: { language: "zh-CN" } }]);
  });

  it("can also be chosen under Reading in the Library view", async () => {
    const shell = fakeShell();
    renderApp("en");
    await userEvent.click(await screen.findByRole("button", { name: "Manage" }));

    const languages = await screen.findByRole("radiogroup", { name: "Interface language" });
    expect(within(languages).getByRole<HTMLInputElement>("radio", { name: "Follow system" }).checked).toBe(true);
    expect(within(languages).getByRole<HTMLInputElement>("radio", { name: "English" }).checked).toBe(false);
    await userEvent.click(within(languages).getByRole("radio", { name: "日本語" }));

    expect(await screen.findByRole("radiogroup", { name: "表示言語" })).toBeTruthy();
    expect(shell.changes).toEqual([{ command: "set_language", args: { language: "ja" } }]);
  });

  it("follows the system again once the reader picks that in the palette", async () => {
    const shell = fakeShell({ language: "ja" });
    renderApp("en");
    await screen.findByRole("combobox", { name: "調べたい語を入力" });

    const palette = await openPalette("コマンドパレット");
    expect(command(palette, "表示言語：日本語").getAttribute("aria-checked")).toBe("true");
    expect(command(palette, "表示言語：システムに合わせる").getAttribute("aria-checked")).toBe("false");
    await userEvent.click(command(palette, "表示言語：システムに合わせる"));

    expect(await screen.findByRole("combobox", { name: "Type a word" })).toBeTruthy();
    expect(document.documentElement.lang).toBe("en");
    expect(shell.changes).toEqual([{ command: "set_language", args: { language: null } }]);
  });

  it("follows the system again once the reader picks that under Reading", async () => {
    const shell = fakeShell({ language: "ja" });
    renderApp("en");
    await userEvent.click(await screen.findByRole("button", { name: "辞書と設定" }));

    const languages = await screen.findByRole("radiogroup", { name: "表示言語" });
    expect(within(languages).getByRole<HTMLInputElement>("radio", { name: "日本語" }).checked).toBe(true);
    await userEvent.click(within(languages).getByRole("radio", { name: "システムに合わせる" }));

    const followed = await screen.findByRole("radiogroup", { name: "Interface language" });
    expect(within(followed).getByRole<HTMLInputElement>("radio", { name: "Follow system" }).checked).toBe(true);
    expect(shell.changes).toEqual([{ command: "set_language", args: { language: null } }]);
  });

  it("starts in the language the reader chose before", async () => {
    fakeShell({ language: "ja" });
    renderApp("en");
    expect(await screen.findByRole("combobox", { name: "調べたい語を入力" })).toBeTruthy();
  });
});

describe("the keyboard map", () => {
  it("/ and Ctrl+L focus the search field from anywhere, but / types in a text field", async () => {
    fakeShell();
    renderApp();
    await lookUp("thumb");

    screen.getByRole("button", { name: "Pronounce" }).focus();
    await userEvent.keyboard("/");
    expect(document.activeElement).toBe(field());
    expect(field().value).toBe("thumb");
    expect(field().selectionStart).toBe(0);
    expect(field().selectionEnd).toBe(5);

    screen.getByRole("button", { name: "Pronounce" }).focus();
    await userEvent.keyboard("{Control>}l{/Control}");
    expect(document.activeElement).toBe(field());

    await userEvent.type(field(), "{End}/");
    expect(field().value).toBe("thumb/");
  });

  it("Enter before the suggestions for the typed word answer still commits it", async () => {
    const shell = fakeShell();
    renderApp();
    await lookUp("apple");

    // The suggestions for "cat" answer after Enter, while Enter's Lookup is still under way.
    let suggest!: () => void;
    let lookUpCat!: () => void;
    shell.holdSuggestions = new Promise((resolve) => (suggest = resolve));
    shell.hold = new Promise((resolve) => (lookUpCat = resolve));
    fireEvent.change(field(), { target: { value: "cat" } });
    fireEvent.submit(field().closest("form")!);
    shell.holdSuggestions = null;
    await act(async () => suggest());
    shell.hold = null;
    await act(async () => lookUpCat());
    await screen.findByRole("heading", { level: 1, name: "【cat】" });

    await vi.waitFor(() => expect(shell.recorded).toEqual(["apple", "cat"]));
    await userEvent.keyboard("{Alt>}{ArrowLeft}{/Alt}");
    await vi.waitFor(() => expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("【apple】"));
  });

  it("Back works at once after Enter, even before the Lookup has answered", async () => {
    const shell = fakeShell();
    renderApp();
    await lookUp("apple");

    let answer!: () => void;
    shell.hold = new Promise((resolve) => (answer = resolve));
    await userEvent.clear(field());
    await userEvent.type(field(), "cat{Enter}");
    await userEvent.keyboard("{Alt>}{ArrowLeft}{/Alt}");
    shell.hold = null;
    await act(async () => answer());

    await vi.waitFor(() => expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("【apple】"));
    expect(field().value).toBe("apple");
  });
});

describe("keys pressed inside an Entry", () => {
  const frame = (title: string) => screen.getByTitle(`Entry from ${title}`) as HTMLIFrameElement;
  const keyFrom = (source: MessageEventSource | null, key: Record<string, unknown>) =>
    act(() => {
      window.dispatchEvent(new MessageEvent("message", { data: { opendict: "key", altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, ...key }, source }));
    });

  it("reach the app's shortcuts: Alt+digit jumps, Alt+P pronounces, Ctrl+K opens the palette", async () => {
    fakeShell();
    const played: string[] = [];
    vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(function (this: HTMLMediaElement) {
      played.push(this.src);
      return Promise.resolve();
    });
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
    Element.prototype.scrollIntoView ??= () => {};
    renderApp();
    await lookUp("thumb");
    const learner = frame("Sample Learner's Dictionary");

    act(() => learner.focus());
    keyFrom(learner.contentWindow, { key: "2", code: "Digit2", altKey: true });
    expect(document.activeElement).toBe(screen.getByRole("region", { name: "Sample Atlas" }));

    act(() => learner.focus());
    keyFrom(learner.contentWindow, { key: "p", code: "KeyP", altKey: true });
    expect(played).toEqual(["http://odentry.localhost/learner/a.mp3"]);

    act(() => learner.focus());
    keyFrom(learner.contentWindow, { key: "k", code: "KeyK", ctrlKey: true });
    expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeTruthy();
  });

  it("are refused unless they come from the frame the reader is in, and are keys the app answers", async () => {
    fakeShell();
    renderApp();
    await lookUp("thumb");
    const learner = frame("Sample Learner's Dictionary");
    const atlas = frame("Sample Atlas");

    // Focus is not in the frame.
    keyFrom(learner.contentWindow, { key: "k", code: "KeyK", ctrlKey: true });
    // Another frame than the one the reader is in.
    act(() => learner.focus());
    keyFrom(atlas.contentWindow, { key: "k", code: "KeyK", ctrlKey: true });
    // Not from a frame at all.
    keyFrom(window, { key: "k", code: "KeyK", ctrlKey: true });
    // Keys that are no shortcut of the app, or not keys at all.
    keyFrom(learner.contentWindow, { key: "w", code: "KeyW", ctrlKey: true });
    keyFrom(learner.contentWindow, { key: "a", code: "KeyA" });
    keyFrom(learner.contentWindow, { key: { toString: () => "k" }, code: "KeyK", ctrlKey: true });
    keyFrom(learner.contentWindow, { key: "k", code: "KeyK", ctrlKey: "yes" });

    expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
  });
});
