import { describe, expect, it } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import type { Dictionary, Group, Library, Suggestions } from "./lib/ipc";

const dictionary = (id: string, title: string): Dictionary => ({
  id,
  title,
  path: `D:\\Dictionaries\\${id}.mdx`,
  entryCount: 10,
  size: 1024,
  enabled: true,
  index: { state: "ready" },
});

/** Which Dictionaries file which headwords. */
const FILED: Record<string, string[]> = {
  apple: ["a", "b", "c"],
  banana: ["b"],
  食べる: ["c"],
};

const answer = (fields: Partial<Suggestions>): Suggestions => ({
  items: [],
  lemma: null,
  near: false,
  miss: false,
  browse: false,
  ...fields,
});

/**
 * A fake shell with three Dictionaries, A, B and C, and two Groups: English (B, A) and
 * 日本語 (C). It keeps the Groups and the Current Group, answers Lookups from the Current
 * Group in its order, and records every command but the column's reads.
 */
function fakeShell({ groups: initialGroups, current = "g1" }: { groups?: Group[]; current?: string | null } = {}) {
  const dictionaries = [dictionary("a", "A"), dictionary("b", "B"), dictionary("c", "C")];
  let groups: Group[] = initialGroups ?? [
    { id: "g1", name: "English", dictionaries: ["b", "a"] },
    { id: "g2", name: "日本語", dictionaries: ["c"] },
  ];
  let currentGroup = current;
  const library = (): Library => ({
    folders: [{ path: "D:\\Dictionaries", dictionaryCount: 3, problem: null }],
    dictionaries,
    failures: [],
    indexing: null,
    problem: null,
    groups,
    currentGroup,
  });
  const answering = () =>
    currentGroup === null ? dictionaries.map((d) => d.id) : groups.find((g) => g.id === currentGroup)!.dictionaries;
  const filed = (word: string) => answering().filter((id) => FILED[word]?.includes(id));
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      switch (command) {
        case "suggest": {
          const query = (args.query as string).trim();
          if (query === "") return answer({ browse: true });
          const words = Object.keys(FILED).filter((word) => word.startsWith(query) && filed(word).length > 0);
          return words.length > 0
            ? answer({ items: words.map((headword) => ({ headword, count: filed(headword).length })) })
            : answer({ miss: true });
        }
        case "headwords_after":
          return [];
        case "history":
          return [];
        case "record_lookup":
          return [];
      }
      calls.push({ command, args });
      switch (command) {
        case "library":
          return library();
        case "look_up":
          return filed(args.headword as string).map((id) => ({
            dictionaryId: id,
            dictionaryTitle: id.toUpperCase(),
            src: "about:blank",
            allowNetwork: false,
            sound: null,
            pronunciation: null,
          }));
        case "elsewhere": {
          const word = args.query as string;
          const other = groups.find((g) => g.id !== currentGroup && g.dictionaries.some((id) => FILED[word]?.includes(id)));
          return other ? { group: other.id, headword: word } : null;
        }
        case "set_current_group":
          currentGroup = args.group as string | null;
          return library();
        case "create_group":
          groups = [...groups, { id: `g${groups.length + 1}`, name: args.name as string, dictionaries: [] }];
          return library();
        case "rename_group":
          groups = groups.map((g) => (g.id === args.id ? { ...g, name: args.name as string } : g));
          return library();
        case "delete_group":
          groups = groups.filter((g) => g.id !== args.id);
          if (currentGroup === args.id) currentGroup = null;
          return library();
        case "set_in_group":
          groups = groups.map((g) =>
            g.id !== args.group
              ? g
              : {
                  ...g,
                  dictionaries: args.member
                    ? [...g.dictionaries, args.dictionary as string]
                    : g.dictionaries.filter((id) => id !== args.dictionary),
                },
          );
          return library();
        case "move_in_group":
          groups = groups.map((g) => {
            if (g.id !== args.group) return g;
            const list = [...g.dictionaries];
            const from = list.indexOf(args.dictionary as string);
            const to = from + (args.direction === "up" ? -1 : 1);
            [list[from], list[to]] = [list[to]!, list[from]!];
            return { ...g, dictionaries: list };
          });
          return library();
      }
      throw new Error(`unexpected command ${command}`);
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
  return screen.getByRole<HTMLInputElement>("combobox", { name: "Type a word" });
}

const page = () => screen.getByRole("main", { name: "Entries" });
const pageHeadword = () => within(page()).queryByRole("heading", { level: 1 })?.textContent;
const sections = () => within(page()).queryAllByRole("region").map((section) => section.getAttribute("aria-label"));
const switcher = () => screen.findByRole<HTMLSelectElement>("combobox", { name: "Current Group" });

describe("Groups", () => {
  it("offers the whole Library and every Group in the cover band, the Current Group chosen", async () => {
    fakeShell();
    renderApp();

    const select = await switcher();

    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "All dictionaries",
      "English",
      "日本語",
    ]);
    await waitFor(() => expect(select.value).toBe("g1"));
  });

  it("switches the Current Group and keeps the word on the page when the new Group has it", async () => {
    const calls = fakeShell();
    const search = renderApp();
    await userEvent.type(search, "apple");
    await waitFor(() => expect(sections()).toEqual(["B", "A"]));
    expect(within(page()).getByText("in 2 of 2 dictionaries")).toBeTruthy();

    await userEvent.selectOptions(await switcher(), "日本語");

    expect(calls.filter((c) => c.command === "set_current_group").at(-1)?.args).toEqual({ group: "g2" });
    await waitFor(() => expect(sections()).toEqual(["C"]));
    expect(pageHeadword()).toBe("【apple】");
    expect(within(page()).getByText("in 1 of 1 dictionary")).toBeTruthy();
    expect(search.value).toBe("apple");

    await userEvent.selectOptions(await switcher(), "All dictionaries");
    expect(calls.filter((c) => c.command === "set_current_group").at(-1)?.args).toEqual({ group: null });
    await waitFor(() => expect(sections()).toEqual(["A", "B", "C"]));
  });

  it("when the new Group lacks the word, says so and points to where it is", async () => {
    fakeShell();
    const search = renderApp();
    await userEvent.type(search, "banana");
    await waitFor(() => expect(sections()).toEqual(["B"]));

    await userEvent.selectOptions(await switcher(), "日本語");

    await waitFor(() => expect(within(page()).getByText("not found")).toBeTruthy());
    expect(pageHeadword()).toBe("【banana】");
    expect(await within(page()).findByRole("button", { name: "“banana” is in Group “English”" })).toBeTruthy();
  });

  it("offers another Group that has the word on the no-match page, and goes there", async () => {
    const calls = fakeShell();
    const search = renderApp();

    await userEvent.type(search, "食べる");
    const offer = await within(page()).findByRole("button", { name: "“食べる” is in Group “日本語”" });
    await userEvent.click(offer);

    expect(calls.filter((c) => c.command === "set_current_group").at(-1)?.args).toEqual({ group: "g2" });
    await waitFor(() => expect(sections()).toEqual(["C"]));
    expect(pageHeadword()).toBe("【食べる】");
    expect((await switcher()).value).toBe("g2");
  });

  it("hides the switcher until the reader makes a Group", async () => {
    fakeShell({ groups: [], current: null });
    renderApp();

    await screen.findByRole("button", { name: "Library" });
    await waitFor(() => expect(screen.queryByRole("combobox", { name: "Current Group" })).toBeNull());
  });
});

describe("Groups in the Library view", () => {
  async function openLibrary() {
    await userEvent.click(await screen.findByRole("button", { name: "Library" }));
    await screen.findByRole("heading", { level: 1, name: "【Library】" });
  }
  const groupList = (name: string) => screen.getByRole("list", { name });

  it("creates a Group and puts Dictionaries in it", async () => {
    const calls = fakeShell({ groups: [], current: null });
    renderApp();
    await openLibrary();

    await userEvent.click(screen.getByRole("button", { name: "New Group…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Group name" }), "Close reading{Enter}");

    expect(calls.at(-1)).toEqual({ command: "create_group", args: { name: "Close reading" } });
    const box = await screen.findByRole("checkbox", { name: "B is in Group Close reading" });
    await userEvent.click(box);
    expect(calls.at(-1)).toEqual({ command: "set_in_group", args: { group: "g1", dictionary: "b", member: true } });
    await waitFor(() =>
      expect(within(groupList("Close reading")).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["BB"]),
    );
    expect(await switcher()).toBeTruthy();
  });

  it("will not create a Group without a name of its own", async () => {
    const calls = fakeShell();
    renderApp();
    await openLibrary();

    await userEvent.click(screen.getByRole("button", { name: "New Group…" }));
    const create = screen.getByRole("button", { name: "Create" });
    expect(create).toHaveProperty("disabled", true);
    await userEvent.type(screen.getByRole("textbox", { name: "Group name" }), "English");

    expect(screen.getByText("Another Group already has this name.")).toBeTruthy();
    expect(create).toHaveProperty("disabled", true);
    expect(calls.map((c) => c.command)).not.toContain("create_group");
  });

  it("keeps each Group's own order", async () => {
    const calls = fakeShell();
    renderApp();
    await openLibrary();

    const english = groupList("English");
    expect(within(english).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["BB", "AA"]);
    expect(within(english).getByRole("button", { name: "Move up in English: B" })).toHaveProperty("disabled", true);
    await userEvent.click(within(english).getByRole("button", { name: "Move up in English: A" }));

    expect(calls.at(-1)).toEqual({ command: "move_in_group", args: { group: "g1", dictionary: "a", direction: "up" } });
    await waitFor(() =>
      expect(within(groupList("English")).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["AA", "BB"]),
    );
  });

  it("renames and deletes a Group", async () => {
    const calls = fakeShell();
    renderApp();
    await openLibrary();

    await userEvent.click(screen.getByRole("button", { name: "Rename English" }));
    const name = screen.getByRole("textbox", { name: "Group name" });
    await userEvent.clear(name);
    await userEvent.type(name, "Close reading{Enter}");
    expect(calls.at(-1)).toEqual({ command: "rename_group", args: { id: "g1", name: "Close reading" } });
    expect(await screen.findByRole("list", { name: "Close reading" })).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Delete Close reading" }));
    expect(screen.getByText("Delete the Group “Close reading”? Its dictionaries stay in the Library.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));

    expect(calls.at(-1)).toEqual({ command: "delete_group", args: { id: "g1" } });
    await waitFor(() => expect(screen.queryByRole("list", { name: "Close reading" })).toBeNull());
    expect((await switcher()).value).toBe("");
  });
});
