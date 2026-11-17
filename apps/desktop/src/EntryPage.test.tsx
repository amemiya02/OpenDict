import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import type { Dictionary, Entry, Library } from "./lib/ipc";

function dictionary(id: string, title: string, enabled = true): Dictionary {
  return { id, title, path: `D:\\Dictionaries\\${id}.mdx`, entryCount: 100, size: 1024, enabled, index: { state: "ready" } };
}

const library: Library = {
  folders: [{ path: "D:\\Dictionaries", dictionaryCount: 4, problem: null }],
  dictionaries: [
    dictionary("learner", "Sample Learner's Dictionary"),
    dictionary("etymology", "Sample Etymology"),
    dictionary("thesaurus", "样例同义词典"),
    dictionary("off", "Switched Off", false),
  ],
  failures: [],
  indexing: null,
  problem: null,
};

function entry(id: string, title: string, pronunciation: string | null = null): Entry {
  return { dictionaryId: id, dictionaryTitle: title, src: `http://odentry.localhost/${id}/?headword=serendipity`, allowNetwork: false, sound: null, pronunciation };
}

/** "serendipity" is in two of the three enabled Dictionaries. */
const answers: Entry[] = [
  entry("learner", "Sample Learner's Dictionary", "/ˌserənˈdɪpəti/"),
  entry("thesaurus", "样例同义词典"),
];

function fakeShell() {
  mockIPC((command, payload) => {
    if (command === "library") return library;
    if (command === "look_up") return (payload as { headword: string }).headword === "serendipity" ? answers : [];
    // The headword column: "serendipity" is the only headword.
    if (command === "suggest") {
      const query = (payload as { query: string }).query.trim();
      const items = "serendipity".startsWith(query) ? [{ headword: "serendipity", count: answers.length }] : [];
      return { items, lemma: null, near: false, miss: items.length === 0 && query !== "", browse: query === "" };
    }
    if (command === "headwords_after") return [];
    throw new Error(`unexpected command ${command}`);
  }, { shouldMockEvents: true });
}

async function lookUp(word: string) {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  await userEvent.type(screen.getByRole("combobox", { name: "Type a word" }), `${word}{Enter}`);
  await screen.findByRole("heading", { level: 1, name: `【${word}】` });
}

describe("the page of Entries", () => {
  it("heads the page with the headword, its pronunciation and how many Dictionaries answered", async () => {
    fakeShell();
    await lookUp("serendipity");

    const head = screen.getByRole("heading", { level: 1 }).parentElement!;
    expect(within(head).getByText("/ˌserənˈdɪpəti/")).toBeTruthy();
    expect(within(head).getByText("in 2 of 3 dictionaries")).toBeTruthy();
  });

  it("gives each answering Dictionary its own section, in Library order", async () => {
    fakeShell();
    await lookUp("serendipity");

    const sections = screen.getAllByRole("region").map((section) => section.getAttribute("aria-label"));
    expect(sections).toEqual(["Sample Learner's Dictionary", "样例同义词典"]);
  });

  it("folds a section away and unfolds it again", async () => {
    fakeShell();
    await lookUp("serendipity");
    const section = screen.getByRole("region", { name: "Sample Learner's Dictionary" });
    const frame = within(section).getByTitle("Entry from Sample Learner's Dictionary");

    await userEvent.click(within(section).getByRole("button", { name: "Collapse" }));
    expect(frame.closest("[hidden]")).not.toBeNull();
    expect(within(section).getByRole("button", { name: "Expand" }).getAttribute("aria-expanded")).toBe("false");
    // The other Dictionary stays open, and the folded Entry keeps its frame.
    expect(screen.getByTitle("Entry from 样例同义词典").closest("[hidden]")).toBeNull();

    await userEvent.click(within(section).getByRole("button", { name: "Expand" }));
    expect(within(section).getByTitle("Entry from Sample Learner's Dictionary")).toBe(frame);
    expect(frame.closest("[hidden]")).toBeNull();
    expect(within(section).getByRole("button", { name: "Collapse" }).getAttribute("aria-expanded")).toBe("true");
  });
});

describe("the thumb index", () => {
  /** jsdom has no layout: record which Dictionary sections the page asked to scroll to instead. */
  function watchScrolling() {
    const scrolledTo: string[] = [];
    Element.prototype.scrollIntoView = function (this: Element) {
      if (this.localName === "section") scrolledTo.push(this.getAttribute("aria-label") ?? this.id);
    };
    return scrolledTo;
  }

  function notches() {
    return within(screen.getByRole("navigation", { name: "Jump to dictionary" })).getAllByRole("link");
  }

  it("has one notch per answering Dictionary, carrying its mark, and says how many had nothing", async () => {
    fakeShell();
    await lookUp("serendipity");

    expect(notches().map((notch) => notch.textContent)).toEqual(["Sample Learner's DictionaryS", "样例同义词典样"]);
    expect(within(screen.getByRole("navigation", { name: "Jump to dictionary" })).getByText("1 without an entry")).toBeTruthy();
  });

  it("jumps to a Dictionary's section when its notch is clicked", async () => {
    const scrolledTo = watchScrolling();
    fakeShell();
    await lookUp("serendipity");

    await userEvent.click(notches()[1]!);

    expect(scrolledTo).toEqual(["样例同义词典"]);
    expect(document.activeElement).toBe(screen.getByRole("region", { name: "样例同义词典" }));
    expect(notches()[1]!.getAttribute("aria-current")).toBe("true");
    expect(notches()[0]!.getAttribute("aria-current")).toBe("false");
  });

  it("marks the Dictionary being read as the reader scrolls", async () => {
    fakeShell();
    await lookUp("serendipity");
    const page = screen.getByRole("main");
    const second = screen.getByRole("region", { name: "样例同义词典" });
    // Lay the page out: it starts 48px down the window; the second section begins 900px into it.
    page.getBoundingClientRect = () => DOMRect.fromRect({ x: 264, y: 48, width: 800, height: 600 });
    Object.defineProperties(page, { clientHeight: { value: 600 }, scrollHeight: { value: 3000 } });
    let scrolled = 0;
    second.getBoundingClientRect = () => DOMRect.fromRect({ x: 298, y: 48 + 900 - scrolled, width: 760, height: 1500 });
    function scrollPage(to: number) {
      scrolled = to;
      page.scrollTop = to;
      fireEvent.scroll(page);
    }
    expect(notches()[0]!.getAttribute("aria-current")).toBe("true");

    scrollPage(700); // the second section's head is still well down the page
    expect(notches()[0]!.getAttribute("aria-current")).toBe("true");

    scrollPage(880); // its head has reached the top
    expect(notches()[1]!.getAttribute("aria-current")).toBe("true");
    expect(notches()[0]!.getAttribute("aria-current")).toBe("false");

    scrollPage(100);
    expect(notches()[0]!.getAttribute("aria-current")).toBe("true");
  });

  it("jumps to the nth Dictionary with Alt and a digit", async () => {
    const scrolledTo = watchScrolling();
    fakeShell();
    await lookUp("serendipity");

    await userEvent.keyboard("{Alt>}2{/Alt}");
    expect(scrolledTo).toEqual(["样例同义词典"]);

    await userEvent.keyboard("{Alt>}1{/Alt}");
    await userEvent.keyboard("{Alt>}9{/Alt}"); // there is no ninth: nothing happens
    expect(scrolledTo).toEqual(["样例同义词典", "Sample Learner's Dictionary"]);
  });
});
