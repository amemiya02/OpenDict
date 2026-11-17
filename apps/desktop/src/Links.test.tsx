import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "./App";
import { I18nProvider } from "./i18n";

const base = (key: string) => `http://odentry.localhost/${key}/`;

function entry(title: string, key: string, headword: string, pronunciation: string | null) {
  return { dictionaryId: `${title}.mdx`, dictionaryTitle: title, src: `${base(key)}?headword=${headword}`, allowNetwork: false, pronunciation };
}

const answers: Record<string, ReturnType<typeof entry>[]> = {
  serendipity: [entry("Quiet", "71", "serendipity", null), entry("Spoken", "73", "serendipity", "uk/serendipity.mp3")],
  serendipitous: [entry("Quiet", "71", "serendipitous", null)],
};

let lookedUp: string[];
let played: string[];

beforeEach(() => {
  lookedUp = [];
  played = [];
  mockIPC((command, args) => {
    if (command === "library")
      return { folders: [], dictionaries: [{ id: "q", title: "Quiet", path: "/q.mdx", entryCount: 2, size: 1, enabled: true }], failures: [], problem: null };
    if (command === "look_up") {
      const { headword } = args as { headword: string };
      lookedUp.push(headword);
      return answers[headword] ?? [];
    }
    if (command === "suggest") {
      const query = (args as { query: string }).query.trim();
      const items = Object.keys(answers)
        .filter((headword) => headword.startsWith(query))
        .map((headword) => ({ headword, count: answers[headword]!.length }));
      return { items, lemma: null, near: false, miss: items.length === 0, browse: !query };
    }
    if (command === "headwords_after") return [];
    throw new Error(`unexpected command ${command}`);
  });
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(function (this: HTMLMediaElement) {
    played.push(this.src);
    return Promise.resolve();
  });
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

async function lookUpSerendipity() {
  render(
    <I18nProvider locale="en">
      <App />
    </I18nProvider>,
  );
  await userEvent.type(screen.getByRole("combobox", { name: "Type a word" }), "serendipity{Enter}");
  await screen.findByRole("heading", { level: 1, name: "【serendipity】" });
  await screen.findByTitle("Entry from Spoken");
  lookedUp.length = 0; // what typing looked up along the way
}

function frame(title: string) {
  return screen.getByTitle(`Entry from ${title}`) as HTMLIFrameElement;
}

/** The reader clicks into a frame: focus moves to it. */
function clickInto(title: string) {
  act(() => frame(title).focus());
  return frame(title).contentWindow;
}

function post(data: unknown, source: MessageEventSource | null) {
  act(() => {
    window.dispatchEvent(new MessageEvent("message", { data, source }));
  });
}

describe("links inside Entries", () => {
  it("an entry:// link looks the linked headword up", async () => {
    await lookUpSerendipity();

    post({ opendict: "link", href: "entry://serendipitous" }, clickInto("Quiet"));

    expect(await screen.findByRole("heading", { level: 1, name: "【serendipitous】" })).toBeTruthy();
    expect(lookedUp).toEqual(["serendipitous"]);
    expect(screen.getByRole<HTMLInputElement>("combobox", { name: "Type a word" }).value).toBe("serendipitous");
  });

  it("ignores link messages that are not a frame's own, or are not links", async () => {
    await lookUpSerendipity();
    clickInto("Quiet");

    post({ opendict: "link", href: "entry://serendipitous" }, window);
    post({ opendict: "link", href: "entry://serendipitous" }, null);
    post({ opendict: "link", href: 42 }, frame("Quiet").contentWindow);
    post({ opendict: "link", href: "https://example.com/" }, frame("Quiet").contentWindow);
    post({ opendict: "look_up", headword: "serendipitous" }, frame("Quiet").contentWindow);
    post("entry://serendipitous", frame("Quiet").contentWindow);

    expect(lookedUp).toEqual([]);
  });

  it("ignores links a script sends without the reader clicking in its frame", async () => {
    await lookUpSerendipity();

    // Focus is still in the search field: the reader has not clicked into any Entry.
    post({ opendict: "link", href: "entry://serendipitous" }, frame("Quiet").contentWindow);
    // The reader clicked into another Dictionary's Entry.
    clickInto("Spoken");
    post({ opendict: "sound", href: "sound://a.mp3" }, frame("Quiet").contentWindow);
    // The reader clicked into this one, but not just now.
    vi.stubGlobal("navigator", { ...navigator, userActivation: { isActive: false, hasBeenActive: true } });
    post({ opendict: "sound", href: "sound://a.mp3" }, frame("Spoken").contentWindow);

    expect(lookedUp).toEqual([]);
    expect(played).toEqual([]);
  });
});

describe("audio", () => {
  it("a sound:// link plays the file from its own Dictionary", async () => {
    await lookUpSerendipity();

    post({ opendict: "sound", href: "sound://us/serendipity.mp3" }, clickInto("Spoken"));
    post({ opendict: "sound", href: "sound://a.mp3" }, clickInto("Quiet"));

    expect(played).toEqual([`${base("73")}us/serendipity.mp3`, `${base("71")}a.mp3`]);
  });

  it("a sound:// link cannot reach another Dictionary's files", async () => {
    await lookUpSerendipity();

    post({ opendict: "sound", href: "sound://../71/a.mp3" }, clickInto("Spoken"));
    post({ opendict: "sound", href: "sound://%2e%2e/71/a.mp3" }, frame("Spoken").contentWindow);

    expect(played).toEqual([]);
  });

  it("Pronounce plays the first audio the Entries offer, from the button or Alt+P", async () => {
    await lookUpSerendipity();

    await userEvent.click(screen.getByRole("button", { name: /Pronounce/ }));
    await userEvent.keyboard("{Alt>}p{/Alt}");

    expect(played).toEqual([`${base("73")}uk/serendipity.mp3`, `${base("73")}uk/serendipity.mp3`]);
  });

  it("offers no Pronounce action when no Entry has audio", async () => {
    await lookUpSerendipity();
    post({ opendict: "link", href: "entry://serendipitous" }, clickInto("Quiet"));
    await screen.findByRole("heading", { level: 1, name: "【serendipitous】" });

    expect(screen.queryByRole("button", { name: /Pronounce/ })).toBeNull();
    await userEvent.keyboard("{Alt>}p{/Alt}");
    expect(played).toEqual([]);
  });
});
