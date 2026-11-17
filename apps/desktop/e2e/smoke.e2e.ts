// End-to-end tests against the built app (Windows: WebView2 over the DevTools protocol).
//
//   npx tauri build --debug --no-bundle
//   npm run e2e
//
// They write a Library Folder with the fixture writer (the sample Library plus hostile
// "probe" Dictionaries), start the app on it with remote debugging enabled, and drive the
// real window. A local HTTP server stands in for the network: any request that reaches it
// from an Entry that was not allowed the network is a failure.

import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import { createServer, type Server } from "node:http";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { release, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { chromium, type Browser, type Frame, type Page } from "playwright-core";

/** The Mini Window's settings for the run (see src-tauri/src/mini/settings.rs). */
const MINI_SHORTCUTS = { lookUpSelection: "Control+Shift+F9", showMainWindow: "Control+Shift+F10", watchClipboard: false };

const repo = resolve(import.meta.dirname, "../../..");
const targetDir = process.env.CARGO_TARGET_DIR ?? join(repo, "target");
const appPath = process.env.OPENDICT_APP ?? join(targetDir, "debug", process.platform === "win32" ? "opendict-desktop.exe" : "opendict-desktop");

let app: ChildProcess | undefined;
let browser: Browser | undefined;
let sink: Server | undefined;
let page: Page;
let scratch = "";
/** Every request that reached the stand-in network, in order. */
const sinkRequests: string[] = [];

before(async () => {
  if (process.platform !== "win32") throw new Error("These tests drive WebView2 and run on Windows only.");
  scratch = mkdtempSync(join(tmpdir(), "opendict-e2e-"));
  const library = join(scratch, "library");

  sink = createServer((request, response) => {
    sinkRequests.push(request.url ?? "");
    response.writeHead(200, { "Content-Type": "text/plain", "Access-Control-Allow-Origin": "*" }).end("reached");
  });
  await new Promise<void>((done) => sink!.listen(0, "127.0.0.1", done));
  const address = sink.address();
  assert.ok(address && typeof address === "object");
  const sinkUrl = `http://127.0.0.1:${address.port}`;

  const cargo = (example: string, ...args: string[]) =>
    execFileSync("cargo", ["run", "-q", "-p", "opendict-mdict-fixture", "--example", example, "--", ...args], { cwd: repo, stdio: "inherit" });
  cargo("sample_library", library);
  cargo("probe_library", join(library, "probes"), sinkUrl);
  writeFileSync(join(library, "notes.mdx"), "these are notes, not a dictionary");
  // Global shortcuts no other application is likely to hold, without Alt (whose release
  // would open the menu bar of whatever window is in front). They go when the app exits.
  mkdirSync(join(scratch, "data"), { recursive: true });
  writeFileSync(join(scratch, "data", "mini-window.json"), JSON.stringify(MINI_SHORTCUTS));

  // A free port of our own: other copies of the app may be under test at the same time.
  const port = await freePort();
  app = spawn(appPath, [], {
    env: {
      ...process.env,
      OPENDICT_LIBRARY_FOLDER: library,
      OPENDICT_DATA_DIR: join(scratch, "data"),
      // Word Books export here instead of asking where with the system dialog.
      OPENDICT_EXPORT_DIR: join(scratch, "exports"),
      // The folder the reader "chooses" when moving their data, without the system dialog.
      OPENDICT_PICK_DATA_FOLDER: join(scratch, "synced"),
      // A WebView2 browser of our own: instances sharing a user data folder share one
      // browser process, and with it the debugging port of whichever started first.
      WEBVIEW2_USER_DATA_FOLDER: join(scratch, "webview"),
      // English interface, whatever the system language, so the test can find controls by name.
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --lang=en-US`,
    },
    stdio: "inherit",
  });

  const deadline = Date.now() + 60_000;
  while (!browser) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    } catch (error) {
      if (Date.now() > deadline) throw error;
      await new Promise((r) => setTimeout(r, 500));
    }
  }
  // The main window; the Mini Window (mini.html) is the other page.
  page = await pageOf((url) => !url.endsWith("/mini.html"));
  await page.waitForLoadState();
  // The status line fills in once the Library has been opened.
  await page.waitForFunction(() => Boolean(document.querySelector("footer")?.textContent?.trim()), null, { timeout: 30_000 });
});

after(async () => {
  await browser?.close().catch(() => {});
  if (app && app.exitCode === null) {
    const exited = new Promise((done) => app!.once("exit", done));
    app.kill();
    await exited;
  }
  sink?.close();
  // WebView2's browser processes let go of their user data folder a moment after the app exits.
  if (scratch) rmSync(scratch, { recursive: true, force: true, maxRetries: 20, retryDelay: 500 });
});

/** The app's page whose URL passes `wanted`, once it exists. */
async function pageOf(wanted: (url: string) => boolean): Promise<Page> {
  const context = browser!.contexts()[0]!;
  const deadline = Date.now() + 30_000;
  for (;;) {
    const found = context.pages().find((p) => wanted(p.url()));
    if (found) return found;
    if (Date.now() > deadline) throw new Error(`no such page among ${context.pages().map((p) => p.url()).join(", ")}`);
    await new Promise((r) => setTimeout(r, 200));
  }
}

async function freePort(): Promise<number> {
  const probe = createServer();
  await new Promise<void>((done) => probe.listen(0, "127.0.0.1", done));
  const { port } = probe.address() as { port: number };
  await new Promise((done) => probe.close(done));
  return port;
}

async function lookUp(headword: string) {
  const search = page.getByRole("combobox", { name: "Type a word" });
  // Key presses sent over DevTools are sometimes dropped when the field was focused by autoFocus
  // before DevTools attached; a real click first makes WebView2 deliver them.
  await search.click();
  await search.fill(headword);
  await search.press("Enter");
  await page
    .getByRole("heading", { level: 1, name: `【${headword}】` })
    .waitFor({ timeout: 10_000 })
    .catch(async (error: unknown) => {
      throw new Error(`No page for ${headword}. The window says: ${await page.locator("body").innerText()}`, { cause: error });
    });
}

/** The frame holding one Dictionary's Entry, once its document has loaded. */
async function entryFrame(dictionaryTitle: string): Promise<Frame> {
  // By section, named with the Dictionary's own title, not by the frame's translated title.
  const element = page.getByRole("region", { name: dictionaryTitle, exact: true }).locator("iframe");
  await element.waitFor({ timeout: 10_000 }).catch(async (error: unknown) => {
    throw new Error(`No frame for ${dictionaryTitle}. The page says: ${await page.locator("body").innerText()}`, { cause: error });
  });
  const frame = await (await element.elementHandle())!.contentFrame();
  assert.ok(frame, `the frame for ${dictionaryTitle} has a document`);
  await frame.waitForLoadState();
  return frame;
}

test("typing a headword shows its Entry in a sandboxed frame", async () => {
  await lookUp("serendipity");

  const frame = await entryFrame("Sample English Dictionary");
  await frame.getByText("Finding something good without looking for it.").waitFor({ timeout: 10_000 });
  assert.equal(await page.locator("iframe").first().getAttribute("sandbox"), "allow-scripts");
  assert.equal(
    await page.evaluate(() => document.querySelector("iframe")?.contentDocument ?? null),
    null,
    "the app must not be able to reach into the Entry frame (opaque origin)",
  );
  assert.equal(await frame.evaluate(() => document.querySelector("p.def")?.getAttribute("class")), "def", "the Entry keeps its own markup");
});

/** Starts recording every sound the app plays, and whether playback really started. */
async function watchSounds() {
  await page.evaluate(() => {
    const w = window as unknown as { __sounds?: string[] };
    if (w.__sounds) return;
    const sounds: string[] = (w.__sounds = []);
    const play = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function (this: HTMLMediaElement) {
      const started = play.call(this);
      started.then(
        () => sounds.push(`played ${this.src}`),
        (error: unknown) => sounds.push(`refused ${this.src}: ${String(error)}`),
      );
      return started;
    };
  });
}

async function sounds(): Promise<string[]> {
  return page.evaluate(() => [...((window as unknown as { __sounds?: string[] }).__sounds ?? [])]);
}

async function waitForSound(count: number): Promise<string[]> {
  const deadline = Date.now() + 10_000;
  for (;;) {
    const heard = await sounds();
    if (heard.length >= count) return heard;
    if (Date.now() > deadline) assert.fail(`expected ${count} sounds, heard ${JSON.stringify(heard)}`);
    await new Promise((r) => setTimeout(r, 200));
  }
}

test("a redirect chain lands on the real Entry", async () => {
  await lookUp("happy accident");

  const frame = await entryFrame("Sample English Dictionary");
  await frame.getByText("Finding something good without looking for it.").waitFor({ timeout: 10_000 });
});

test("clicking an entry:// link looks the linked headword up; an anchor link stays in the Entry", async () => {
  await lookUp("serendipity");
  let frame = await entryFrame("Sample English Dictionary");

  await frame.getByRole("link", { name: "top" }).click();
  await new Promise((r) => setTimeout(r, 500));
  assert.equal(await page.getByRole("heading", { level: 1 }).innerText(), "【serendipity】");

  await frame.getByRole("link", { name: "serendipitous" }).click();
  await page.getByRole("heading", { level: 1, name: "【serendipitous】" }).waitFor({ timeout: 10_000 });
  frame = await entryFrame("Sample English Dictionary");
  await frame.getByText("Found by happy accident.").waitFor({ timeout: 10_000 });

  await frame.getByRole("link", { name: "serendipity" }).click();
  await page.getByRole("heading", { level: 1, name: "【serendipity】" }).waitFor({ timeout: 10_000 });
});

test("a sound:// link plays the Dictionary's audio, and so do Pronounce and Alt+P", async () => {
  await watchSounds();
  await lookUp("serendipity");
  const before = (await sounds()).length;
  const frame = await entryFrame("Sample English Dictionary");
  const key = new URL(frame.url()).pathname.split("/")[1];
  const expected = `played http://odentry.localhost/${key}/serendipity.wav`;

  await frame.getByRole("link", { name: "Listen" }).click();
  assert.equal((await waitForSound(before + 1))[before], expected);

  await page.getByRole("button", { name: /Pronounce/ }).click();
  assert.equal((await waitForSound(before + 2))[before + 1], expected);

  await page.getByRole("heading", { level: 1 }).click();
  await page.keyboard.press("Alt+KeyP");
  assert.equal((await waitForSound(before + 3))[before + 2], expected);

  // With the focus inside the Entry, the frame reports the keys and the window answers them.
  await frame.locator("body").click({ position: { x: 2, y: 2 } });
  await page.keyboard.press("Alt+KeyP");
  assert.equal((await waitForSound(before + 4))[before + 3], expected);
});

test("an Entry shows the image from its MDD and the stylesheet beside its MDX, and its frame fits it", async () => {
  await lookUp("cat");

  const frame = await entryFrame("Sample Illustrated Dictionary");
  await frame.waitForFunction(() => (document.querySelector("img.pic") as HTMLImageElement | null)?.complete, null, { timeout: 10_000 });
  assert.equal(await frame.evaluate(() => (document.querySelector("img.pic") as HTMLImageElement).naturalWidth), 8, "the MDD image loads");
  assert.equal(
    await frame.evaluate(() => getComputedStyle(document.querySelector(".hw")!).color),
    "rgb(0, 128, 128)",
    "the stylesheet beside the MDX applies",
  );

  const documentHeight = await frame.evaluate(() => Math.ceil(document.documentElement.getBoundingClientRect().height));
  await page.waitForFunction(
    (expected) => Math.abs((document.querySelector('section[aria-label="Sample Illustrated Dictionary"] iframe') as HTMLElement).clientHeight - expected) <= 1,
    documentHeight,
    { timeout: 5_000 },
  );
});

test("an Entry's scripts run but cannot reach the app, stored data, other Dictionaries or the network", async () => {
  await lookUp("probe");

  const frame = await entryFrame("Probe");
  await frame.locator("#probe[data-done]").waitFor({ timeout: 30_000 });
  const results = JSON.parse((await frame.locator("#probe").textContent()) ?? "{}") as Record<string, string>;
  console.log("What the probe Entry could do:", JSON.stringify(results, null, 2));

  // Scripts run, on an opaque origin.
  assert.equal(results.origin, "null");
  // No IPC route answers. (WebView2 injects Tauri's bootstrap into frames, so it is there.)
  for (const route of ["invokeLookUp", "invokeSetAllowNetwork", "fetch http://ipc.localhost/set_allow_network", "fetch ipc://localhost/set_allow_network"]) {
    if (results[route] !== undefined) assert.ok(!results[route]!.startsWith("answered"), `${route}: ${results[route]}`);
  }
  // Not the app's document or storage, nor any of its own.
  for (const route of ["parentDocument", "topDocument", "parentLocalStorage", "localStorage", "cookie"]) {
    assert.ok(results[route]!.startsWith("blocked"), `${route}: ${results[route]}`);
  }
  // Not this or another Dictionary's files by script, and not the network.
  for (const route of ["fetchOwnResource", "fetchOtherDictionary", "fetchSink", "image", "webSocket"]) {
    assert.ok(!results[route]!.startsWith("answered"), `${route}: ${results[route]}`);
  }
  assert.ok(!results.topNavigation!.startsWith("allowed"), `topNavigation: ${results.topNavigation}`);
  // The link and sound it sent without a click were ignored.
  assert.equal(results.unpromptedLink, "sent");
  assert.equal(await page.getByRole("heading", { level: 1 }).innerText(), "【probe】");
  assert.ok(!(await sounds()).some((sound) => sound.includes("networked")), JSON.stringify(await sounds()));

  // Meanwhile "Wandering Probe" navigated its own frame to the network. Whatever the
  // frames tried, nothing reached the network.
  await new Promise((r) => setTimeout(r, 2000));
  assert.deepEqual(sinkRequests, [], "nothing reached the network");
  const wandering = page.getByRole("region", { name: "Wandering Probe", exact: true }).locator("iframe");
  assert.ok(!(await (await wandering.elementHandle())!.contentFrame())!.url().startsWith("http://127.0.0.1"), "the frame did not leave");

  // Nothing a frame tried changed a preference: no probe has network access.
  const entries = await page.evaluate(() =>
    (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<{ dictionaryTitle: string; allowNetwork: boolean }[]> } })
      .__TAURI_INTERNALS__.invoke("look_up", { headword: "probe" }),
  );
  assert.deepEqual(
    entries.map((e) => [e.dictionaryTitle, e.allowNetwork]),
    [["Networked Probe", false], ["Probe", false], ["Wandering Probe", false]],
  );
});

test("a Dictionary reaches the network only once the reader allows it from its section header", async () => {
  await lookUp("probe");
  await entryFrame("Networked Probe");
  await new Promise((r) => setTimeout(r, 1000));
  assert.ok(!sinkRequests.some((url) => url.startsWith("/allowed.png")), "blocked by default");

  const networkSwitch = page.getByRole("region", { name: "Networked Probe", exact: true }).getByRole("button", { name: "Allow network access" });
  assert.equal(await networkSwitch.getAttribute("aria-pressed"), "false");
  await networkSwitch.click();

  const deadline = Date.now() + 10_000;
  while (!sinkRequests.some((url) => url.startsWith("/allowed.png"))) {
    if (Date.now() > deadline) assert.fail(`the allowed Dictionary did not reach the network; requests: ${JSON.stringify(sinkRequests)}`);
    await new Promise((r) => setTimeout(r, 200));
  }
  const frame = await entryFrame("Networked Probe");
  await frame.waitForFunction(() => (document.getElementById("remote") as HTMLImageElement | null)?.complete, null, { timeout: 5_000 });
});

test("the thumb index jumps between the Dictionaries on the page and follows the scrolling", async () => {
  await lookUp("thumb");
  const rail = page.getByRole("navigation", { name: "Jump to dictionary" });
  const notches = rail.getByRole("link");
  const titles = ["Sample English Dictionary", "Sample Illustrated Dictionary", "サンプル国語辞典"];
  assert.deepEqual(
    await notches.evaluateAll((links) => links.map((link) => link.querySelector("span")?.textContent)),
    titles,
    "one notch per answering Dictionary, in page order",
  );
  const head = page.getByRole("heading", { level: 1 }).locator("..");
  assert.match(await head.innerText(), /【thumb】\s*\/θʌm\/\s*in 3 of 6 dictionaries/, "the page head: headword, pronunciation, answers");
  for (const title of titles) await entryFrame(title);
  // Every frame has sized itself to its Entry, each taller than the window.
  await page.waitForFunction(
    () => [...document.querySelectorAll("main iframe")].every((frame) => frame.clientHeight > document.querySelector("main")!.clientHeight),
    null,
    { timeout: 10_000 },
  );
  /** Waits until the section's head is at the top of the scrolling page, give or take its margin. */
  const reaches = (title: string) =>
    page
      .waitForFunction(
        (title) => {
          const main = document.querySelector("main")!;
          const section = document.querySelector(`section[aria-label="${title}"]`)!;
          return Math.abs(section.getBoundingClientRect().top - main.getBoundingClientRect().top) <= 16;
        },
        title,
        { timeout: 5_000 },
      )
      .catch(async (error: unknown) => {
        const at = await page.evaluate(() => document.querySelector("main")!.scrollTop);
        throw new Error(`${title} did not reach the top of the page (scrolled to ${at})`, { cause: error });
      });
  const current = (title: string) => rail.getByRole("link", { name: title, exact: false }).and(rail.locator('[aria-current="true"]')).waitFor({ timeout: 5_000 });
  await current(titles[0]!);

  await notches.nth(1).click();
  await reaches(titles[1]!);
  await current(titles[1]!);
  assert.equal(await page.evaluate(() => document.activeElement?.getAttribute("aria-label")), titles[1], "focus moves to the section");

  await page.keyboard.press("Alt+3");
  await reaches(titles[2]!);
  await current(titles[2]!);
  if (process.env.OPENDICT_E2E_THUMB_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_THUMB_SCREENSHOT });

  // Scrolling by hand back to the top: the first notch is current again.
  await page.locator("main").hover();
  await page.mouse.wheel(0, -100_000);
  await page.waitForFunction(() => document.querySelector("main")!.scrollTop === 0, null, { timeout: 5_000 });
  await current(titles[0]!);
});

test("the Library view lists the Dictionaries, and a disabled one stops answering", async () => {
  const libraryButton = page.getByRole("button", { name: "Library", exact: true });
  await libraryButton.click();
  await page.getByRole("heading", { level: 1, name: "【Library】" }).waitFor({ timeout: 10_000 });

  const names = await page.getByRole("rowheader").allInnerTexts();
  const position = (list: string[], title: string) => list.findIndex((name) => name.includes(title));
  assert.equal(names.length, 6, `six Dictionaries, got ${JSON.stringify(names)}`);
  const japanese = position(names, "サンプル国語辞典");
  assert.ok(position(names, "Sample English Dictionary") < japanese, "in Library order");
  assert.match(await page.getByRole("listitem").filter({ hasText: "notes.mdx" }).innerText(), /Not a dictionary file/);
  if (process.env.OPENDICT_E2E_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_SCREENSHOT });

  await page.getByRole("switch", { name: "Enable Sample English Dictionary" }).click();
  await page.getByRole("switch", { name: "Enable Sample English Dictionary", checked: false }).waitFor({ timeout: 10_000 });
  await page.getByRole("button", { name: "Move up: サンプル国語辞典" }).click();
  await page.waitForFunction(
    (at) => document.querySelectorAll("tbody th")[at]?.textContent?.includes("サンプル国語辞典"),
    japanese - 1,
    { timeout: 10_000 },
  );

  const search = page.getByRole("combobox", { name: "Type a word" });
  await search.fill("serendipity");
  await search.press("Enter");
  await page.getByText("Nothing found for “serendipity”.").waitFor({ timeout: 10_000 });

  await libraryButton.click();
  await page.getByRole("switch", { name: "Enable Sample English Dictionary" }).click();
  await page.getByRole("switch", { name: "Enable Sample English Dictionary", checked: true }).waitFor({ timeout: 10_000 });
  await search.fill("serendipity");
  await search.press("Enter");
  await page.locator("iframe").waitFor({ timeout: 10_000 });
});

test("the headword column suggests as the reader types and the keyboard browses it", async () => {
  const search = page.getByRole("combobox", { name: "Type a word" });
  const column = page.getByRole("listbox", { name: "Headwords" });
  const heading = page.getByRole("heading", { level: 1 });

  // An empty field shows the headword index to browse, guide words above it.
  await search.click();
  await search.fill("");
  await column.getByRole("option", { name: "apple, in 1 dictionary" }).waitFor({ timeout: 10_000 });
  const index = await column.getByRole("option").evaluateAll((options) => options.map((o) => (o as HTMLElement).dataset.headword));
  assert.deepEqual(index, ["apple", "apples", "cat", "happy accident", "probe", "serendipities", "serendipitous", "serendipity", "thumb", "食べる"]);
  assert.equal((await page.getByTestId("guide-words").innerText()).replace(/\s+/g, " "), "apple 食べる");

  // Typing suggests by prefix, shows the first match at once, and the column carries on.
  await search.pressSequentially("app");
  await column.getByRole("option", { name: "apple, in 1 dictionary", selected: true }).waitFor({ timeout: 10_000 });
  await page.waitForFunction(() => document.querySelector("h1")?.textContent === "【apple】", null, { timeout: 10_000 });

  await search.press("ArrowDown");
  await page.waitForFunction(() => document.querySelector("h1")?.textContent === "【apples】", null, { timeout: 10_000 });
  await search.press("ArrowDown");
  await search.press("Enter");
  await page.waitForFunction(() => document.querySelector("h1")?.textContent === "【cat】", null, { timeout: 10_000 });
  assert.equal(await search.inputValue(), "cat");
  assert.equal(await heading.count(), 1);
  if (process.env.OPENDICT_E2E_COLUMN_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_COLUMN_SCREENSHOT });
});

test("in the dark theme Entries are recoloured unless shown as authored; text size and zoom adjust; all are kept", async () => {
  const openReading = async () => {
    await page.getByRole("button", { name: "Library", exact: true }).click();
    const reading = page.getByRole("region", { name: "Reading" });
    await reading.waitFor({ timeout: 10_000 });
    return reading;
  };
  const invoke = <T>(command: string, args: Record<string, unknown> = {}) =>
    page.evaluate(
      ([command, args]) =>
        (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> } }).__TAURI_INTERNALS__.invoke(command, args),
      [command, args] as const,
    ) as Promise<T>;
  const frameFilter = () =>
    page.evaluate(() => getComputedStyle(document.querySelector('section[aria-label="Sample English Dictionary"] iframe')!).filter);

  let reading = await openReading();
  await reading.getByRole("radio", { name: "Dark" }).check();
  await page.waitForFunction(() => document.documentElement.dataset.theme === "dark");
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--paper").trim()), "#111a30", "bookcloth navy");
  // The shell gave the window the theme as well, so its native frame (and Mica) match the page.
  assert.equal(await invoke("plugin:window|theme", { label: "main" }), "dark");
  await reading.getByRole("button", { name: "Larger text" }).click();
  await reading.getByRole("button", { name: "Larger text" }).click();
  await reading.getByText("18 px").waitFor({ timeout: 5_000 });
  if (process.env.OPENDICT_E2E_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_SCREENSHOT.replace(/\.png$/, "-reading-dark.png") });

  await lookUp("serendipity");
  const frame = await entryFrame("Sample English Dictionary");
  assert.match(await frameFilter(), /invert/, "recoloured by default");
  // From outside: the Entry document keeps its own colours, and takes the reader's text size.
  assert.equal(await frame.evaluate(() => getComputedStyle(document.documentElement).fontSize), "18px");
  if (process.env.OPENDICT_E2E_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_SCREENSHOT.replace(/\.png$/, "-dark.png") });

  const section = page.getByRole("region", { name: "Sample English Dictionary", exact: true });
  await section.getByRole("button", { name: "Show as authored" }).click();
  await page.waitForFunction(
    () => getComputedStyle(document.querySelector('section[aria-label="Sample English Dictionary"] iframe')!).filter === "none",
    null,
    { timeout: 5_000 },
  );
  if (process.env.OPENDICT_E2E_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_SCREENSHOT.replace(/\.png$/, "-as-authored.png") });

  // Page zoom: the whole window, Entry frames included.
  const ratio = await page.evaluate(() => devicePixelRatio);
  await page.getByRole("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("Control+Equal");
  await page.waitForFunction((before) => Math.abs(devicePixelRatio / before - 1.1) < 0.02, ratio, { timeout: 5_000 });

  // Everything is kept in the reader's preferences.
  assert.deepEqual(await invoke("appearance"), { theme: "dark", allAsAuthored: false, textSize: 18, zoom: 110, language: null });
  const entries = await invoke<{ dictionaryTitle: string; asAuthored: boolean }[]>("look_up", { headword: "serendipity" });
  assert.equal(entries.find((e) => e.dictionaryTitle === "Sample English Dictionary")?.asAuthored, true);

  // Back as it was, for the tests after this one.
  await page.keyboard.press("Control+Digit0");
  await page.waitForFunction((before) => Math.abs(devicePixelRatio - before) < 0.01, ratio, { timeout: 5_000 });
  await section.getByRole("button", { name: "Show as authored" }).click();
  reading = await openReading();
  await reading.getByRole("button", { name: "Smaller text" }).click();
  await reading.getByRole("button", { name: "Smaller text" }).click();
  await reading.getByRole("radio", { name: "Light" }).check();
  await page.waitForFunction(() => document.documentElement.dataset.theme === "light");
  if (process.env.OPENDICT_E2E_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_SCREENSHOT.replace(/\.png$/, "-reading-light.png") });
  await reading.getByRole("radio", { name: "Follow system" }).check();
  assert.deepEqual(await invoke("appearance"), { theme: "system", allAsAuthored: false, textSize: 16, zoom: 100, language: null });
  await lookUp("serendipity"); // back on the page
});

test("a word saved with Ctrl+D goes into the Word Book, keeps a note and exports to CSV", async () => {
  await lookUp("serendipity");
  await page.getByRole("button", { name: "Add to Word Book", pressed: false }).waitFor({ timeout: 10_000 });

  await page.keyboard.press("Control+d");
  await page.getByRole("button", { name: "In Word Book", pressed: true }).waitFor({ timeout: 10_000 });
  const screenshot = process.env.OPENDICT_E2E_WORDBOOK_SCREENSHOT;
  if (screenshot) await page.screenshot({ path: screenshot.replace(/\.png$/, "-page.png") });

  await page.getByRole("button", { name: "Word Book", exact: true }).click();
  await page.getByRole("heading", { level: 1, name: "【Word Book】" }).waitFor({ timeout: 10_000 });
  const column = page.getByRole("listbox", { name: "Headwords" });
  assert.deepEqual(await column.getByRole("option").evaluateAll((options) => options.map((o) => (o as HTMLElement).dataset.headword)), ["serendipity"]);
  const note = page.getByRole("textbox", { name: "Note for serendipity" });
  await note.fill("Finding a book I was not looking for, in a shop.");
  await note.press("Enter");
  if (screenshot) await page.screenshot({ path: screenshot });

  await page.getByRole("button", { name: "Export CSV…" }).click();
  const file = join(scratch, "exports", "Word Book.csv");
  await page.getByText(`Exported to ${file}`).waitFor({ timeout: 10_000 });
  const [header, row, ...rest] = readFileSync(file, "utf8").split("\n");
  assert.equal(header, "Headword,Note,Dictionary,Added");
  assert.match(row!, /^serendipity,"Finding a book I was not looking for, in a shop\.",Sample English Dictionary,\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
  assert.deepEqual(rest, [""]);
});

test("History lists committed Lookups, and Back and Forward retrace them", async () => {
  const search = page.getByRole("combobox", { name: "Type a word" });
  const column = page.getByRole("listbox", { name: "Headwords" });
  const onPage = (headword: string) =>
    page.waitForFunction((word) => document.querySelector("h1")?.textContent === `【${word}】`, headword, { timeout: 10_000 });

  await lookUp("apple");
  await lookUp("cat");

  await search.press("Alt+ArrowLeft");
  await onPage("apple");
  assert.equal(await search.inputValue(), "apple");
  await page.getByRole("button", { name: "Forward" }).click();
  await onPage("cat");

  await page.getByRole("button", { name: "History" }).click();
  await column.getByRole("option", { name: "cat", exact: true }).waitFor({ timeout: 10_000 });
  const history = await column.getByRole("option").evaluateAll((options) => options.map((o) => (o as HTMLElement).dataset.headword));
  assert.deepEqual(history.slice(0, 2), ["cat", "apple"]);
  if (process.env.OPENDICT_E2E_HISTORY_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_HISTORY_SCREENSHOT });
  await page.getByRole("button", { name: "Suggestions" }).click();
});

test("a Group made in the Library is switched from the cover band, and a miss points back to the whole Library", async () => {
  const page = browser!.contexts()[0]!.pages()[0]!;
  const search = page.getByRole("combobox", { name: "Type a word" });
  const onPage = (headword: string) =>
    page.waitForFunction((word) => document.querySelector("h1")?.textContent === `【${word}】`, headword, { timeout: 10_000 });

  await page.getByRole("button", { name: "Library", exact: true }).click();
  await page.getByRole("button", { name: "New Group…" }).click();
  await page.getByLabel("Group name", { exact: true }).fill("日本語");
  await page.getByLabel("Group name", { exact: true }).press("Enter");
  const tick = page.getByRole("checkbox", { name: "サンプル国語辞典 is in Group 日本語" });
  await tick.click();
  await page.getByRole("checkbox", { name: "サンプル国語辞典 is in Group 日本語", checked: true }).waitFor({ timeout: 10_000 });
  await page.getByRole("list", { name: "日本語" }).waitFor({ timeout: 10_000 });
  if (process.env.OPENDICT_E2E_GROUPS_SCREENSHOT) {
    await page.screenshot({ path: process.env.OPENDICT_E2E_GROUPS_SCREENSHOT.replace(/\.png$/, "-library.png"), fullPage: true });
  }

  const switcher = page.getByRole("combobox", { name: "Current Group" });
  await search.fill("thumb");
  await search.press("Enter");
  await onPage("thumb");
  await switcher.selectOption({ label: "日本語" });
  // The word stays; only the Group's one Dictionary answers now.
  await page.getByText("in 1 of 1 dictionary").waitFor({ timeout: 10_000 });
  await onPage("thumb");
  assert.equal(await page.getByRole("region").count(), 1);

  await search.fill("serendipity");
  const offer = page.getByRole("button", { name: "“serendipity” is in Group “All dictionaries”" });
  await offer.waitFor({ timeout: 10_000 });
  if (process.env.OPENDICT_E2E_GROUPS_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_GROUPS_SCREENSHOT });
  await offer.click();
  // The no-match page has the same heading: wait for the Entries themselves.
  await page.getByRole("region", { name: "Sample English Dictionary" }).waitFor({ timeout: 10_000 });
  await onPage("serendipity");
  assert.equal(await switcher.inputValue(), "");
});

test("the window has the system's own title bar controls, Mica on Windows 11, and stays usable when narrow", async () => {
  // The system draws the title bar and its controls; on Windows 11 Mica shows through the column.
  const decorated = await page.evaluate(() =>
    (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args: object) => Promise<boolean> } }).__TAURI_INTERNALS__.invoke(
      "plugin:window|is_decorated",
      { label: "main" },
    ),
  );
  assert.equal(decorated, true, "native window controls");
  const windows11 = Number(release().split(".")[2]) >= 22000;
  const material = await page.evaluate(() => document.documentElement.dataset.material ?? null);
  assert.equal(material, windows11 ? "mica" : null, "Mica exactly where Windows has it");
  if (windows11) {
    const grounds = await page.evaluate(() => ({
      body: getComputedStyle(document.body).backgroundColor,
      column: getComputedStyle(document.querySelector("aside")!).backgroundColor,
      main: getComputedStyle(document.querySelector("main")!.parentElement!).backgroundColor,
    }));
    assert.equal(grounds.body, "rgba(0, 0, 0, 0)", "the window is clear for Mica");
    assert.match(grounds.column, /color\(srgb [\d. ]+ \/ 0\.65\)|rgba\(.*0\.65\)/, `the column lets Mica through (${grounds.column})`);
    assert.doesNotMatch(grounds.main, /rgba\(0, 0, 0, 0\)|\/ 0\)/, "the page stays solid");
  }

  await page.getByRole("button", { name: "Suggestions", exact: true }).click();
  await lookUp("thumb");
  const cdp = await page.context().newCDPSession(page);
  const narrow = async (width: number) => {
    await cdp.send("Emulation.setDeviceMetricsOverride", { width, height: 700, deviceScaleFactor: 0, mobile: false });
    await page.waitForFunction((width) => window.innerWidth === width, width);
  };
  const box = (selector: string) =>
    page.evaluate((selector) => {
      const element = document.querySelector(selector);
      if (!element) return null;
      const { x, width } = element.getBoundingClientRect();
      return { x, width, display: getComputedStyle(element).display, position: getComputedStyle(element).position };
    }, selector);
  const column = page.getByRole("complementary", { name: "Headword column" });
  const toggle = page.getByRole("button", { name: "Show or hide the headword column" });
  try {
    await narrow(1000);
    assert.equal((await box('nav[aria-label="Jump to dictionary"]'))?.width, 46, "at 1100px and below the thumb index is notches only");
    assert.equal((await box('nav[aria-label="Jump to dictionary"] a span'))?.display, "none", "no captions");
    assert.equal((await box("aside"))?.width, 264);
    assert.equal(await toggle.isVisible(), false, "no toggle while the column fits");

    await narrow(800);
    assert.equal((await box("aside"))?.width, 200, "at 860px and below the column narrows");

    await narrow(600);
    assert.equal(await column.isVisible(), false, "at 620px and below the column leaves the page");
    assert.equal(await toggle.isVisible(), true, "a toggle in the cover band brings it back");
    assert.ok(((await box("main")) ?? { width: 0 }).width > 500, "the page takes the width");
    for (const name of ["Back", "Forward"]) {
      const button = page.getByRole("button", { name, exact: true });
      if ((await button.count()) > 0) assert.equal(await button.isVisible(), true, `${name} stays`);
    }

    await toggle.click();
    await column.waitFor({ state: "visible" });
    const overlay = await box("aside");
    assert.equal(overlay?.position, "fixed", "the column lies over the page");
    assert.equal(overlay?.x, 0);
    if (process.env.OPENDICT_E2E_NARROW_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_NARROW_SCREENSHOT });
    const other = column.getByRole("option").filter({ hasNotText: "thumb" }).first();
    const word = await other.getAttribute("data-headword");
    await other.click();
    await page.getByRole("heading", { level: 1, name: `【${word}】` }).waitFor({ timeout: 10_000 });
    await column.waitFor({ state: "hidden" });

    // Typing brings the column up over the page; Esc puts it away.
    const search = page.getByRole("combobox", { name: "Type a word" });
    await search.click();
    await search.fill("ser");
    await column.waitFor({ state: "visible" });
    await search.press("Escape");
    await column.waitFor({ state: "hidden" });
  } finally {
    await cdp.send("Emulation.clearDeviceMetricsOverride");
    await cdp.detach();
  }
});

/**
 * Presses keys at the operating system's level (as the reader's keyboard does), so that
 * global shortcuts see them: down in order, then up in reverse. Virtual-key codes.
 */
function pressKeys(...keys: number[]) {
  const send = (key: number, up: boolean) => `[OpenDictE2E.Keys]::keybd_event(${key}, 0, ${up ? 2 : 0}, [UIntPtr]::Zero)`;
  const script = [
    `Add-Type -Namespace OpenDictE2E -Name Keys -MemberDefinition '[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);'`,
    ...keys.map((key) => send(key, false)),
    "Start-Sleep -Milliseconds 60",
    ...[...keys].reverse().map((key) => send(key, true)),
  ].join("; ");
  execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script]);
}

/** The text on the system clipboard (empty when it holds none). Reads only. */
function clipboardText(): string {
  return execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", "Get-Clipboard -Raw"], { encoding: "utf8" });
}

const VK = { CONTROL: 0x11, SHIFT: 0x10, F9: 0x78, F10: 0x79 };

async function waitUntil(what: string, check: () => Promise<boolean>) {
  const deadline = Date.now() + 10_000;
  while (!(await check())) {
    if (Date.now() > deadline) assert.fail(`timed out waiting until ${what}`);
    await new Promise((r) => setTimeout(r, 100));
  }
}

test("the global shortcut looks the selection up in the Mini Window, and Esc closes it and gives focus back", async () => {
  const mini = await pageOf((url) => url.endsWith("/mini.html"));
  // WebView2 keeps a hidden window's page "visible"; the window itself says whether it shows.
  const shown = () =>
    mini.evaluate(() =>
      (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args: object) => Promise<boolean> } }).__TAURI_INTERNALS__.invoke(
        "plugin:window|is_visible",
        { label: "mini" },
      ),
    );
  assert.equal(await shown(), false, "the Mini Window waits hidden");

  // The reader selects a word in the main window (standing in for any application) ...
  const search = page.getByRole("combobox", { name: "Type a word" });
  await search.click();
  await search.fill("serendipity");
  // ... which the shortcut for the main window brings to the front first.
  pressKeys(VK.CONTROL, VK.SHIFT, VK.F10);
  await waitUntil("the main window is in front", () => page.evaluate(() => document.hasFocus()));
  await search.selectText();
  const clipboardBefore = clipboardText();

  pressKeys(VK.CONTROL, VK.SHIFT, VK.F9);
  await waitUntil("the Mini Window shows", shown);
  await mini.getByRole("heading", { level: 1, name: "【serendipity】" }).waitFor({ timeout: 10_000 });
  assert.equal(await mini.getByRole("searchbox", { name: "Type a word" }).inputValue(), "serendipity");
  const frame = mini.getByRole("region", { name: "Sample English Dictionary", exact: true }).locator("iframe");
  assert.equal(await frame.getAttribute("sandbox"), "allow-scripts");
  assert.ok((await mini.getByRole("navigation", { name: "Jump to dictionary" }).getByRole("link").count()) >= 1, "a compact thumb index");
  assert.equal(clipboardText(), clipboardBefore, "the clipboard holds what it held before");
  await waitUntil("the Mini Window has the focus", () => mini.evaluate(() => document.hasFocus()));
  if (process.env.OPENDICT_E2E_MINI_SCREENSHOT) await mini.screenshot({ path: process.env.OPENDICT_E2E_MINI_SCREENSHOT });

  await mini.keyboard.press("Escape");
  await waitUntil("the Mini Window is gone", async () => !(await shown()));
  await waitUntil("the main window has the focus again", () => page.evaluate(() => document.hasFocus()));
});

test("the data directory moves into another folder, and the app keeps the reader's data there from then on", async () => {
  const home = join(scratch, "data");
  const synced = join(scratch, "synced");
  await lookUp("apple");
  assert.ok(existsSync(join(home, "history.json")), "History starts in the home folder");

  await page.getByRole("button", { name: "Library", exact: true }).click();
  const settings = page.getByRole("region", { name: "Your data" });
  await settings.getByText(home, { exact: true }).waitFor({ timeout: 10_000 });
  await settings.getByRole("button", { name: "Move…" }).click();
  await settings.getByRole("status").filter({ hasText: `Your data is now in ${synced}.` }).waitFor({ timeout: 30_000 });
  if (process.env.OPENDICT_E2E_DATA_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_DATA_SCREENSHOT });

  for (const name of ["history.json", "word-books.json", "preferences.json"]) {
    assert.ok(existsSync(join(synced, name)), `${name} moved`);
    assert.ok(!existsSync(join(home, name)), `${name} is no longer in the old place`);
  }
  await lookUp("cat");
  // The Lookup is recorded just after its page shows.
  const recorded = () => (JSON.parse(readFileSync(join(synced, "history.json"), "utf8")) as { headwords: string[] }).headwords.slice(0, 2);
  for (let tries = 0; tries < 50 && recorded()[0] !== "cat"; tries++) await new Promise((r) => setTimeout(r, 100));
  assert.deepEqual(recorded(), ["cat", "apple"], "History goes on in the new place");
});

// Runs last: unlocking adds a Dictionary, and with it a headword, to the Library.
test("the command palette switches the theme and the interface language; shortcuts work inside an Entry", async () => {
  const palette = page.getByRole("dialog", { name: "Command palette" });
  const run = async (label: string) => {
    await page.keyboard.press("Control+k");
    await palette.waitFor({ timeout: 5_000 });
    await palette.getByRole("combobox").fill(label);
    await palette.getByRole("option", { name: label, exact: false }).first().waitFor({ timeout: 5_000 });
    await palette.getByRole("combobox").press("Enter");
    await palette.waitFor({ state: "hidden", timeout: 5_000 });
  };
  await lookUp("thumb");

  await run("Theme: dark");
  await page.waitForFunction(() => document.documentElement.dataset.theme === "dark", null, { timeout: 5_000 });
  await page.keyboard.press("Control+k");
  const dark = palette.getByRole("option", { name: "Theme: dark" });
  assert.equal(await dark.getAttribute("aria-checked"), "true", "the palette ticks the theme");
  assert.equal(await palette.getByRole("option", { name: "Interface language: English" }).getAttribute("aria-checked"), "true");
  if (process.env.OPENDICT_E2E_PALETTE_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_PALETTE_SCREENSHOT });
  await page.keyboard.press("Escape");
  await palette.waitFor({ state: "hidden", timeout: 5_000 });
  await run("Theme: follow system");
  await page.waitForFunction(() => document.documentElement.dataset.theme !== "dark" || matchMedia("(prefers-color-scheme: dark)").matches);

  // The interface language switches at once, the page staying as it was.
  await run("Interface language: 日本語");
  await page.getByRole("combobox", { name: "調べたい語を入力" }).waitFor({ timeout: 5_000 });
  assert.equal(await page.evaluate(() => document.documentElement.lang), "ja");
  await page.getByRole("heading", { level: 1, name: "【thumb】" }).waitFor({ timeout: 5_000 });
  await run("表示言語：English");
  await page.getByRole("combobox", { name: "Type a word" }).waitFor({ timeout: 5_000 });

  // `/` focuses the search field from anywhere.
  await page.getByRole("heading", { level: 1 }).click();
  await page.keyboard.press("/");
  assert.equal(await page.evaluate(() => document.activeElement?.getAttribute("role")), "combobox");

  // Keys pressed with the focus inside an Entry reach the window: Alt+2 jumps, Ctrl+K opens the palette.
  const frame = await entryFrame("Sample English Dictionary");
  await frame.locator("body").click({ position: { x: 2, y: 2 } });
  await page.keyboard.press("Alt+2");
  const rail = page.getByRole("navigation", { name: "Jump to dictionary" });
  await rail.getByRole("link", { name: "Sample Illustrated Dictionary", exact: false }).and(rail.locator('[aria-current="true"]')).waitFor({ timeout: 5_000 });
  await frame.locator("body").click({ position: { x: 2, y: 2 } });
  await page.keyboard.press("Control+k");
  await palette.waitFor({ timeout: 5_000 });
  await page.keyboard.press("Escape");
  await palette.waitFor({ state: "hidden", timeout: 5_000 });
});

test("a Dictionary that needs a registration code opens once the reader enters it", async () => {
  await page.getByRole("button", { name: "Library", exact: true }).click();
  const failure = page.getByRole("listitem").filter({ hasText: "sample-bought.mdx" });
  assert.match(await failure.innerText(), /needs its registration code/);

  await failure.getByRole("button", { name: /^Enter the registration code for / }).click();
  await page.getByLabel("E-mail", { exact: true }).fill("reader@example.com");
  await page.getByLabel("Registration code", { exact: true }).fill("FEDCBA9876543210FEDCBA9876543210");
  await page.getByRole("button", { name: "Unlock" }).click();
  await page.getByRole("alert").filter({ hasText: "do not unlock the dictionary" }).waitFor({ timeout: 10_000 });
  if (process.env.OPENDICT_E2E_SCREENSHOT) {
    await page.screenshot({ path: process.env.OPENDICT_E2E_SCREENSHOT.replace(/\.png$/, "-registration.png") });
  }

  await page.getByLabel("Registration code", { exact: true }).fill("0123 4567 89AB CDEF 0123 4567 89AB CDEF");
  await page.getByRole("button", { name: "Unlock" }).click();
  await page.getByRole("rowheader", { name: /Sample Bought Dictionary/ }).waitFor({ timeout: 10_000 });

  const search = page.getByRole("combobox", { name: "Type a word" });
  await search.fill("bargain");
  await search.press("Enter");
  await page.frameLocator("iframe").getByText("Something bought for less than it is worth.").waitFor({ timeout: 10_000 });
});
