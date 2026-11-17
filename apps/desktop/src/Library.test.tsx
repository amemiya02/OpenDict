import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { act } from "react";
import { App } from "./App";
import { I18nProvider, type Locale } from "./i18n";
import type { Dictionary, Library } from "./lib/ipc";

const english: Dictionary = {
  id: "en01",
  title: "Sample English Dictionary",
  path: "D:\\Dictionaries\\english.mdx",
  entryCount: 4120,
  size: 38 * 1024 * 1024,
  enabled: true,
  index: { state: "ready" },
};
const japanese: Dictionary = {
  id: "ja02",
  title: "サンプル国語辞典",
  path: "D:\\Dictionaries\\日本語\\japanese.mdx",
  entryCount: 2740,
  size: 54 * 1024 * 1024,
  enabled: true,
  index: { state: "ready" },
};

const empty: Library = { folders: [], dictionaries: [], failures: [], indexing: null, problem: null };
const stocked: Library = {
  folders: [{ path: "D:\\Dictionaries", dictionaryCount: 3, problem: null }],
  dictionaries: [english, japanese],
  failures: [{ path: "D:\\Dictionaries\\notes.mdx", reason: { kind: "notADictionary" } }],
  indexing: null,
  problem: null,
};

/** A fake shell: keeps a Library, records every command, and answers like the real one. */
function fakeShell(initial: Library, { picked = stocked }: { picked?: Library | null } = {}) {
  let library = initial;
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ command, args });
    switch (command) {
      case "library":
        return library;
      case "look_up":
        return [];
      case "add_library_folder":
        if (picked) library = picked;
        return picked;
      case "rescan_library":
        return library;
      case "index_library":
        return library;
      case "cancel_indexing":
        return null;
      case "remove_library_folder":
        library = empty;
        return library;
      case "set_dictionary_enabled":
        library = {
          ...library,
          dictionaries: library.dictionaries.map((d) => (d.id === args.id ? { ...d, enabled: args.enabled as boolean } : d)),
        };
        return library;
      case "move_dictionary": {
        const list = [...library.dictionaries];
        const from = list.findIndex((d) => d.id === args.id);
        const to = from + (args.direction === "up" ? -1 : 1);
        [list[from], list[to]] = [list[to]!, list[from]!];
        library = { ...library, dictionaries: list };
        return library;
      }
    }
    throw new Error(`unexpected command ${command}`);
  }, { shouldMockEvents: true });
  return calls;
}

function renderApp(locale: Locale = "en") {
  render(
    <I18nProvider locale={locale}>
      <App />
    </I18nProvider>,
  );
}

async function openLibrary() {
  await userEvent.click(await screen.findByRole("button", { name: "Library" }));
  return screen.findByRole("heading", { level: 1, name: "【Library】" });
}

describe("Library view", () => {
  it("tells the reader of an empty Library how to add a Library Folder", async () => {
    const calls = fakeShell(empty);
    renderApp();

    expect(await screen.findByText(/No dictionaries yet/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Add folder…" }));

    expect(calls.map((c) => c.command)).toContain("add_library_folder");
    expect(await screen.findByRole("heading", { level: 1, name: "【Library】" })).toBeTruthy();
    expect(screen.getByRole("rowheader", { name: /Sample English Dictionary/ })).toBeTruthy();
  });

  it("lists each Dictionary with its name, Entry count and size, in Library order", async () => {
    fakeShell(stocked);
    renderApp();
    await openLibrary();

    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getByRole("rowheader").textContent)).toEqual([
      "SSample English Dictionary",
      "ササンプル国語辞典",
    ]);
    expect(rows[0]!.textContent).toContain("4,120");
    expect(rows[0]!.textContent).toContain("38 MB");
    expect(screen.getByText("D:\\Dictionaries")).toBeTruthy();
    expect(screen.getByText("3 dictionaries found")).toBeTruthy();
  });

  it("enables and disables a Dictionary with its switch", async () => {
    const calls = fakeShell(stocked);
    renderApp();
    await openLibrary();

    const toggle = screen.getByRole("switch", { name: "Enable Sample English Dictionary" });
    expect(toggle).toHaveProperty("checked", true);
    await userEvent.click(toggle);

    expect(calls.at(-1)).toEqual({ command: "set_dictionary_enabled", args: { id: "en01", enabled: false } });
    expect(await screen.findByRole("switch", { name: "Enable Sample English Dictionary", checked: false })).toBeTruthy();
    expect(screen.getByText("1 dictionary")).toBeTruthy();
  });

  it("moves a Dictionary up and down; the ends cannot move further", async () => {
    const calls = fakeShell(stocked);
    renderApp();
    await openLibrary();

    expect(screen.getByRole("button", { name: "Move up: Sample English Dictionary" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Move down: サンプル国語辞典" })).toHaveProperty("disabled", true);
    await userEvent.click(screen.getByRole("button", { name: "Move up: サンプル国語辞典" }));

    expect(calls.at(-1)).toEqual({ command: "move_dictionary", args: { id: "ja02", direction: "up" } });
    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(within(rows[0]!).getByRole("rowheader").textContent).toContain("サンプル国語辞典");
  });

  it("rescans and removes Library Folders", async () => {
    const calls = fakeShell(stocked);
    renderApp();
    await openLibrary();

    await userEvent.click(screen.getByRole("button", { name: "Rescan" }));
    expect(calls.at(-1)?.command).toBe("rescan_library");

    await userEvent.click(screen.getByRole("button", { name: "Remove D:\\Dictionaries" }));
    expect(calls.at(-1)).toEqual({ command: "remove_library_folder", args: { path: "D:\\Dictionaries" } });
    expect(await screen.findByText(/No dictionaries yet/)).toBeTruthy();
  });

  it("lists Dictionaries that failed to open with a plain reason", async () => {
    fakeShell(stocked);
    renderApp("ja");
    await userEvent.click(await screen.findByRole("button", { name: "ライブラリ" }));

    const failure = await screen.findByText("D:\\Dictionaries\\notes.mdx");
    expect(failure.closest("li")?.textContent).toContain("辞書ファイルではありません");
  });

  it("shows indexing progress as the shell reports it, and cancels it", async () => {
    const calls = fakeShell(stocked);
    renderApp();
    await openLibrary();

    const indexing: Library = {
      ...stocked,
      dictionaries: [english, { ...japanese, index: { state: "pending" } }],
      indexing: { dictionaryId: "ja02", title: "サンプル国語辞典", done: 685, total: 2740, waiting: 0 },
    };
    await act(() => emit("library", indexing));

    const progress = await screen.findByRole("progressbar", { name: "Indexing サンプル国語辞典" });
    expect(progress.getAttribute("value")).toBe("685");
    expect(progress.getAttribute("max")).toBe("2740");
    expect(screen.getByText(/25%/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Stop indexing" }));
    expect(calls.at(-1)?.command).toBe("cancel_indexing");
  });

  it("says which Dictionaries are not indexed and offers to index them", async () => {
    const calls = fakeShell({
      ...stocked,
      dictionaries: [
        { ...english, index: { state: "failed", detail: "the file is damaged" } },
        { ...japanese, index: { state: "pending" } },
      ],
    });
    renderApp();
    await openLibrary();

    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(rows[0]!.textContent).toContain("Could not be indexed: the file is damaged");
    expect(rows[1]!.textContent).toContain("Not indexed yet");
    await userEvent.click(screen.getByRole("button", { name: "Index now" }));
    expect(calls.at(-1)?.command).toBe("index_library");
  });

  it("a cancelled folder dialog changes nothing", async () => {
    fakeShell(empty, { picked: null });
    renderApp();
    await openLibrary();

    await userEvent.click(screen.getByRole("button", { name: "Add folder…" }));

    expect(await screen.findByText(/No dictionaries yet/)).toBeTruthy();
  });
});
