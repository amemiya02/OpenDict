import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import type { Entry, Library, SavedWord, WordBook, WordBooks } from "./lib/ipc";

const library: Library = {
  folders: [{ path: "D:\\Dictionaries", dictionaryCount: 2, problem: null }],
  dictionaries: [
    { id: "learner", title: "Sample Learner's Dictionary", path: "D:\\Dictionaries\\learner.mdx", entryCount: 3, size: 1024, enabled: true },
    { id: "kokugo", title: "サンプル国語辞典", path: "D:\\Dictionaries\\kokugo.mdx", entryCount: 3, size: 1024, enabled: true },
  ],
  failures: [],
  problem: null,
};

const headwords = ["apple", "banter", "serendipity"];

function entry(dictionaryId: string, dictionaryTitle: string, headword: string): Entry {
  return { dictionaryId, dictionaryTitle, src: `http://odentry.localhost/${dictionaryId}/?headword=${headword}`, allowNetwork: false, pronunciation: null };
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
});
