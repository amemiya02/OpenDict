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
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { chromium, type Browser, type Frame, type Page } from "playwright-core";

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

  // A free port of our own: other copies of the app may be under test at the same time.
  const port = await freePort();
  app = spawn(appPath, [], {
    env: {
      ...process.env,
      OPENDICT_LIBRARY_FOLDER: library,
      OPENDICT_DATA_DIR: join(scratch, "data"),
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
  const context = browser.contexts()[0]!;
  page = context.pages()[0] ?? (await context.waitForEvent("page"));
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

async function freePort(): Promise<number> {
  const probe = createServer();
  await new Promise<void>((done) => probe.listen(0, "127.0.0.1", done));
  const { port } = probe.address() as { port: number };
  await new Promise((done) => probe.close(done));
  return port;
}

async function lookUp(headword: string) {
  const search = page.getByRole("searchbox");
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

  const networkSwitch = page.getByRole("region", { name: "Networked Probe", exact: true }).locator("button[aria-pressed]");
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
  const page = browser!.contexts()[0]!.pages()[0]!;
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

  const search = page.getByRole("searchbox");
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
