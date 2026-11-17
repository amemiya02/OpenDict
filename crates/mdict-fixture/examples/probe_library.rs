//! Writes a Library Folder of hostile sample Dictionaries for the desktop app's isolation
//! test. Their Entries run scripts that try to escape the Entry frame.
//!
//! `cargo run -p opendict-mdict-fixture --example probe_library -- <folder> <sink URL>`
//!
//! `<sink URL>` is a local HTTP server the test watches: any request reaching it is a
//! network access that should not have happened (or, for "Networked Probe" once the reader
//! allows it, one that should).

use std::path::PathBuf;

use opendict_mdict_fixture::MdxBuilder;

/// Tries every route out of the frame it can think of and writes what happened into
/// `#probe` as JSON.
const PROBE_SCRIPT: &str = r#"
(async () => {
  const sink = document.querySelector("[data-sink]").dataset.sink;
  const out = {};
  // The Entry document's URL is <base>/<hex of the Dictionary id>/?headword=…
  const hex = (text) => [...new TextEncoder().encode(text)].map((b) => b.toString(16).padStart(2, "0")).join("");
  const ownKey = new URL(document.URL).pathname.split("/")[1];
  const ownId = new TextDecoder().decode(new Uint8Array(ownKey.match(/../g).map((h) => parseInt(h, 16))));
  const otherId = ownId.replace(/probe\.mdx$/, "networked.mdx");
  out.dictionaryId = ownId;
  const settle = (promise, ms = 2500) => Promise.race([
    Promise.resolve(promise).then((v) => "answered: " + JSON.stringify(v), (e) => "refused: " + String(e)),
    new Promise((resolve) => setTimeout(() => resolve("no answer"), ms)),
  ]);
  const attempt = (f) => { try { return "allowed: " + String(f()); } catch (e) { return "blocked: " + e.name; } };

  out.origin = String(self.origin);
  out.tauriInternals = typeof window.__TAURI_INTERNALS__;
  out.chromeWebview = typeof window.chrome?.webview;
  out.ipc = typeof window.ipc;

  // Tauri's invoke, if its bootstrap was injected into this frame. Capture the invoke key
  // from the request it makes, to try the other IPC routes with it.
  let key = "";
  const realFetch = window.fetch;
  window.fetch = (url, init) => {
    try { key = new Headers(init?.headers).get("Tauri-Invoke-Key") ?? key; } catch {}
    return realFetch(url, init);
  };
  const T = window.__TAURI_INTERNALS__;
  if (T?.invoke) {
    out.invokeLookUp = await settle(T.invoke("look_up", { headword: "probe" }));
    out.invokeSetAllowNetwork = await settle(T.invoke("set_allow_network", { dictionaryId: ownId, allow: true }));
  }
  window.fetch = realFetch;
  out.capturedInvokeKey = key ? "yes" : "no";
  const message = JSON.stringify({ cmd: "set_allow_network", callback: 1, error: 2,
    payload: { dictionaryId: ownId, allow: true }, options: {}, __TAURI_INVOKE_KEY__: key });
  out.webviewPostMessage = attempt(() => window.chrome.webview.postMessage(message));
  out.ipcPostMessage = attempt(() => window.ipc.postMessage(message));
  for (const url of ["http://ipc.localhost/set_allow_network", "ipc://localhost/set_allow_network"]) {
    out["fetch " + url] = await settle(fetch(url, { method: "POST",
      headers: { "Content-Type": "application/json", "Tauri-Invoke-Key": key },
      body: JSON.stringify({ dictionaryId: ownId, allow: true }) }));
  }

  // The app and stored data.
  out.parentDocument = attempt(() => parent.document.title);
  out.topDocument = attempt(() => top.document.title);
  out.parentLocalStorage = attempt(() => parent.localStorage.length);
  out.localStorage = attempt(() => localStorage.length);
  out.cookie = attempt(() => { document.cookie = "probe=1"; return document.cookie; });
  out.indexedDB = attempt(() => indexedDB.open("probe") && "opened");

  // Other Dictionaries' files, and this Dictionary's own, read by script.
  out.fetchOwnResource = await settle(fetch("probe.css").then((r) => r.status));
  out.fetchOtherDictionary = await settle(fetch(new URL(`/${hex(otherId)}/networked.css`, document.URL)).then((r) => r.status));

  // The network.
  out.fetchSink = await settle(fetch(sink + "/fetch"));
  out.beacon = attempt(() => navigator.sendBeacon(sink + "/beacon", "x"));
  out.image = await settle(new Promise((resolve, reject) => {
    const img = new Image(); img.onload = () => resolve("loaded"); img.onerror = () => reject("error"); img.src = sink + "/image";
  }));
  out.webSocket = await settle(new Promise((resolve, reject) => {
    try { const ws = new WebSocket(sink.replace("http", "ws") + "/socket"); ws.onopen = () => resolve("open"); ws.onerror = () => reject("error"); }
    catch (e) { reject(e.name); }
  }));
  out.popup = attempt(() => window.open(sink + "/popup"));
  out.topNavigation = attempt(() => { top.location.href = sink + "/top"; return "assigned"; });

  const report = document.getElementById("probe");
  report.textContent = JSON.stringify(out);
  report.dataset.done = "yes";
})();
"#;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let folder: PathBuf = args.next().map(Into::into).expect("a folder to write to");
    let sink = args
        .next()
        .map(|s| s.to_string_lossy().into_owned())
        .expect("the sink URL");
    std::fs::create_dir_all(&folder)?;

    let probe = format!(
        r#"<p id="probe" data-sink="{sink}">probing (sample)</p><script>{PROBE_SCRIPT}</script>"#
    );
    MdxBuilder::new()
        .title("Probe")
        .entry("probe", &probe)
        .write_to(folder.join("probe.mdx"))?;
    std::fs::write(folder.join("probe.css"), "p { margin: 0 }")?;

    let networked = format!(
        r#"<p>an image from the network (sample)</p><img id="remote" src="{sink}/allowed.png" alt="">"#
    );
    MdxBuilder::new()
        .title("Networked Probe")
        .entry("probe", &networked)
        .write_to(folder.join("networked.mdx"))?;
    std::fs::write(folder.join("networked.css"), "p { margin: 0 }")?;

    // Navigating its own frame is the one thing a content security policy inside the
    // frame cannot forbid; the app's policy for frames has to.
    let wandering = format!(
        r#"<p>leaving for the network (sample)</p><script>location.href = "{sink}/navigate?word=probe";</script>"#
    );
    MdxBuilder::new()
        .title("Wandering Probe")
        .entry("probe", &wandering)
        .write_to(folder.join("wandering.mdx"))?;

    println!("{}", folder.display());
    Ok(())
}
