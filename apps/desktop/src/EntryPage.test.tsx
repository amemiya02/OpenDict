import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import type { Dictionary, Entry, Library } from "./lib/ipc";

function dictionary(id: string, title: string, enabled = true): Dictionary {
  return { id, title, path: `D:\\Dictionaries\\${id}.mdx`, entryCount: 100, size: 1024, enabled };
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
  problem: null,
};

function entry(id: string, title: string, pronunciation: string | null = null): Entry {
  return { dictionaryId: id, dictionaryTitle: title, src: `http://odentry.localhost/${id}/?headword=serendipity`, allowNetwork: false, pronunciation };
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
    throw new Error(`unexpected command ${command}`);
  });
}

async function lookUp(word: string) {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  await userEvent.type(screen.getByRole("searchbox"), `${word}{Enter}`);
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
});
