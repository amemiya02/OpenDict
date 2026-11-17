import { describe, expect, it } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";

const sample = {
  dictionaryId: "sample.mdx",
  dictionaryTitle: "Sample Dictionary",
  src: "http://odentry.localhost/73616d706c652e6d6478/?headword=serendipity",
  allowNetwork: false,
  sound: null,
  pronunciation: null,
};

function fakeShell(calls: { command: string; args: unknown }[] = []) {
  mockIPC((command, args) => {
    calls.push({ command, args });
    if (command === "library")
      return {
        folders: [{ path: "/library", dictionaryCount: 1, problem: null }],
        dictionaries: [{ id: "sample", title: "Sample Dictionary", path: "/library/sample.mdx", entryCount: 1, size: 2048, enabled: true, index: { state: "ready" } }],
        indexing: null,
        failures: [],
        problem: null,
      };
    if (command === "look_up") return (args as { headword: string }).headword === "serendipity" ? [sample] : [];
    if (command === "suggest") {
      const query = (args as { query: string }).query.trim();
      const matches = "serendipity".startsWith(query);
      return { items: matches ? [{ headword: "serendipity", count: 1 }] : [], lemma: null, near: false, miss: !matches, browse: !query };
    }
    if (command === "headwords_after") return [];
    if (command === "set_allow_network") return null;
    throw new Error(`unexpected command ${command}`);
  }, { shouldMockEvents: true });
}

async function lookUpSerendipity() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  await userEvent.type(screen.getByRole("combobox", { name: "Type a word" }), "serendipity{Enter}");
  return (await screen.findByTitle("Entry from Sample Dictionary")) as HTMLIFrameElement;
}

describe("main window", () => {
  it("Enter takes the typed word when the suggestions still answer an earlier query", async () => {
    const lookedUp: string[] = [];
    mockIPC((command, args) => {
      if (command === "library")
        return { folders: [], dictionaries: [], indexing: null, failures: [], problem: null };
      if (command === "look_up") {
        const { headword } = args as { headword: string };
        lookedUp.push(headword);
        return [{ ...sample, src: `${sample.src}&for=${headword}` }];
      }
      if (command === "suggest") {
        const query = (args as { query: string }).query.trim();
        // The shell is slow to answer the whole word; earlier queries offered "serene".
        if (query === "serendipity") return new Promise(() => {});
        return { items: [{ headword: "serene", count: 1 }], lemma: null, near: false, miss: false, browse: !query };
      }
      if (command === "headwords_after") return [];
      throw new Error(`unexpected command ${command}`);
    }, { shouldMockEvents: true });
    render(
      <I18nProvider locale="en">
        <App />
      </I18nProvider>,
    );

    await userEvent.type(screen.getByRole("combobox", { name: "Type a word" }), "serendipity{Enter}");

    expect(await screen.findByRole("heading", { level: 1, name: "【serendipity】" })).toBeTruthy();
    expect(lookedUp.at(-1)).toBe("serendipity");
  });

  it("loads the Entry into a frame that may run scripts but stays on an opaque origin", async () => {
    fakeShell();

    const frame = await lookUpSerendipity();

    expect(screen.getByRole("heading", { level: 1 })).toHaveProperty("textContent", "【serendipity】");
    expect(frame.getAttribute("src")).toBe(sample.src);
    expect(frame.getAttribute("sandbox")).toBe("allow-scripts");
    expect(frame.hasAttribute("srcdoc")).toBe(false);
  });

  it("sizes the frame to the height its own document reports, ignoring other senders", async () => {
    fakeShell();
    const frame = await lookUpSerendipity();

    act(() => {
      window.dispatchEvent(new MessageEvent("message", { data: { opendict: "size", height: 999 }, source: window }));
    });
    expect(frame.style.height).not.toBe("999px");

    act(() => {
      window.dispatchEvent(new MessageEvent("message", { data: { opendict: "size", height: 321 }, source: frame.contentWindow }));
    });
    expect(frame.style.height).toBe("321px");
  });

  it("lets the reader allow network access for one Dictionary from its section header", async () => {
    const calls: { command: string; args: unknown }[] = [];
    fakeShell(calls);
    await lookUpSerendipity();

    const toggle = screen.getByRole("button", { name: "Allow network access" });
    expect(toggle.getAttribute("aria-pressed")).toBe("false");
    await userEvent.click(toggle);

    expect(calls).toContainEqual({ command: "set_allow_network", args: { dictionaryId: "sample.mdx", allow: true } });
    expect(toggle.getAttribute("aria-pressed")).toBe("true");
  });

  it("says plainly when nothing is found", async () => {
    fakeShell();
    render(
      <I18nProvider locale="ja">
        <App />
      </I18nProvider>,
    );

    await userEvent.type(screen.getByRole("combobox"), "nonexistent{Enter}");

    expect(await screen.findByText("「nonexistent」は見つかりませんでした。")).toBeTruthy();
  });
});
