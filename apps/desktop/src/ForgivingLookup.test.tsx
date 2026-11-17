import { describe, expect, it } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import { PAGE_SIZE } from "./lib/useHeadwordColumn";
import type { Suggestions } from "./lib/ipc";

const answer = (fields: Partial<Suggestions>): Suggestions => ({
  items: [],
  lemma: null,
  near: false,
  miss: false,
  browse: false,
  ...fields,
});

/** A fake shell that answers suggest from a table of canned answers, as the core would. */
function fakeShell(answers: Record<string, Suggestions>) {
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ command, args });
    switch (command) {
      case "library":
        return {
          folders: [{ path: "/library", dictionaryCount: 1, problem: null }],
          dictionaries: [{ id: "a", title: "A", path: "/library/a.mdx", entryCount: 3, size: 1, enabled: true, index: { state: "ready" } }],
          failures: [],
          indexing: null,
          problem: null,
        };
      case "suggest":
        return answers[(args.query as string).trim()] ?? answer({ browse: true });
      case "headwords_after":
        return [];
      case "look_up":
        return [{ dictionaryId: "a", dictionaryTitle: "A", src: "about:blank", allowNetwork: false }];
      default:
        throw new Error(`unexpected command ${command}`);
    }
  }, { shouldMockEvents: true });
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

const column = () => screen.getByRole("complementary", { name: "Headword column" });
const page = () => screen.getByRole("main", { name: "Entries" });
const pageHeadword = () => within(page()).getByRole("heading", { level: 1 }).textContent;

describe("forgiving lookup", () => {
  it("offers close spellings for a typo, says so, and shows the closest", async () => {
    const calls = fakeShell({
      serendipty: answer({ items: [{ headword: "serendipity", count: 2 }], near: true }),
    });
    const search = renderApp();

    await userEvent.type(search, "serendipty");

    expect(await within(column()).findByText("No exact match. Close headwords:")).toBeTruthy();
    await waitFor(() => expect(pageHeadword()).toBe("【serendipity】"));
    // Close spellings are a short list of their own: the column does not continue it.
    await userEvent.keyboard("{ArrowDown}");
    expect(calls.filter((call) => call.command === "headwords_after")).toEqual([]);
  });

  it("says plainly when nothing is found, suggests a wildcard and lists the headwords that follow", async () => {
    fakeShell({
      sex: answer({ items: [{ headword: "shadow", count: 1 }, { headword: "shore", count: 1 }], miss: true }),
      "se*": answer({ items: [{ headword: "sea", count: 1 }] }),
    });
    const search = renderApp();

    await userEvent.type(search, "sex");

    await waitFor(() => expect(pageHeadword()).toBe("【sex】"));
    expect(within(page()).getByText("Nothing found for “sex”.")).toBeTruthy();
    expect(within(page()).getByText(/Use \* for any run of letters/)).toBeTruthy();
    const next = within(page()).getByRole("list", { name: "Next in the headword index" });
    expect(within(next).getAllByRole("button").map((button) => button.textContent)).toEqual(["shadow", "shore"]);

    await userEvent.click(within(page()).getByRole("button", { name: "Try se*" }));

    expect(search.value).toBe("se*");
    await waitFor(() => expect(pageHeadword()).toBe("【sea】"));
  });

  it("looks up a headword the reader picks from the ones that follow a miss", async () => {
    fakeShell({ sex: answer({ items: [{ headword: "shadow", count: 1 }], miss: true }) });
    const search = renderApp();
    await userEvent.type(search, "sex");

    await userEvent.click(await within(page()).findByRole("button", { name: "shadow" }));

    await waitFor(() => expect(pageHeadword()).toBe("【shadow】"));
    expect(search.value).toBe("shadow");
  });

  it("lists nothing to follow when no headword follows where the word would stand", async () => {
    fakeShell({ zzz: answer({ miss: true }) });
    const search = renderApp();

    await userEvent.type(search, "zzz");

    expect(await within(page()).findByText("Nothing found for “zzz”.")).toBeTruthy();
    expect(within(page()).queryByRole("list", { name: "Next in the headword index" })).toBeNull();
    expect(within(page()).getByRole("button", { name: "Try zz*" })).toBeTruthy();
  });

  it("continues a column with what the query offers, so a pattern's column stays its matches", async () => {
    const items = Array.from({ length: PAGE_SIZE }, (_, i) => ({ headword: `s${String(i).padStart(3, "0")}s`, count: 1 }));
    const calls = fakeShell({ "s*s": answer({ items }) });
    const search = renderApp();
    await userEvent.type(search, "s*s");
    await waitFor(() => expect(pageHeadword()).toBe(`【${items[0]!.headword}】`));

    // Moving past the last row loads more.
    await userEvent.keyboard(`{ArrowDown>${PAGE_SIZE}/}`);

    await waitFor(() =>
      expect(calls).toContainEqual({
        command: "headwords_after",
        args: { query: "s*s", headword: items.at(-1)!.headword, limit: PAGE_SIZE },
      }),
    );
  }, 15_000);
});
