import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";

const sample = {
  dictionaryId: "sample",
  dictionaryTitle: "Sample Dictionary",
  html: "<p class='def'>a happy accident (sample entry)</p>",
};

function fakeShell() {
  mockIPC((command, args) => {
    if (command === "library")
      return {
        folders: [{ path: "/library", dictionaryCount: 1, problem: null }],
        dictionaries: [{ id: "sample", title: "Sample Dictionary", path: "/library/sample.mdx", entryCount: 1, size: 2048, enabled: true, index: { state: "ready" } }],
        indexing: null,
        failures: [],
        problem: null,
      };
    if (command === "look_up") return (args as { headword: string }).headword === "serendipity" ? [sample] : [];
    throw new Error(`unexpected command ${command}`);
  }, { shouldMockEvents: true });
}

describe("main window", () => {
  it("shows the Entry in a sandboxed frame when the reader types a headword", async () => {
    fakeShell();
    render(
      <I18nProvider locale="en">
        <App />
      </I18nProvider>,
    );

    await userEvent.type(screen.getByRole("searchbox", { name: "Type a word" }), "serendipity{Enter}");

    const frame = await screen.findByTitle("Entry from Sample Dictionary");
    expect(screen.getByRole("heading", { level: 1 })).toHaveProperty("textContent", "【serendipity】");
    expect(frame.getAttribute("sandbox")).toBe("");
    expect(frame.getAttribute("srcdoc")).toContain("a happy accident (sample entry)");
  });

  it("says plainly when nothing is found", async () => {
    fakeShell();
    render(
      <I18nProvider locale="ja">
        <App />
      </I18nProvider>,
    );

    await userEvent.type(screen.getByRole("searchbox"), "nonexistent{Enter}");

    expect(await screen.findByText("「nonexistent」は見つかりませんでした。")).toBeTruthy();
  });
});
