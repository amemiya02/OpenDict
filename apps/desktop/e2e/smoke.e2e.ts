// End-to-end smoke test against the built app (Windows: WebView2 over the DevTools protocol).
//
//   npx tauri build --debug --no-bundle
//   npm run e2e
//
// It writes a sample Library Folder with the fixture writer, starts the app on it with
// remote debugging enabled, types a headword and checks that the Entry appears inside
// a sandboxed frame the app cannot reach into.

import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { chromium, type Browser } from "playwright-core";

const repo = resolve(import.meta.dirname, "../../..");
const targetDir = process.env.CARGO_TARGET_DIR ?? join(repo, "target");
const appPath = process.env.OPENDICT_APP ?? join(targetDir, "debug", process.platform === "win32" ? "opendict-desktop.exe" : "opendict-desktop");
// Override with OPENDICT_E2E_PORT when several worktrees run the test at once.
const port = Number(process.env.OPENDICT_E2E_PORT ?? 9333);

let app: ChildProcess | undefined;
let browser: Browser | undefined;
let library = "";
let webviewProfile = "";

before(async () => {
  if (process.platform !== "win32") throw new Error("This smoke test drives WebView2 and runs on Windows only.");
  library = mkdtempSync(join(tmpdir(), "opendict-e2e-"));
  execFileSync("cargo", ["run", "-q", "-p", "opendict-mdict-fixture", "--example", "sample_library", "--", library], { cwd: repo, stdio: "inherit" });
  writeFileSync(join(library, "notes.mdx"), "these are notes, not a dictionary");
  // A throwaway WebView2 profile: the test never shares the developer's (or another
  // test run's) profile, which WebView2 refuses to open with different arguments.
  webviewProfile = mkdtempSync(join(tmpdir(), "opendict-e2e-webview-"));

  app = spawn(appPath, [], {
    env: {
      ...process.env,
      OPENDICT_LIBRARY_FOLDER: library,
      WEBVIEW2_USER_DATA_FOLDER: webviewProfile,
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
});

after(async () => {
  await browser?.close().catch(() => {});
  app?.kill();
  if (library) rmSync(library, { recursive: true, force: true });
  if (webviewProfile) rmSync(webviewProfile, { recursive: true, force: true, maxRetries: 5, retryDelay: 500 });
});

test("typing a headword shows its Entry in a sandboxed frame", async () => {
  const context = browser!.contexts()[0]!;
  const page = context.pages()[0] ?? (await context.waitForEvent("page"));
  await page.waitForLoadState();

  // The status line fills in once the Library has been opened.
  await page.waitForFunction(() => Boolean(document.querySelector("footer")?.textContent?.trim()), null, { timeout: 30_000 });
  const search = page.getByRole("searchbox");
  await search.fill("serendipity");
  await search.press("Enter");

  const frame = page.locator("iframe");
  await frame.waitFor({ timeout: 10_000 }).catch(async (error: unknown) => {
    throw new Error(`No Entry frame appeared. The page says: ${await page.locator("body").innerText()}`, { cause: error });
  });
  await page.frameLocator("iframe").getByText("Finding something good without looking for it.").waitFor({ timeout: 10_000 });

  assert.equal(await frame.getAttribute("sandbox"), "", "the Entry frame must have no sandbox allowances");
  assert.equal(
    await page.evaluate(() => document.querySelector("iframe")?.contentDocument ?? null),
    null,
    "the app must not be able to reach into the Entry frame (opaque origin)",
  );
  const entryFrame = page.frames().find((f) => f !== page.mainFrame());
  assert.ok(entryFrame);
  assert.equal(
    await entryFrame.evaluate(() => document.querySelector("p.def")?.getAttribute("class")),
    "def",
    "the Entry keeps its own markup",
  );

  // WebView2 injects Tauri's bootstrap into subframes too. The frame runs no scripts, so
  // Entry content cannot use it; this probe (run through DevTools, which ignores the
  // sandbox) checks that even a call made from inside the frame never reaches the app.
  // Timers do not run in a frame without scripts, so the time limit lives out here.
  const probe = await Promise.race([
    entryFrame.evaluate(async () => {
      const internals = (window as unknown as { __TAURI_INTERNALS__?: { invoke: (cmd: string) => Promise<unknown> } }).__TAURI_INTERNALS__;
      if (!internals) return "no Tauri internals";
      return internals.invoke("library_status").then(
        (value) => ({ answered: value }),
        (error: unknown) => `rejected: ${String(error)}`,
      );
    }),
    new Promise<string>((resolve) => setTimeout(() => resolve("no answer within 3 s"), 3000)),
  ]);
  console.log("IPC probe from inside the Entry frame:", JSON.stringify(probe));
  assert.ok(typeof probe === "string", "a call from inside the Entry frame must not reach the app");
});

test("the Library view lists the Dictionaries, and a disabled one stops answering", async () => {
  const page = browser!.contexts()[0]!.pages()[0]!;
  const libraryButton = page.getByRole("button", { name: "Library", exact: true });
  await libraryButton.click();
  await page.getByRole("heading", { level: 1, name: "【Library】" }).waitFor({ timeout: 10_000 });

  const names = await page.getByRole("rowheader").allInnerTexts();
  assert.equal(names.length, 2, `two Dictionaries, got ${JSON.stringify(names)}`);
  assert.ok(names[0]!.includes("Sample English Dictionary") && names[1]!.includes("サンプル国語辞典"), "in Library order");
  assert.match(await page.getByRole("listitem").filter({ hasText: "notes.mdx" }).innerText(), /Not a dictionary file/);
  if (process.env.OPENDICT_E2E_SCREENSHOT) await page.screenshot({ path: process.env.OPENDICT_E2E_SCREENSHOT });

  await page.getByRole("switch", { name: "Enable Sample English Dictionary" }).click();
  await page.getByRole("switch", { name: "Enable Sample English Dictionary", checked: false }).waitFor({ timeout: 10_000 });
  await page.getByRole("button", { name: "Move up: サンプル国語辞典" }).click();
  await page.waitForFunction(() => document.querySelector("tbody th")?.textContent?.includes("サンプル国語辞典"), null, { timeout: 10_000 });

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

test("a Dictionary that needs a registration code opens once the reader enters it", async () => {
  const page = browser!.contexts()[0]!.pages()[0]!;
  await page.getByRole("button", { name: "Library", exact: true }).click();
  const failure = page.getByRole("listitem").filter({ hasText: "sample-bought.mdx" });
  assert.match(await failure.innerText(), /needs its registration code/);

  await failure.getByRole("button", { name: /^Enter the registration code for / }).click();
  await page.getByLabel("E-mail").fill("reader@example.com");
  await page.getByLabel("Registration code").fill("FEDCBA9876543210FEDCBA9876543210");
  await page.getByRole("button", { name: "Unlock" }).click();
  await page.getByRole("alert").filter({ hasText: "do not unlock the dictionary" }).waitFor({ timeout: 10_000 });
  if (process.env.OPENDICT_E2E_SCREENSHOT) {
    await page.screenshot({ path: process.env.OPENDICT_E2E_SCREENSHOT.replace(/\.png$/, "-registration.png") });
  }

  await page.getByLabel("Registration code").fill("0123 4567 89AB CDEF 0123 4567 89AB CDEF");
  await page.getByRole("button", { name: "Unlock" }).click();
  await page.getByRole("rowheader", { name: /Sample Bought Dictionary/ }).waitFor({ timeout: 10_000 });

  const search = page.getByRole("searchbox");
  await search.fill("bargain");
  await search.press("Enter");
  await page.frameLocator("iframe").getByText("Something bought for less than it is worth.").waitFor({ timeout: 10_000 });
});
