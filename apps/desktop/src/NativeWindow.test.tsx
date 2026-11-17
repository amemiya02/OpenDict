import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";

type Chrome = { platform: "windows" | "macos" | "linux"; material: "mica" | "vibrancy" | null };

/** A fake shell with one Dictionary that files "serendipity" and "serene". */
function fakeShell(chrome: Chrome = { platform: "windows", material: null }) {
  const words = ["serendipity", "serene"];
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    switch (command) {
      case "window_chrome":
        return chrome;
      case "library":
        return {
          folders: [{ path: "/library", dictionaryCount: 1, problem: null }],
          dictionaries: [{ id: "a", title: "A", path: "/library/a.mdx", entryCount: 2, size: 1, enabled: true, index: { state: "ready" } }],
          failures: [],
          indexing: null,
          problem: null,
          groups: [],
          currentGroup: null,
        };
      case "suggest": {
        const query = (args.query as string).trim();
        const items = words.filter((word) => word.startsWith(query)).map((headword) => ({ headword, count: 1 }));
        return { items, lemma: null, near: false, miss: items.length === 0, browse: !query };
      }
      case "headwords_after":
        return [];
      case "look_up":
        return words.includes(args.headword as string)
          ? [{ dictionaryId: "a", dictionaryTitle: "A", src: "http://odentry.localhost/a", allowNetwork: false, sound: null, pronunciation: null }]
          : [];
      default:
        throw new Error(`unexpected command ${command}`);
    }
  }, { shouldMockEvents: true });
}

function renderApp() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  return screen.getByRole<HTMLInputElement>("combobox", { name: "Type a word" });
}

const toggle = () => screen.getByRole("button", { name: "Show or hide the headword column" });

describe("narrow windows: the headword column as an overlay", () => {
  it("the cover band's toggle opens and closes the column", async () => {
    fakeShell();
    renderApp();

    expect(toggle()).toHaveProperty("ariaExpanded", "false");
    const controlled = document.getElementById(toggle().getAttribute("aria-controls") ?? "");
    expect(controlled).toBe(screen.getByRole("complementary", { name: "Headword column" }));

    await userEvent.click(toggle());
    expect(toggle()).toHaveProperty("ariaExpanded", "true");
    await userEvent.click(toggle());
    expect(toggle()).toHaveProperty("ariaExpanded", "false");
  });

  it("typing opens the column over the page, and choosing a word puts it away", async () => {
    fakeShell();
    const search = renderApp();

    await userEvent.type(search, "ser");
    expect(toggle()).toHaveProperty("ariaExpanded", "true");

    await userEvent.type(search, "{Enter}");
    await screen.findByRole("heading", { level: 1, name: "【serendipity】" });
    expect(toggle()).toHaveProperty("ariaExpanded", "false");
  });

  it("a word picked from the opened column closes it", async () => {
    fakeShell();
    renderApp();
    await userEvent.click(toggle());

    await userEvent.click(await screen.findByRole("option", { name: /^serene/ }));

    await screen.findByRole("heading", { level: 1, name: "【serene】" });
    expect(toggle()).toHaveProperty("ariaExpanded", "false");
  });

  it("Esc puts the overlay away and leaves the typed text", async () => {
    fakeShell();
    const search = renderApp();
    await userEvent.type(search, "ser");

    await userEvent.keyboard("{Escape}");

    expect(toggle()).toHaveProperty("ariaExpanded", "false");
    expect(search.value).toBe("ser");
  });
});

describe("native chrome", () => {
  it("marks the window with its platform and the material behind it, for the styles", async () => {
    fakeShell({ platform: "windows", material: "mica" });
    renderApp();

    await waitFor(() => expect(document.documentElement.dataset.material).toBe("mica"));
    expect(document.documentElement.dataset.platform).toBe("windows");
  });

  it("on macOS the cover band is the title bar: it drags the window, but the search field does not", async () => {
    fakeShell({ platform: "macos", material: "vibrancy" });
    renderApp();

    await waitFor(() => expect(document.documentElement.dataset.platform).toBe("macos"));
    expect(screen.getByRole("banner").dataset.tauriDragRegion).toBe("deep");
    expect(screen.getByRole("search").dataset.tauriDragRegion).toBe("false");
  });

  it("with no material the window stays solid", async () => {
    fakeShell({ platform: "linux", material: null });
    renderApp();

    await waitFor(() => expect(document.documentElement.dataset.platform).toBe("linux"));
    expect(document.documentElement.dataset.material).toBeUndefined();
    expect(screen.getByRole("banner").dataset.tauriDragRegion).toBeUndefined();
  });
});
