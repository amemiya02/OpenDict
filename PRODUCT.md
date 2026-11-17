# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Tauri 2 (stable; move to Tauri 3 once it is stable) with a Rust core. Interface: React 19, TypeScript, Vite, Tailwind CSS 4.

The interface runs in the system WebView, not a bundled browser: WebView2 on Windows 10/11, WKWebView on macOS 14+, WebKitGTK on recent Linux distributions (Ubuntu 24.04 class). Newly available web platform features are allowed when feature-detected with graceful degradation. No polyfills.

## Users

Primary: close readers of foreign-language text. They are reading an English or Japanese original on their computer (a paper, a novel, an article), hit a word they do not know many times a session, look it up, and go straight back to the text.

Also served, but not the design centre: translators and writers who keep the main window open and compare several dictionaries; collectors who manage a hundred or more dictionaries.

## Product Purpose

OpenDict is an open-source, cross-platform reader for the MDict dictionary files (MDX/MDD) a person already owns. It aims at feature parity with MDict with a completely rebuilt interface. Success is a lookup that feels instant and a reader who is back in their text within seconds.

## Positioning

Every Dictionary in the Current Group answers on one continuous page, each Entry still in the layout and styles its dictionary's authors gave it. Local only: no account, no server.

## Operating Context

- The Mini Window is summoned by a global shortcut over whatever the reader is in: a PDF viewer, a browser, an e-book.
- The main window is for longer sessions: comparing dictionaries, browsing, managing the Library.
- Libraries range from a handful of dictionaries to a hundred or more, around 10 GB.
- Keyboard first: every action is reachable by shortcut and through a command palette. The mouse always works too.
- Lookup languages: English, Japanese, Chinese. Interface languages: Simplified Chinese, English, Japanese. Mixed CJK and Latin text is the normal case, not the exception.

## Capabilities and Constraints

First release (v0.1), desktop only (Windows, macOS, Linux); mobile is a later phase:

- Library management: Library Folders, enable/disable, ordering, Groups.
- Lookup as you type: prefix suggestions, all dictionaries of the Current Group at once, results as collapsible per-dictionary cards on one scrolling page with a jump rail.
- Wildcard and fuzzy Lookup; Normalization; Lemmatization for English and Japanese.
- History and back/forward; Word Books with CSV/TSV export.
- Audio playback, links between Entries.
- Light and dark themes, adjustable font and zoom.
- Mini Window on a global shortcut; clipboard watching is opt-in.

Constraints:

- Entry content is third-party HTML, CSS and JavaScript. It is shown sandboxed, with each dictionary's styles isolated from the app and from other dictionaries. OpenDict does not control how an Entry looks inside its section.
- Under the dark theme, Entries are recoloured by default, and each Dictionary can be switched to show as authored on its white page (accepted with the prototype, 2026-11-17).
- Vocabulary is fixed in `GLOSSARY.md`.

## Brand Commitments

- Name: OpenDict.
- Craft bar set by the user: keyboard-first, minimal, content-first, at the level of Raycast, Linear and Arc. These are a bar, not a look to copy.
- Native window materials (Mica on Windows, vibrancy on macOS).
- The result is wrong, however polished, if it: feels like a web page rather than a desktop application; lets the chrome upstage Entry content; looks like generic AI-generated UI (purple gradients, stacked glass cards, glowing borders); is sparse, showing little per screen.

## Evidence on Hand

- No logo, screenshots, users or testimonials exist yet.
- No real dictionaries are in the repository and none may ever be committed (copyright). Entries shown in prototypes are authored samples and must be labelled as such.

## Product Principles

1. Back to the text. Optimise the path from unknown word to understanding to returning to what the reader was reading.
2. The Entry is the product. Everything around it recedes.
3. Faithful to the dictionary. Show Entries as their authors made them.
4. Keyboard first, mouse always possible.
5. Local and yours. The user's files and data stay on their machine and can be moved as a folder.

## Accessibility & Inclusion

Every action must be operable from the keyboard. The interface ships in three languages and must set mixed CJK and Latin text well.
