# OpenDict: notes for coding agents

Read these before changing anything:

- `GLOSSARY.md` has the project's vocabulary; use those terms in code and docs.
- `PRODUCT.md` says who the product is for.
- `docs/roadmap.md` has the founding decisions and milestones; `docs/adr/` has the hard-to-reverse ones.
- `DESIGN.md` is required reading before any interface work. `design/prototype/` is the reference build of the main window and the Mini Window. It is a design reference, not production code. Serve it over HTTP (it uses ES modules), for example with `python -m http.server` from the repository root, then open `/design/prototype/`.

## Rules

- **Never commit dictionary files** (`.mdx`, `.mdd`) or any dictionary content; they are copyrighted. Test with generated fixtures.
- **Licence boundary.** `crates/mdict` is MIT OR Apache-2.0. Nothing from GPL-licensed code may enter it, and its dependencies must be permissively licensed.
- **Entry content is untrusted HTML, CSS and JavaScript.** It always renders sandboxed and style-isolated. Never let it reach app capabilities or user data.

## Web platform policy

**Browser Support:** the interface runs in system WebViews, not a bundled browser:

- WebView2 on Windows 10 and 11;
- WKWebView on macOS 14 and later;
- WebKitGTK on recent Linux distributions (Ubuntu 24.04 class).

Newly Available web platform features are allowed when they are feature-detected and degrade gracefully. Do not add polyfills. Check WebKit support specifically: WebKitGTK and WKWebView lag Chromium.

## Agent skills

### Issue tracker

Issues and specs live in GitHub Issues for amemiya02/OpenDict, managed with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
