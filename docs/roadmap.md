# Roadmap and founding decisions

These decisions were settled in the founding design review on 2026-11-16, before any code was written. Change them deliberately: update this file, and write an ADR in `docs/adr/` when the change is hard to reverse.

Vocabulary (Dictionary, Library, Group, Entry, Lookup and so on) is defined in [GLOSSARY.md](../GLOSSARY.md). Who the product is for is in [PRODUCT.md](../PRODUCT.md). How it looks is in [DESIGN.md](../DESIGN.md).

## Scope

- **Platforms.** Desktop only for now: Windows, macOS, Linux. Android may follow in a later phase, so the core stays free of desktop-only assumptions. There will be no iOS version (decided 2026-11-17).
- **Contributions.** Accepted under the repository's licences, with no CLA and no extra licence exceptions; without iOS there is no App Store conflict to plan for (see ADR-0001).
- **MDict compatibility means reading MDict files.**
  - MDX and MDD, format versions 1.2 and 2.0.
  - zlib and LZO compression.
  - UTF-8, UTF-16, GBK and Big5 encodings.
  - `Encrypted=2` (encrypted key index, no key needed) and `Encrypted=1` (needs the user's registration code or e-mail).
  - `entry://` links, `sound://` audio, `@@@LINK=` redirects, and MDD resources (CSS, JS, images, fonts, audio).
  - Format 3.0 comes later. Authoring dictionaries (what MdxBuilder does) is out of scope.
- **Other formats.** Only MDict in v0.1, behind a pluggable format interface so StarDict, DSL and others can follow.
- **v0.1 features.**
  - Library: Library Folders, enable and disable, ordering, Groups.
  - Lookup as you type: prefix suggestions; every Dictionary of the Current Group on one page as collapsible sections with a thumb index.
  - Wildcard and fuzzy Lookup, Normalization, and Lemmatization for English and Japanese.
  - History with back and forward; Word Books with CSV/TSV export.
  - Audio playback and links between Entries.
  - Light and dark themes, adjustable font and zoom.
  - The Mini Window on a global shortcut. Clipboard watching is opt-in.
- **Later.** Full-text search, screen word capture and OCR, text-to-speech, online dictionaries, custom Entry styles, direct Anki sync, portable mode, winget/Homebrew/Flatpak, paid code signing.
- **Data.** Local only: no account, no server. All user data lives in one data directory the user can move, for example into a synced folder.
- **Languages.** Interface in Simplified Chinese, English and Japanese from the first release. Code, commit messages and documentation are in English; the README is bilingual.
- **Speed.** There are no numeric targets, but "fast" constrains every technical choice.

## Architecture

- **Shell.** Tauri 2 (stable). Move to Tauri 3 once it is stable; it was at 3.0.0-alpha in November 2026. Keep Tauri-specific code in a thin layer so the move stays cheap.
- **Stack.** Rust for parsing, indexing and Lookup. React 19, TypeScript, Vite and Tailwind CSS 4 for the interface, which runs in the system WebView (see `CLAUDE.md` for the web platform policy).
- **Repository layout.**
  - `crates/mdict`: the MDict parser, licensed MIT OR Apache-2.0, publishable on its own as the package `opendict-mdict` (imported as `opendict_mdict`; the names `mdict`, `mdict-rs` and `mdx` are taken on crates.io).
  - `crates/opendict-core`: Library, index, Lookup, Word Books; no UI. GPL-3.0-or-later.
  - `apps/desktop`: the Tauri shell and React interface. GPL-3.0-or-later.

  See [ADR-0001](adr/0001-gpl-app-permissive-parser.md) for the licence split.
- **Parser.** Written from scratch in Rust from public descriptions of the format. It is not ported from GPL readers and does not wrap third-party MDict crates.
- **Index.** Built once per Dictionary when it is first found, stored in the app data directory, rebuilt when the file changes. Dictionary files are referenced in place and never modified.
- **Entry content is untrusted.** Dictionary JavaScript runs in a sandbox. It cannot call app capabilities, read other Dictionaries or user data, or reach the network. Network access can be enabled per Dictionary.
- **Entry rendering.** One page per Lookup, with one style-isolated section per Dictionary. In the dark theme Entries are recoloured by default, and each Dictionary can be switched to show as authored on a white page.
- **Normalization and Lemmatization.**
  - Normalization ignores case, diacritics, full-width vs half-width forms, Simplified vs Traditional characters, and hiragana vs katakana.
  - Lemmatization uses rule tables for English inflection and Japanese conjugation.
  - There is no sentence segmentation.
- **Mini Window.** A global shortcut copies the current selection and opens the Mini Window near the pointer; Esc closes it. The main window has its own shortcut.
- **Word Books.** Several per user. Each record keeps the headword, the source Dictionary, an optional note and the time it was added. There is no spaced-repetition study mode; export to CSV/TSV, which Anki can import.
- **Releases.** GitHub Actions builds for all three platforms and publishes to GitHub Releases. The app updates itself from there using Tauri's updater, which is signed with a self-generated key. Installers are not code-signed for now.

## Testing

- **Never commit copyrighted dictionaries** (`*.mdx` and `*.mdd` are git-ignored). Automated tests use small fixture dictionaries that we generate, covering each version, encoding, compression and encryption combination.
- **Compatibility runs against real dictionaries.** These point at a local folder through an environment variable (planned name: `OPENDICT_TEST_DICTS`). Their results stay on the developer's machine.

## Milestones

Each milestone is verified before the next starts.

1. **Founding documents and design.** Product record, glossary, decisions, design system and the clickable prototype in `design/prototype/`. *Done with the first push.*
2. **`crates/mdict` and a command-line tool** that lists headwords, looks words up and extracts resources, verified against real dictionaries.
3. **`crates/opendict-core`.** Library scanning, indexing, Lookup across Dictionaries, Normalization, Lemmatization.
4. **Desktop interface.** Main window, sandboxed Entry rendering, Library and Groups, built to `DESIGN.md` and the prototype.
5. **History, Word Books, the Mini Window, and the three interface languages.**
6. **Release pipeline.** CI builds, GitHub Releases, in-app updates.

Until v0.1, work is committed directly to `main`; after v0.1, through branches and pull requests.

## Design process

The visual direction ("The Lexicographer's Page") was chosen from four sketches. The prototype passed an independent review, and the remaining findings were fixed. `DESIGN.md` records the system as built.

- New screens follow `DESIGN.md`.
- Implementation follows the modern web guidance and the policy in `CLAUDE.md`.
- Each finished surface gets a design review before it ships.

## Open questions

- **Headword font for CJK.** Bundle a CJK serif for headwords, or rely on the system's Songti and Mincho faces. The prototype loads Noto Serif from Google Fonts, which an offline app cannot do.
- **Ordering of the headword index for Japanese.** Order by reading or by code point, so that the column shows a match's real neighbours.
