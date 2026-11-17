import { describe, expect, it } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import type { Entry, Library, SavedWord, WordBook, WordBooks } from "./lib/ipc";

const library: Library = {
  folders: [{ path: "D:\\Dictionaries", dictionaryCount: 2, problem: null }],
  dictionaries: [
    { id: "learner", title: "Sample Learner's Dictionary", path: "D:\\Dictionaries\\learner.mdx", entryCount: 3, size: 1024, enabled: true, index: { state: "ready" } },
    { id: "kokugo", title: "サンプル国語辞典", path: "D:\\Dictionaries\\kokugo.mdx", entryCount: 3, size: 1024, enabled: true, index: { state: "ready" } },
  ],
  failures: [],
  indexing: null,
  problem: null,
  groups: [],
  currentGroup: null,
};

const headwords = ["apple", "banter", "serendipity"];

function entry(dictionaryId: string, dictionaryTitle: string, headword: string): Entry {
  return { dictionaryId, dictionaryTitle, src: `http://odentry.localhost/${dictionaryId}/?headword=${headword}`, allowNetwork: false, sound: null, pronunciation: null, asAuthored: false };
}

/** 2026-11-17 09:30 UTC */
const MORNING = 1_794_907_800_000;

/**
 * A fake shell: three headwords, each in both Dictionaries, and Word Books kept the way
 * the core keeps them. Records every Word Book command it receives.
 */
function fakeShell(start: WordBook[] = [], current: number | null = start[0]?.id ?? null) {
  const state: WordBooks = { books: structuredClone(start), current };
  let nextId = Math.max(0, ...start.map((b) => b.id + 1));
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  const book = (id: unknown) => {
    const found = state.books.find((b) => b.id === id);
    if (!found) throw new Error(`there is no Word Book ${String(id)}`);
    return found;
  };
  const create = (name: string) => {
    state.books.push({ id: nextId, name, words: [] });
    state.current = nextId++;
    return state.current;
  };
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    switch (command) {
      case "library":
        return library;
      case "look_up": {
        const headword = args.headword as string;
        return headwords.includes(headword) ? [entry("learner", "Sample Learner's Dictionary", headword), entry("kokugo", "サンプル国語辞典", headword)] : [];
      }
      case "suggest": {
        const query = (args.query as string).trim();
        const items = headwords.filter((h) => h >= query).map((headword) => ({ headword, count: 2 }));
        return { items, lemma: null, near: false, miss: query !== "" && !items[0]?.headword.startsWith(query), browse: query === "" };
      }
      case "headwords_after":
        return [];
    }
    calls.push({ command, args });
    switch (command) {
      case "word_books":
        break;
      case "toggle_word": {
        const id = state.current ?? create(args.newBookName as string);
        const words = book(id).words;
        const at = words.findIndex((w) => w.headword === args.headword);
        if (at >= 0) words.splice(at, 1);
        else
          words.unshift({
            headword: args.headword as string,
            dictionaryId: args.dictionaryId as string,
            dictionaryTitle: args.dictionaryTitle as string,
            note: null,
            added: MORNING,
          });
        break;
      }
      case "create_word_book":
        create(args.name as string);
        break;
      case "rename_word_book":
        book(args.id).name = args.name as string;
        break;
      case "delete_word_book":
        book(args.id);
        state.books = state.books.filter((b) => b.id !== args.id);
        if (state.current === args.id) state.current = state.books[0]?.id ?? null;
        break;
      case "choose_word_book":
        state.current = book(args.id).id;
        break;
      case "remove_saved_word": {
        const b = book(args.id);
        b.words = b.words.filter((w) => w.headword !== args.headword);
        break;
      }
      case "set_word_note": {
        const word = book(args.id).words.find((w) => w.headword === args.headword)!;
        word.note = (args.note as string).trim() || null;
        break;
      }
      case "export_word_book":
        return `C:\\Users\\reader\\Documents\\${book(args.id).name}.${args.format as string}`;
      default:
        throw new Error(`unexpected command ${command}`);
    }
    return structuredClone(state);
  });
  return { calls, state };
}

function saved(headword: string, note: string | null = null): SavedWord {
  return { headword, dictionaryId: "learner", dictionaryTitle: "Sample Learner's Dictionary", note, added: MORNING };
}

function renderApp() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  return screen.getByRole<HTMLInputElement>("combobox", { name: "Type a word" });
}

async function lookUp(word: string) {
  const search = renderApp();
  await userEvent.type(search, `${word}{Enter}`);
  await screen.findByRole("heading", { level: 1, name: `【${word}】` });
  return search;
}

describe("Word Books", () => {
  it("Ctrl+D saves the word on the page with the Dictionary it came from, and again takes it out", async () => {
    const { calls, state } = fakeShell();
    await lookUp("serendipity");
    expect(screen.getByRole("button", { name: "Add to Word Book", pressed: false })).toBeTruthy();

    await userEvent.keyboard("{Control>}d{/Control}");

    await screen.findByRole("button", { name: "In Word Book", pressed: true });
    expect(calls.find((c) => c.command === "toggle_word")?.args).toEqual({
      headword: "serendipity",
      dictionaryId: "learner",
      dictionaryTitle: "Sample Learner's Dictionary",
      newBookName: "Word Book",
    });
    expect(state.books[0]!.words.map((w) => w.headword)).toEqual(["serendipity"]);

    await userEvent.keyboard("{Control>}d{/Control}");

    await screen.findByRole("button", { name: "Add to Word Book", pressed: false });
    expect(state.books[0]!.words).toEqual([]);
  });

  it("the button in the page head saves the word too", async () => {
    const { state } = fakeShell([{ id: 4, name: "Reading", words: [saved("apple")] }]);
    await lookUp("banter");

    await userEvent.click(screen.getByRole("button", { name: "Add to Word Book" }));

    await screen.findByRole("button", { name: "In Word Book", pressed: true });
    expect(state.books[0]!.words.map((w) => w.headword)).toEqual(["banter", "apple"]);
  });

  it("a word already in the chosen Word Book shows as saved", async () => {
    fakeShell([{ id: 4, name: "Reading", words: [saved("serendipity")] }]);
    await lookUp("serendipity");

    await screen.findByRole("button", { name: "In Word Book", pressed: true });
  });

  it("the Word Book tab lists the chosen Word Book in the column, newest first, and a click looks a word up", async () => {
    fakeShell([{ id: 4, name: "Reading", words: [saved("serendipity"), saved("apple")] }]);
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Word Book" }));

    const column = screen.getByRole("listbox", { name: "Headwords" });
    expect(within(column).getAllByRole("option").map((o) => o.dataset.headword)).toEqual(["serendipity", "apple"]);
    expect(screen.getByRole("button", { name: "Word Book", pressed: true })).toBeTruthy();
    await screen.findByRole("heading", { level: 1, name: "【Reading】" });

    await userEvent.click(within(column).getByRole("option", { name: "apple" }));

    await screen.findByRole("heading", { level: 1, name: "【apple】" });
    expect(within(column).getByRole("option", { name: "apple", selected: true })).toBeTruthy();
  });

  it("the reader chooses which of several Word Books Ctrl+D saves into", async () => {
    const { state } = fakeShell([
      { id: 1, name: "Novel", words: [] },
      { id: 2, name: "Paper", words: [] },
    ]);
    renderApp();
    await userEvent.click(await screen.findByRole("button", { name: "Word Book" }));
    await screen.findByRole("heading", { level: 1, name: "【Novel】" });

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Saving to" }), "Paper");
    await screen.findByRole("heading", { level: 1, name: "【Paper】" });
    await userEvent.type(screen.getByRole("combobox", { name: "Type a word" }), "banter{Enter}");
    await screen.findByRole("heading", { level: 1, name: "【banter】" });
    await userEvent.keyboard("{Control>}d{/Control}");

    await screen.findByRole("button", { name: "In Word Book", pressed: true });
    expect(screen.getByRole("status").textContent).toBe("Saved “banter” to Paper.");
    expect(state.books.map((b) => [b.name, b.words.map((w) => w.headword)])).toEqual([
      ["Novel", []],
      ["Paper", ["banter"]],
    ]);
  });

  it("makes, renames and deletes Word Books", async () => {
    const { state } = fakeShell([{ id: 1, name: "Novel", words: [saved("banter")] }]);
    renderApp();
    await userEvent.click(await screen.findByRole("button", { name: "Word Book" }));

    await userEvent.click(await screen.findByRole("button", { name: "New Word Book…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "Paper reading{Enter}");
    await screen.findByRole("heading", { level: 1, name: "【Paper reading】" });

    await userEvent.click(screen.getByRole("button", { name: "Rename…" }));
    const name = screen.getByRole("textbox", { name: "Name" });
    expect((name as HTMLInputElement).value).toBe("Paper reading");
    await userEvent.clear(name);
    await userEvent.type(name, "Papers{Enter}");
    await screen.findByRole("heading", { level: 1, name: "【Papers】" });

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Saving to" }), "Novel");
    await userEvent.click(await screen.findByRole("button", { name: "Delete…" }));
    expect(screen.getByText("Delete “Novel” and its 1 word?")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Keep" }));
    expect(state.books).toHaveLength(2);
    await userEvent.click(screen.getByRole("button", { name: "Delete…" }));
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));

    await screen.findByRole("heading", { level: 1, name: "【Papers】" });
    expect(state.books.map((b) => b.name)).toEqual(["Papers"]);
  });

  it("keeps a note or example sentence with a saved word, and removes words", async () => {
    const { state } = fakeShell([{ id: 1, name: "Novel", words: [saved("banter"), saved("apple")] }]);
    renderApp();
    await userEvent.click(await screen.findByRole("button", { name: "Word Book" }));

    await userEvent.type(await screen.findByRole("textbox", { name: "Note for banter" }), "Some banter at breakfast{Enter}");
    await waitFor(() => expect(state.books[0]!.words[0]!.note).toBe("Some banter at breakfast"));

    await userEvent.click(screen.getByRole("button", { name: "Remove apple" }));

    await waitFor(() => expect(screen.queryByRole("button", { name: "Remove apple" })).toBeNull());
    expect(state.books[0]!.words.map((w) => w.headword)).toEqual(["banter"]);
    expect(within(screen.getByRole("listbox", { name: "Headwords" })).getAllByRole("option").map((o) => o.dataset.headword)).toEqual(["banter"]);
    expect((screen.getByRole("textbox", { name: "Note for banter" }) as HTMLInputElement).value).toBe("Some banter at breakfast");
  });

  it("exports a Word Book to CSV or TSV and says where", async () => {
    const { calls } = fakeShell([{ id: 7, name: "Novel", words: [saved("banter")] }]);
    renderApp();
    await userEvent.click(await screen.findByRole("button", { name: "Word Book" }));

    await userEvent.click(await screen.findByRole("button", { name: "Export CSV…" }));
    await screen.findByText(String.raw`Exported to C:\Users\reader\Documents\Novel.csv`);
    await userEvent.click(screen.getByRole("button", { name: "Export TSV…" }));
    await screen.findByText(String.raw`Exported to C:\Users\reader\Documents\Novel.tsv`);

    expect(calls.filter((c) => c.command === "export_word_book").map((c) => c.args)).toEqual([
      { id: 7, format: "csv" },
      { id: 7, format: "tsv" },
    ]);
  });

  it("without any Word Book, the view offers to make one", async () => {
    fakeShell();
    renderApp();
    await userEvent.click(await screen.findByRole("button", { name: "Word Book" }));

    await screen.findByText(/No Word Books yet\./);
    expect(screen.queryByRole("button", { name: "Export CSV…" })).toBeNull();
    expect(screen.getByRole("button", { name: "New Word Book…" })).toBeTruthy();
  });
});
