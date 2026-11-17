# OpenDict

**English** | [简体中文](README.zh-CN.md)

OpenDict is an open-source, cross-platform dictionary reader for the MDict dictionary files (`.mdx` / `.mdd`) you already own. It aims to match MDict feature for feature with a completely rebuilt interface: every dictionary answers on one page, each in its own typographic style, and every action is reachable from the keyboard.

> **Status: pre-release.** The v0.1 milestone (see [`docs/roadmap.md`](docs/roadmap.md)) is in place. No release has been published yet; see "Installing" for how releases will arrive.

## What is here

| Path | What it is |
|---|---|
| [`PRODUCT.md`](PRODUCT.md) | Who OpenDict is for and what it must do |
| [`GLOSSARY.md`](GLOSSARY.md) | The project's vocabulary |
| [`docs/roadmap.md`](docs/roadmap.md) | Founding decisions, architecture and milestones |
| [`docs/adr/`](docs/adr/) | Architecture decision records |
| [`DESIGN.md`](DESIGN.md) | The design system ("The Lexicographer's Page") |
| [`design/prototype/`](design/prototype/) | Clickable HTML prototype of the main window and the Mini Window |
| [`crates/mdict/`](crates/mdict/) | `opendict-mdict`, the MDict parser (MIT OR Apache-2.0) |
| [`crates/opendict-core/`](crates/opendict-core/) | `opendict-core`: Library and Lookup, no interface |
| [`crates/mdict-fixture/`](crates/mdict-fixture/) | Writes the small MDX files the tests use |
| [`apps/desktop/`](apps/desktop/) | The desktop app: Tauri 2 shell and React interface |

To try the prototype, serve the repository root over HTTP and open `/design/prototype/`:

```bash
python -m http.server 8000
```

Then open <http://localhost:8000/design/prototype/>. Every entry in the prototype is a written sample, not taken from a real dictionary.

## Installing

Installers for Windows, macOS and Linux are attached to each [GitHub Release](https://github.com/amemiya02/OpenDict/releases) once one is published. An installed copy checks for a newer release when it starts, shows it in the status line, and installs it with one click on **Install and restart**.

OpenDict is not code-signed with a paid certificate, so Windows and macOS warn the first time you open it. The updates themselves are signed with OpenDict's own key and checked before they are installed.

- **Windows.** Run `OpenDict_<version>_x64-setup.exe`. If SmartScreen says "Windows protected your PC", click **More info**, then **Run anyway**.
- **macOS.** Open the `.dmg` for your Mac (`aarch64` for Apple silicon, `x64` for Intel) and drag OpenDict to Applications. The first time you open it, macOS says it cannot verify the developer: click **Done**, then open **System Settings → Privacy & Security**, scroll down, and click **Open Anyway** next to the message about OpenDict. Alternatively, run `xattr -dr com.apple.quarantine /Applications/OpenDict.app` once in Terminal.
- **Linux.** Use the `.AppImage` (make it executable with `chmod +x`, then run it) or install the `.deb` with `sudo apt install ./OpenDict_<version>_amd64.deb`. Only the AppImage updates itself; with the `.deb`, install each new release the same way.

Maintainers: how releases are made and signed is in [`docs/releasing.md`](docs/releasing.md).

## Developing

You need Rust (stable), Node.js 24 or later, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```bash
cargo test --workspace                       # parser, core and shell tests
cargo run -p opendict-mdict-fixture --example sample_library -- sample-library
cd apps/desktop
npm ci
npm run typecheck && npm run lint && npm test
OPENDICT_LIBRARY_FOLDER=../../sample-library npx tauri dev
```

The app reads its Library Folder from `libraryFolder` in `config.json` in its configuration directory, or from the `OPENDICT_LIBRARY_FOLDER` environment variable. On Windows, `npx tauri build --debug --no-bundle && npm run e2e` runs the end-to-end smoke test against the built app.

## Planned

- **Desktop.** Windows, macOS and Linux, built on Tauri and Rust. Android may follow later.
- **Reading MDict files.** MDX/MDD versions 1.2 and 2.0, common compressions and encodings, and both kinds of MDict encryption.
- **Lookup.** Lookup as you type across a Group of dictionaries, with wildcards, fuzzy matching, and inflection handling for English and Japanese.
- **Reading tools.** History, Word Books with CSV/TSV export, and a Mini Window that looks up selected text from any application.
- **Interface languages.** Simplified Chinese, English and Japanese.
- **Privacy.** Local only: no account and no server.

See [`docs/roadmap.md`](docs/roadmap.md) for the full plan.

## License

The application is licensed under the [GNU GPL v3.0 or later](LICENSE). The MDict parsing library (`crates/mdict`) is licensed MIT OR Apache-2.0 so other projects can reuse it. See [ADR-0001](docs/adr/0001-gpl-app-permissive-parser.md).

OpenDict ships no dictionaries. Use dictionaries you have the right to use.
