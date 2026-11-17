import { describe, expect, it } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import { PAGE_SIZE } from "./lib/useHeadwordColumn";
import type { Suggestion } from "./lib/ipc";

/**
 * A fake shell with a headword index in index order (each headword with how many
 * Dictionaries file it). It answers suggest and headwords_after the way the core does
 * and records every command it receives. `lemmas` maps inflected forms to base forms,
 * standing in for the core's Lemmatization.
 */
function fakeShell(index: Suggestion[], lemmas: Record<string, string> = {}) {
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  const position = (key: string) => {
    const at = index.findIndex((item) => item.headword.toLowerCase() >= key);
    return at < 0 ? index.length : at;
  };
  const library = {
    folders: [{ path: "/library", dictionaryCount: 1, problem: null }],
    dictionaries: [{ id: "a", title: "A", path: "/library/a.mdx", entryCount: index.length, size: 1, enabled: true, index: { state: "ready" } }],
    failures: [],
    indexing: null,
    problem: null,
    groups: [],
    currentGroup: null,
  };
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ command, args });
    switch (command) {
      case "library":
      case "rescan_library":
        return library;
      case "suggest": {
        const key = (args.query as string).trim().toLowerCase();
        const base = lemmas[key];
        if (base) {
          const items = index.slice(position(base)).slice(0, args.limit as number);
          return { items, lemma: { from: (args.query as string).trim(), to: base }, near: false, miss: false, browse: false };
        }
        const items = index.slice(position(key)).slice(0, args.limit as number);
        const browse = key === "";
        return { items, lemma: null, near: false, miss: !browse && !items[0]?.headword.toLowerCase().startsWith(key), browse };
      }
      case "headwords_after": {
        const at = index.findIndex((item) => item.headword === args.headword);
        return index.slice(at + 1).slice(0, args.limit as number);
      }
      case "look_up": {
        const headword = args.headword as string;
        return index.some((item) => item.headword === headword)
          ? [{ dictionaryId: "a", dictionaryTitle: "A", html: `<p>${headword}</p>` }]
          : [];
      }
      default:
        throw new Error(`unexpected command ${command}`);
    }
  }, { shouldMockEvents: true });
  return calls;
}

const words: Suggestion[] = [
  { headword: "apple", count: 2 },
  { headword: "sea", count: 1 },
  { headword: "serendipity", count: 3 },
  { headword: "serene", count: 1 },
  { headword: "shadow", count: 2 },
];

function renderApp() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  return screen.getByRole<HTMLInputElement>("combobox", { name: "Type a word" });
}

const column = () => screen.getByRole("listbox", { name: "Headwords" });
const shownHeadwords = () => within(column()).queryAllByRole("option").map((option) => option.dataset.headword);
const pageHeadword = () => screen.getByRole("heading", { level: 1 }).textContent;

describe("headword column", () => {
  it("suggests prefix matches as the reader types and carries on through the index", async () => {
    fakeShell(words);
    const search = renderApp();

    await userEvent.type(search, "ser");

    await waitFor(() => expect(shownHeadwords()).toEqual(["serendipity", "serene", "shadow"]));
    // The first match is current and already on the page.
    expect(within(column()).getByRole("option", { selected: true }).dataset.headword).toBe("serendipity");
    await waitFor(() => expect(pageHeadword()).toBe("【serendipity】"));
  });

  it("notes the base form an inflected form was taken to, and shows its Entry", async () => {
    fakeShell(words, { serendipities: "serendipity" });
    const search = renderApp();

    await userEvent.type(search, "serendipities");

    const note = await screen.findByText((_, element) => element?.tagName === "P" && element.textContent === "Lemmatized serendipities → serendipity");
    expect(within(note).getByText("serendipities").tagName).toBe("B");
    expect(within(column()).getByRole("option", { selected: true }).dataset.headword).toBe("serendipity");
    await waitFor(() => expect(pageHeadword()).toBe("【serendipity】"));
  });

  it("shows how many Dictionaries have each suggestion", async () => {
    fakeShell(words);
    const search = renderApp();

    await userEvent.type(search, "ser");

    const option = await within(column()).findByRole("option", { name: "serendipity, in 3 dictionaries" });
    expect(option.textContent).toContain("3");
  });

  it("moves through the suggestions with the arrow keys, showing each at once, and Enter commits", async () => {
    fakeShell(words);
    const search = renderApp();
    await userEvent.type(search, "ser");
    await waitFor(() => expect(pageHeadword()).toBe("【serendipity】"));

    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(pageHeadword()).toBe("【serene】"));
    expect(search.getAttribute("aria-activedescendant")).toBe(
      within(column()).getByRole("option", { selected: true }).id,
    );
    await userEvent.keyboard("{ArrowDown}{ArrowUp}{ArrowUp}");
    await waitFor(() => expect(pageHeadword()).toBe("【serendipity】"));

    await userEvent.keyboard("{ArrowDown}{Enter}");
    await waitFor(() => expect(pageHeadword()).toBe("【serene】"));
    expect(search.value).toBe("serene");
  });

  it("sets guide words, the first and last headwords shown, above the column", async () => {
    fakeShell(words);
    const search = renderApp();

    await userEvent.type(search, "se");

    const guide = await screen.findByTestId("guide-words");
    await waitFor(() => expect(guide.textContent).toBe("seashadow"));
    expect(within(guide).getByText("sea")).toBeTruthy();
    expect(within(guide).getByText("shadow")).toBeTruthy();
  });

  it("shows the headword index to browse when the search field is empty", async () => {
    const calls = fakeShell(words);
    renderApp();

    await waitFor(() => expect(shownHeadwords()).toEqual(words.map((w) => w.headword)));
    expect(calls).toContainEqual({ command: "suggest", args: { query: "", limit: PAGE_SIZE } });
    // Browsing, nothing is current until the reader picks a word.
    expect(within(column()).queryByRole("option", { selected: true })).toBeNull();
  });

  it("loads the headwords that follow when the reader moves past the end", async () => {
    const word = (i: number) => `w${String(i).padStart(4, "0")}`;
    const long: Suggestion[] = Array.from({ length: PAGE_SIZE + 1 }, (_, i) => ({ headword: word(i), count: 1 }));
    const calls = fakeShell(long);
    const search = renderApp();
    await waitFor(() => expect(shownHeadwords()).toHaveLength(PAGE_SIZE));
    search.focus();

    // Nothing is selected while browsing: ↑ starts from the last headword shown.
    await userEvent.keyboard("{ArrowUp}");
    await waitFor(() => expect(pageHeadword()).toBe(`【${word(PAGE_SIZE - 1)}】`));
    await userEvent.keyboard("{ArrowDown}");

    await waitFor(() => expect(pageHeadword()).toBe(`【${word(PAGE_SIZE)}】`));
    expect(calls).toContainEqual({ command: "headwords_after", args: { query: "", headword: word(PAGE_SIZE - 1), limit: PAGE_SIZE } });
    expect(shownHeadwords()).toHaveLength(PAGE_SIZE + 1);
  });

  it("keeps the reader's query when the Library changes, and moves the Library into the column's tabs", async () => {
    const calls = fakeShell(words);
    const search = renderApp();
    await userEvent.type(search, "ser");
    await waitFor(() => expect(shownHeadwords()).toEqual(["serendipity", "serene", "shadow"]));

    await userEvent.click(within(screen.getByRole("complementary", { name: "Headword column" })).getByRole("button", { name: "Library" }));
    await userEvent.click(await screen.findByRole("button", { name: "Rescan" }));

    await waitFor(() => expect(calls.at(-1)).toEqual({ command: "suggest", args: { query: "ser", limit: PAGE_SIZE } }));
    expect(shownHeadwords()).toEqual(["serendipity", "serene", "shadow"]);
    // In the Library, nothing in the column is current.
    expect(within(column()).queryByRole("option", { selected: true })).toBeNull();
  });

  it("commits a suggestion the reader clicks", async () => {
    fakeShell(words);
    const search = renderApp();
    await userEvent.type(search, "s");

    await userEvent.click(await within(column()).findByRole("option", { name: /^shadow/ }));

    await waitFor(() => expect(pageHeadword()).toBe("【shadow】"));
    expect(search.value).toBe("shadow");
  });

  it("never looks up or commits while an IME is composing", async () => {
    const calls = fakeShell([...words, { headword: "食べる", count: 1 }]);
    const search = renderApp();
    await waitFor(() => expect(shownHeadwords()).toHaveLength(6));
    const before = calls.length;

    fireEvent.compositionStart(search);
    compose(search, "たべ");
    fireEvent.keyDown(search, { key: "ArrowDown", keyCode: 229, isComposing: true });
    compose(search, "食べ");
    // The Enter that confirms the composition.
    fireEvent.keyDown(search, { key: "Enter", keyCode: 229, isComposing: true });

    expect(calls.slice(before)).toEqual([]);
    expect(search.value).toBe("食べ");

    fireEvent.compositionEnd(search, { data: "食べ" });

    await waitFor(() => expect(calls.slice(before)).toContainEqual({ command: "suggest", args: { query: "食べ", limit: PAGE_SIZE } }));
    expect(calls.slice(before).filter((call) => call.command !== "suggest" && call.command !== "look_up")).toEqual([]);
  });
});

/** What an IME does while composing: the field's text changes with isComposing set. */
function compose(input: HTMLInputElement, text: string) {
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, text);
    input.dispatchEvent(new InputEvent("input", { bubbles: true, isComposing: true, data: text }));
  });
}
