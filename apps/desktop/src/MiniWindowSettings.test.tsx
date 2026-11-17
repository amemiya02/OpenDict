import { describe, expect, it } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import type { MiniSettings, WordBooks } from "./lib/ipc";

/**
 * A fake shell with one Dictionary that files "serendipity" and "serene", and the Mini
 * Window's settings kept the way the shell keeps them. Records every settings command.
 */
function fakeShell(start: Partial<MiniSettings> = {}, refuse: Record<string, string> = {}) {
  const words = ["serendipity", "serene"];
  const settings: MiniSettings = {
    lookUpSelection: "Control+Alt+KeyD",
    showMainWindow: "Control+Alt+KeyO",
    watchClipboard: false,
    unavailable: [],
    problem: null,
    ...start,
  };
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      switch (command) {
        case "library":
          return {
            folders: [{ path: "/library", dictionaryCount: 1, problem: null }],
            dictionaries: [{ id: "a", title: "A", path: "/library/a.mdx", entryCount: 2, size: 1, enabled: true, index: { state: "ready" } }],
            failures: [],
            indexing: null,
            problem: null,
          };
        case "suggest": {
          const query = (args.query as string).trim();
          const items = words.filter((word) => word.startsWith(query)).map((headword) => ({ headword, count: 1 }));
          return { items, lemma: null, near: false, miss: items.length === 0, browse: !query };
        }
        case "headwords_after":
          return [];
        case "look_up":
          return words.includes(args.headword as string)
            ? [{ dictionaryId: "a", dictionaryTitle: "A", src: "http://odentry.localhost/a", allowNetwork: false, sound: null, pronunciation: null }]
            : [];
        case "word_books":
          return { books: [], current: null };
        case "history":
          return [];
        case "record_lookup":
          return [args.headword];
        case "window_chrome":
          return { platform: "windows", material: null };
        case "mini_settings":
          return structuredClone(settings);
      }
      calls.push({ command, args });
      switch (command) {
        case "set_shortcut": {
          const accelerator = args.accelerator as string | null;
          if (accelerator && refuse[accelerator]) throw refuse[accelerator];
          settings[args.role as "lookUpSelection" | "showMainWindow"] = accelerator;
          return structuredClone(settings);
        }
        case "set_watch_clipboard":
          settings.watchClipboard = args.on as boolean;
          return structuredClone(settings);
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return calls;
}

function renderApp() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
}

async function openSettings() {
  renderApp();
  await userEvent.click(screen.getByRole("button", { name: "Library" }));
  return screen.findByRole("region", { name: "Mini Window" });
}

describe("the main window and the Mini Window", () => {
  it("looks up the word the Mini Window opens in it", async () => {
    fakeShell();
    renderApp();
    await act(() => emit("open-in-main", "serene"));
    expect(await screen.findByRole("heading", { level: 1, name: "【serene】" })).toBeTruthy();
    expect(screen.getByRole<HTMLInputElement>("combobox", { name: "Type a word" }).value).toBe("serene");
  });

  it("shows a word saved in the Mini Window as saved", async () => {
    fakeShell();
    renderApp();
    await act(() => emit("open-in-main", "serene"));
    await screen.findByRole("heading", { level: 1, name: "【serene】" });
    expect(screen.getByRole("button", { name: /Add to Word Book/ }).getAttribute("aria-pressed")).toBe("false");

    const books: WordBooks = {
      books: [{ id: 0, name: "Word Book", words: [{ headword: "serene", dictionaryId: "a", dictionaryTitle: "A", note: null, added: 0 }] }],
      current: 0,
    };
    await act(() => emit("word-books", books));
    expect(screen.getByRole("button", { name: /In Word Book/ }).getAttribute("aria-pressed")).toBe("true");
  });
});

describe("Mini Window settings", () => {
  it("show both global shortcuts, and clipboard watching off by default", async () => {
    fakeShell();
    const section = await openSettings();
    expect(within(section).getByRole("button", { name: "Change the shortcut: Look up the selection" }).textContent).toBe("Ctrl Alt D");
    expect(within(section).getByRole("button", { name: "Change the shortcut: Bring the main window forward" }).textContent).toBe("Ctrl Alt O");
    expect(within(section).getByRole<HTMLInputElement>("checkbox", { name: /clipboard watching/ }).checked).toBe(false);
  });

  it("record a new shortcut from the keys the reader presses", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    const section = await openSettings();
    const change = within(section).getByRole("button", { name: "Change the shortcut: Look up the selection" });

    await user.click(change);
    expect(change.textContent).toBe("Press the new shortcut…");
    await user.keyboard("{Control>}{Shift>}{l}{/Shift}{/Control}");
    expect(calls).toContainEqual({ command: "set_shortcut", args: { role: "lookUpSelection", accelerator: "Control+Shift+KeyL" } });
    await waitFor(() => expect(change.textContent).toBe("Ctrl Shift L"));
  });

  it("refuse a key without a modifier, and Esc leaves the shortcut as it was", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    const section = await openSettings();
    const change = within(section).getByRole("button", { name: "Change the shortcut: Look up the selection" });

    await user.click(change);
    await user.keyboard("l");
    expect(within(section).getByText("A shortcut needs Ctrl, Alt, Shift or ⌘ and one other key.")).toBeTruthy();
    await user.keyboard("{Escape}");
    expect(change.textContent).toBe("Ctrl Alt D");
    expect(calls.filter((c) => c.command === "set_shortcut")).toHaveLength(0);
  });

  it("say why the shell refused a shortcut", async () => {
    fakeShell({}, { "Control+Alt+KeyL": "Control+Alt+KeyL is already the other shortcut" });
    const user = userEvent.setup();
    const section = await openSettings();
    await user.click(within(section).getByRole("button", { name: "Change the shortcut: Bring the main window forward" }));
    await user.keyboard("{Control>}{Alt>}{l}{/Alt}{/Control}");
    expect(await within(section).findByText(/already the other shortcut/)).toBeTruthy();
  });

  it("mark a shortcut another application already has", async () => {
    fakeShell({ unavailable: ["lookUpSelection"] });
    const section = await openSettings();
    expect(within(section).getByText("Another application already uses this shortcut.")).toBeTruthy();
  });

  it("turn a shortcut off", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    const section = await openSettings();
    await user.click(within(section).getByRole("button", { name: "Turn off the shortcut: Bring the main window forward" }));
    expect(calls).toContainEqual({ command: "set_shortcut", args: { role: "showMainWindow", accelerator: null } });
    await waitFor(() =>
      expect(within(section).getByRole("button", { name: "Change the shortcut: Bring the main window forward" }).textContent).toBe("Off"),
    );
  });

  it("turn clipboard watching on", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    const section = await openSettings();
    const watch = within(section).getByRole<HTMLInputElement>("checkbox", { name: /clipboard watching/ });
    await user.click(watch);
    expect(calls).toContainEqual({ command: "set_watch_clipboard", args: { on: true } });
    await waitFor(() => expect(watch.checked).toBe(true));
  });
});
