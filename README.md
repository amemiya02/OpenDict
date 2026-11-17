# OpenDict

**English** | [简体中文](README.zh-CN.md)

An open-source, cross-platform reader for the MDict dictionaries (`.mdx` / `.mdd`) you already own. Every dictionary answers on one page, each in its own style, and everything works from the keyboard.

## Install

Download the installer for your system from [Releases](https://github.com/amemiya02/OpenDict/releases). OpenDict updates itself from then on.

OpenDict is not code-signed, so the first launch shows a warning:

- **Windows:** in SmartScreen, click **More info → Run anyway**.
- **macOS:** open **System Settings → Privacy & Security** and click **Open Anyway**, or run `xattr -dr com.apple.quarantine /Applications/OpenDict.app`.
- **Linux:** use the `.AppImage` (`chmod +x` first) or the `.deb`. Only the AppImage updates itself.

## Develop

Needs Rust (stable), Node.js 24+ and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
cargo test --workspace
cargo run -p opendict-mdict-fixture --example sample_library -- sample-library
cd apps/desktop && npm ci && npm test
OPENDICT_LIBRARY_FOLDER=../../sample-library npx tauri dev
```

Plan and design: [`docs/roadmap.md`](docs/roadmap.md), [`DESIGN.md`](DESIGN.md). Releasing: [`docs/releasing.md`](docs/releasing.md).

## License

The app is [GPL-3.0-or-later](LICENSE); the MDict parser `crates/mdict` is MIT OR Apache-2.0 ([ADR-0001](docs/adr/0001-gpl-app-permissive-parser.md)). OpenDict ships no dictionaries.
