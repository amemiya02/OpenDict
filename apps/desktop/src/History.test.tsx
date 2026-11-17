import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";

const index = ["apple", "sea", "serendipity", "serene"];

/**
 * A fake shell with a small headword index, one Dictionary that has every headword in it,
 * and a History it keeps the way the core does (newest first, each word once).
 */
function fakeShell(initialHistory: string[] = []) {
  let history = [...initialHistory];
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ command, args });
    switch (command) {
      case "library":
        return {
          folders: [{ path: "/library", dictionaryCount: 1, problem: null }],
          dictionaries: [{ id: "a", title: "A", path: "/library/a.mdx", entryCount: index.length, size: 1, enabled: true }],
          failures: [],
          problem: null,
        };
      case "suggest": {
        const key = (args.query as string).trim().toLowerCase();
        const at = index.findIndex((word) => word >= key);
        const items = (at < 0 ? [] : index.slice(at)).map((headword) => ({ headword, count: 1 }));
        const browse = key === "";
        return { items, lemma: null, near: false, miss: !browse && !items[0]?.headword.startsWith(key), browse };
      }
      case "headwords_after":
        return [];
      case "look_up": {
        const headword = args.headword as string;
        return index.includes(headword)
          ? [{ dictionaryId: "a", dictionaryTitle: "A", src: `http://odentry.localhost/a/?headword=${headword}`, allowNetwork: false }]
          : [];
      }
      case "history":
        return history;
      case "record_lookup": {
        const headword = args.headword as string;
        history = [headword, ...history.filter((word) => word !== headword)];
        return history;
      }
      case "clear_history":
        history = [];
        return null;
      default:
        throw new Error(`unexpected command ${command}`);
    }
  });
  return { calls, history: () => history };
}

function renderApp() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  return screen.getByRole<HTMLInputElement>("combobox", { name: "Type a word" });
}

const column = () => screen.getByRole("listbox", { name: "Headwords" });
const listed = () => within(column()).queryAllByRole("option").map((option) => option.dataset.headword);
const pageHeadword = () => screen.getByRole("heading", { level: 1 }).textContent;
const historyTab = () => screen.getByRole("button", { name: "History" });
const back = () => screen.getByRole<HTMLButtonElement>("button", { name: "Back" });
const forward = () => screen.getByRole<HTMLButtonElement>("button", { name: "Forward" });

async function lookUp(field: HTMLInputElement, word: string) {
  await userEvent.clear(field);
  await userEvent.type(field, `${word}{Enter}`);
  await waitFor(() => expect(pageHeadword()).toBe(`【${word}】`));
}

describe("History", () => {
  it("records committed Lookups and lists them newest first in the History tab", async () => {
    const shell = fakeShell();
    const field = renderApp();

    await lookUp(field, "apple");
    await lookUp(field, "serendipity");
    await waitFor(() => expect(shell.history()).toEqual(["serendipity", "apple"]));
    await userEvent.click(historyTab());

    expect(historyTab().getAttribute("aria-pressed")).toBe("true");
    await waitFor(() => expect(listed()).toEqual(["serendipity", "apple"]));
  });

  it("does not record what the reader only typed or browsed past", async () => {
    const shell = fakeShell();
    const field = renderApp();

    await userEvent.type(field, "sea");
    await waitFor(() => expect(pageHeadword()).toBe("【sea】"));
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(pageHeadword()).toBe("【serendipity】"));

    expect(shell.calls.filter((call) => call.command === "record_lookup")).toEqual([]);
  });

  it("records a word clicked in the headword column", async () => {
    const shell = fakeShell();
    const field = renderApp();
    await userEvent.type(field, "se");
    await waitFor(() => expect(listed()).toContain("serene"));

    await userEvent.click(within(column()).getByRole("option", { name: /^serene/ }));

    await waitFor(() => expect(shell.history()).toEqual(["serene"]));
  });

  it("shows History kept from earlier sessions, and a word picked there is looked up again", async () => {
    const shell = fakeShell(["serene", "apple"]);
    renderApp();

    await userEvent.click(historyTab());
    await waitFor(() => expect(listed()).toEqual(["serene", "apple"]));
    await userEvent.click(within(column()).getByRole("option", { name: /^apple/ }));

    await waitFor(() => expect(pageHeadword()).toBe("【apple】"));
    expect(screen.getByRole<HTMLInputElement>("combobox").value).toBe("apple");
    await waitFor(() => expect(shell.history()).toEqual(["apple", "serene"]));
    await waitFor(() => expect(listed()).toEqual(["apple", "serene"]));
  });

  it("can be cleared after the reader confirms", async () => {
    const shell = fakeShell(["serene", "apple"]);
    renderApp();
    await userEvent.click(historyTab());
    await waitFor(() => expect(listed()).toEqual(["serene", "apple"]));

    await userEvent.click(screen.getByRole("button", { name: "Clear History…" }));
    await userEvent.click(screen.getByRole("button", { name: "Keep" }));
    expect(listed()).toEqual(["serene", "apple"]);

    await userEvent.click(screen.getByRole("button", { name: "Clear History…" }));
    await userEvent.click(screen.getByRole("button", { name: "Clear" }));

    await waitFor(() => expect(listed()).toEqual([]));
    expect(shell.history()).toEqual([]);
    expect(screen.getByText("No lookups yet")).toBeTruthy();
  });

  it("typing a word goes back to the suggestions", async () => {
    fakeShell(["serene"]);
    const field = renderApp();
    await userEvent.click(historyTab());

    await userEvent.type(field, "app");

    await waitFor(() => expect(screen.getByRole("button", { name: "Suggestions" }).getAttribute("aria-pressed")).toBe("true"));
    await waitFor(() => expect(listed()?.[0]).toBe("apple"));
  });

  it("lists recent lookups on the empty page", async () => {
    fakeShell(["serene", "apple"]);
    renderApp();

    const recent = await screen.findByRole("list", { name: "Recent lookups" });
    expect(within(recent).getAllByRole("button").map((button) => button.textContent)).toEqual(["serene", "apple"]);
    await userEvent.click(within(recent).getByRole("button", { name: "apple" }));

    await waitFor(() => expect(pageHeadword()).toBe("【apple】"));
  });

  it("still looks words up when History cannot be read or written", async () => {
    mockIPC((command, payload) => {
      if (command === "history" || command === "record_lookup") throw new Error("History file damaged");
      if (command === "library") return { folders: [], dictionaries: [{ id: "a", title: "A", path: "/a.mdx", entryCount: 1, size: 1, enabled: true }], failures: [], problem: null };
      if (command === "suggest") return { items: [{ headword: "apple", count: 1 }], lemma: null, near: false, miss: false, browse: false };
      if (command === "look_up") {
        const headword = (payload as { headword: string }).headword;
        return [{ dictionaryId: "a", dictionaryTitle: "A", src: `http://odentry.localhost/a/?headword=${headword}`, allowNetwork: false }];
      }
      return [];
    });
    const field = renderApp();

    await lookUp(field, "apple");

    expect(pageHeadword()).toBe("【apple】");
  });
});

describe("Back and Forward", () => {
  it("are unavailable until the reader has moved between two words", async () => {
    fakeShell();
    const field = renderApp();
    expect(back().disabled).toBe(true);
    expect(forward().disabled).toBe(true);

    await lookUp(field, "apple");
    expect(back().disabled).toBe(true);

    await lookUp(field, "serene");
    expect(back().disabled).toBe(false);
    expect(forward().disabled).toBe(true);
  });

  it("move through the words visited, with the buttons and with Alt+← / Alt+→", async () => {
    const shell = fakeShell();
    const field = renderApp();
    await lookUp(field, "apple");
    await lookUp(field, "sea");
    await lookUp(field, "serene");

    await userEvent.click(back());
    await waitFor(() => expect(pageHeadword()).toBe("【sea】"));
    expect(field.value).toBe("sea");

    fireEvent.keyDown(field, { key: "ArrowLeft", altKey: true });
    await waitFor(() => expect(pageHeadword()).toBe("【apple】"));
    expect(back().disabled).toBe(true);

    fireEvent.keyDown(document.body, { key: "ArrowRight", altKey: true });
    await waitFor(() => expect(pageHeadword()).toBe("【sea】"));
    await userEvent.click(forward());
    await waitFor(() => expect(pageHeadword()).toBe("【serene】"));
    expect(forward().disabled).toBe(true);

    // Retracing steps is not a new Lookup: History keeps its order.
    expect(shell.history()).toEqual(["serene", "sea", "apple"]);
  });

  it("drop the words ahead when the reader looks up a new word from an earlier one", async () => {
    fakeShell();
    const field = renderApp();
    await lookUp(field, "apple");
    await lookUp(field, "sea");
    await userEvent.click(back());
    await waitFor(() => expect(pageHeadword()).toBe("【apple】"));

    await lookUp(field, "serendipity");

    expect(forward().disabled).toBe(true);
    await userEvent.click(back());
    await waitFor(() => expect(pageHeadword()).toBe("【apple】"));
  });
});
