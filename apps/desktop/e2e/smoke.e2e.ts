// End-to-end smoke test against the built app (Windows: WebView2 over the DevTools protocol).
//
//   npx tauri build --debug --no-bundle
//   npm run e2e
//
// It writes a sample Library Folder with the fixture writer, starts the app on it with
// remote debugging enabled, types a headword and checks that the Entry appears inside
// a sandboxed frame the app cannot reach into.

import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { chromium, type Browser } from "playwright-core";

const repo = resolve(import.meta.dirname, "../../..");
const targetDir = process.env.CARGO_TARGET_DIR ?? join(repo, "target");
const appPath = process.env.OPENDICT_APP ?? join(targetDir, "debug", process.platform === "win32" ? "opendict-desktop.exe" : "opendict-desktop");
const port = 9333;

let app: ChildProcess | undefined;
let browser: Browser | undefined;
let library = "";

before(async () => {
  if (process.platform !== "win32") throw new Error("This smoke test drives WebView2 and runs on Windows only.");
  library = mkdtempSync(join(tmpdir(), "opendict-e2e-"));
  execFileSync("cargo", ["run", "-q", "-p", "opendict-mdict-fixture", "--example", "sample_library", "--", library], { cwd: repo, stdio: "inherit" });

  app = spawn(appPath, [], {
    env: {
      ...process.env,
      OPENDICT_LIBRARY_FOLDER: library,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
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
});

test("typing a headword shows its Entry in a sandboxed frame", async () => {
  const context = browser!.contexts()[0]!;
  const page = context.pages()[0] ?? (await context.waitForEvent("page"));
  await page.waitForLoadState();

  // The status line fills in once the Library has been opened.
  await page.waitForFunction(() => Boolean(document.querySelector("footer")?.textContent?.trim()), null, { timeout: 30_000 });
  const search = page.getByRole("searchbox");
  // Key presses sent over DevTools are sometimes dropped when the field was focused by autoFocus
  // before DevTools attached; a real click first makes WebView2 deliver them.
  await search.click();
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
