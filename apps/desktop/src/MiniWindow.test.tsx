import { describe, expect, it } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { MiniApp } from "./MiniApp";
import { I18nProvider } from "./i18n";
import type { Entry, WordBooks } from "./lib/ipc";

function entry(dictionaryId: string, dictionaryTitle: string, headword: string): Entry {
  return {
    dictionaryId,
    dictionaryTitle,
    src: `http://odentry.localhost/${dictionaryId}/?headword=${headword}`,
    allowNetwork: false,
    sound: null,
    pronunciation: dictionaryId === "learner" ? "/sɜːv/" : null,
  };
}

/**
 * A fake shell: "serve" and "serene" are filed in two Dictionaries, "served" lemmatizes to
 * "serve". Records every command the Mini Window sends that is not a Lookup.
 */
function fakeShell({ last = null as { text: string; serial: number } | null, books = { books: [], current: null } as WordBooks } = {}) {
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  let state = structuredClone(books);
  mockWindows("mini", "main");
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      switch (command) {
        case "suggest": {
          const query = (args.query as string).trim();
          if (query === "served") return { items: [{ headword: "serve", count: 2 }], lemma: { from: "served", to: "serve" }, near: false, miss: false, browse: false };
          const items = ["serene", "serve"].filter((h) => h.startsWith(query)).map((headword) => ({ headword, count: 2 }));
          return { items, lemma: null, near: false, miss: items.length === 0, browse: query === "" };
        }
        case "look_up": {
          const headword = args.headword as string;
          return ["serve", "serene"].includes(headword)
            ? [entry("learner", "Sample Learner's Dictionary", headword), entry("kokugo", "サンプル国語辞典", headword)]
            : [];
        }
        case "mini_summons":
          return last;
        case "word_books":
          return state;
      }
      calls.push({ command, args });
      if (command === "toggle_word") {
        const words = state.books[0]?.words ?? [];
        const at = words.findIndex((w) => w.headword === args.headword);
        if (at >= 0) words.splice(at, 1);
        else words.unshift({ headword: args.headword as string, dictionaryId: args.dictionaryId as string, dictionaryTitle: args.dictionaryTitle as string, note: null, added: 0 });
        if (state.books.length === 0) state = { books: [{ id: 0, name: args.newBookName as string, words }], current: 0 };
        return state;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return calls;
}

function renderMini() {
  return render(
    <I18nProvider locale="en">
      <MiniApp />
    </I18nProvider>,
  );
}

async function summon(text: string, serial = 1) {
  await act(() => emit("mini-summoned", { text, serial }));
}

describe("Mini Window", () => {
  it("looks the selection up as the main window does: Lemmatization note, every answering Dictionary, a compact thumb index", async () => {
    fakeShell();
    renderMini();
    await summon("served");

    await screen.findByRole("heading", { level: 1, name: "【serve】" });
    expect((screen.getByRole("searchbox", { name: "Type a word" }) as HTMLInputElement).value).toBe("served");
    expect(screen.getByText(/Lemmatized/).textContent).toBe("Lemmatized served → serve");
    screen.getByText("/sɜːv/");

    const sections = screen.getAllByRole("region");
    expect(sections.map((s) => s.getAttribute("aria-label"))).toEqual(["Sample Learner's Dictionary", "サンプル国語辞典"]);
    for (const section of sections) {
      expect(within(section).getByTitle(/Entry from/).getAttribute("sandbox")).toBe("allow-scripts");
    }
    const index = screen.getByRole("navigation", { name: "Jump to dictionary" });
    expect(within(index).getAllByRole("link")).toHaveLength(2);
  });

  it("shows the latest summons when it loads after it", async () => {
    fakeShell({ last: { text: "serene", serial: 3 } });
    renderMini();
    await screen.findByRole("heading", { level: 1, name: "【serene】" });
  });

  it("looks up again when the reader corrects the word in its search field", async () => {
    fakeShell();
    const user = userEvent.setup();
    renderMini();
    await summon("serene");
    await screen.findByRole("heading", { level: 1, name: "【serene】" });

    const field = screen.getByRole("searchbox", { name: "Type a word" });
    await user.clear(field);
    await user.type(field, "serv");
    await screen.findByRole("heading", { level: 1, name: "【serve】" });
  });

  it("says so when no Dictionary has the word, and offers it to the main window as typed", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    renderMini();
    await summon("xyzzy");
    await screen.findByText("No dictionary has “xyzzy”.");
    expect((screen.getByRole("button", { name: /Add to Word Book/ }) as HTMLButtonElement).disabled).toBe(true);

    await user.click(screen.getByRole("button", { name: "Open in main window" }));
    expect(calls).toContainEqual({ command: "open_in_main", args: { headword: "xyzzy" } });
  });

  it("closes with Esc, giving the reader's focus back", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    renderMini();
    await summon("serve");
    await screen.findByRole("heading", { level: 1, name: "【serve】" });

    await user.keyboard("{Escape}");
    expect(calls.filter((c) => c.command === "close_mini")).toHaveLength(1);
  });

  it("closes with its close button too", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    renderMini();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(calls.map((c) => c.command)).toContain("close_mini");
  });

  it("adds the word to the Word Book, from the first answering Dictionary", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    renderMini();
    await summon("serve");
    await screen.findByRole("heading", { level: 1, name: "【serve】" });

    const save = screen.getByRole("button", { name: /Add to Word Book/ });
    expect(save.getAttribute("aria-pressed")).toBe("false");
    await user.click(save);
    expect(calls).toContainEqual({
      command: "toggle_word",
      args: { headword: "serve", dictionaryId: "learner", dictionaryTitle: "Sample Learner's Dictionary", newBookName: "Word Book" },
    });
    await waitFor(() => expect(screen.getByRole("button", { name: /In Word Book/ }).getAttribute("aria-pressed")).toBe("true"));
  });

  it("follows Word Book changes made in the main window", async () => {
    fakeShell();
    renderMini();
    await summon("serve");
    await screen.findByRole("heading", { level: 1, name: "【serve】" });

    const saved: WordBooks = {
      books: [{ id: 0, name: "Word Book", words: [{ headword: "serve", dictionaryId: "learner", dictionaryTitle: "L", note: null, added: 0 }] }],
      current: 0,
    };
    await act(() => emit("word-books", saved));
    expect(screen.getByRole("button", { name: /In Word Book/ }).getAttribute("aria-pressed")).toBe("true");
  });

  it("opens the word in the main window", async () => {
    const calls = fakeShell();
    const user = userEvent.setup();
    renderMini();
    await summon("served");
    await screen.findByRole("heading", { level: 1, name: "【serve】" });

    await user.click(screen.getByRole("button", { name: "Open in main window" }));
    expect(calls).toContainEqual({ command: "open_in_main", args: { headword: "serve" } });
  });

  it("opens empty, ready to type, when nothing was selected", async () => {
    fakeShell();
    renderMini();
    await summon("");
    const field = screen.getByRole("searchbox", { name: "Type a word" }) as HTMLInputElement;
    expect(field.value).toBe("");
    expect(document.activeElement).toBe(field);
    expect(screen.queryAllByRole("region")).toHaveLength(0);
  });
});
