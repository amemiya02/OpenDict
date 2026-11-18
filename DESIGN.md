---
name: OpenDict
description: A dictionary reader set like one well-made page of a dictionary.
colors:
  vermilion: "#c9361d"
  vermilion-night: "#f0553a"
  india-paper: "#f5f5f0"
  paper-shade: "#e8e8e0"
  ink: "#141518"
  ink-secondary: "#55575f"
  hairline: "#cfcfc7"
  here-ink: "#ffffff"
  cover-black: "#16171a"
  cover-edge: "#3a3c43"
  on-cover: "#f0f0ea"
  on-cover-muted: "#a6a8af"
  bookcloth-navy: "#111a30"
  bookcloth-raised: "#1b2745"
  night-cover: "#0a1020"
  night-field: "#1e2a4a"
  night-paper-ink: "#ece9df"
  night-ink-secondary: "#a6adc0"
  night-hairline: "#2a3757"
  night-rule: "#8791ad"
  cloth-notch: "#33426a"
typography:
  display:
    fontFamily: "Charis SIL, Noto Serif SC, Noto Serif JP, Songti SC, Yu Mincho, serif"
    fontSize: "38px"
    fontWeight: 700
    lineHeight: 1.15
  headline:
    fontFamily: "Charis SIL, Noto Serif SC, Noto Serif JP, Songti SC, Yu Mincho, serif"
    fontSize: "24px"
    fontWeight: 700
    lineHeight: 1.15
  search:
    fontFamily: "Charis SIL, Noto Serif SC, Noto Serif JP, Songti SC, Yu Mincho, serif"
    fontSize: "17px"
    fontWeight: 400
  index:
    fontFamily: "Charis SIL, Noto Serif SC, Noto Serif JP, Songti SC, Yu Mincho, serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: "26px"
  title:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, -apple-system, Microsoft YaHei UI, PingFang SC, Hiragino Sans, Yu Gothic UI, sans-serif"
    fontSize: "12px"
    fontWeight: 700
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, -apple-system, Microsoft YaHei UI, PingFang SC, Hiragino Sans, Yu Gothic UI, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, -apple-system, Microsoft YaHei UI, PingFang SC, Hiragino Sans, Yu Gothic UI, sans-serif"
    fontSize: "11.5px"
    fontWeight: 400
    fontFeature: "tnum"
rounded:
  sharp: "2px"
  window: "8px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "34px"
components:
  cover-band:
    backgroundColor: "{colors.cover-black}"
    textColor: "{colors.on-cover}"
    height: "48px"
  search-field:
    backgroundColor: "{colors.india-paper}"
    textColor: "{colors.ink}"
    typography: "{typography.search}"
    rounded: "{rounded.sharp}"
    height: "32px"
    width: "620px"
  search-field-night:
    backgroundColor: "{colors.night-field}"
    textColor: "{colors.night-paper-ink}"
  button-outline:
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.sharp}"
    padding: "0 10px"
    height: "28px"
  button-outline-hover:
    backgroundColor: "{colors.paper-shade}"
  button-outline-pressed:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.india-paper}"
  index-row:
    textColor: "{colors.ink}"
    typography: "{typography.index}"
    padding: "0 16px 0 30px"
    height: "26px"
  index-row-selected:
    backgroundColor: "{colors.paper-shade}"
    padding: "0 16px 0 16px"
  dictionary-header:
    textColor: "{colors.ink}"
    typography: "{typography.title}"
    height: "36px"
  thumb-notch:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.india-paper}"
    width: "42px"
    height: "46px"
  thumb-notch-current:
    backgroundColor: "{colors.vermilion}"
    textColor: "{colors.here-ink}"
  thumb-notch-night:
    backgroundColor: "{colors.cloth-notch}"
    textColor: "{colors.night-paper-ink}"
  command-palette:
    backgroundColor: "{colors.india-paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sharp}"
    width: "560px"
  mini-window:
    backgroundColor: "{colors.india-paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.window}"
    width: "408px"
    height: "468px"
---

# Design System: OpenDict

## Overview

**Creative North Star: "The Lexicographer's Page"**

The window is one well-set page of a dictionary. Every Dictionary in the Current Group answers on a single ruled page, each Entry in the typographic voice its own authors gave it, and a thumb index cut into the fore-edge is how the reader moves between them. The interface uses a cover, rules, an index column and a thumb index.

Density is a feature. A reader comparing four dictionaries wants to see four dictionaries, so the headword column runs full, the page rules run to the fore-edge, and empty space only appears where there is genuinely nothing to show. Colour is almost absent: paper, ink, and one vermilion that only ever means "you are here".

The system was built and reviewed as a clickable prototype in `design/prototype/`. The rules below describe that build. Where the shipped app differs, the build in the app wins and this file is updated.

**Key Characteristics:**
- Ink-black cover band as the title bar; India-paper page beneath it.
- Bold serif headwords inside 【 】; system sans for every control.
- Structure drawn with rules (a double rule, single hairlines), never with boxed cards.
- A half-round thumb index at the right edge, one notch per answering Dictionary.
- One vermilion, reserved for the current position.
- Keyboard first: every action reachable from the command palette.

## Colors

A near-monochrome palette of paper and ink, with a navy bookcloth night side and a single vermilion accent.

### Primary
- **Rubric Vermilion** (`vermilion`, `vermilion-night` at night): the selected row in the headword column, the current notch in the thumb index, the text caret, and the check marks for active settings in the command palette. Nothing else.

### Neutral
- **India Paper** (`india-paper`): the page ground, the search field by day, the command palette and the Mini Window.
- **Paper Shade** (`paper-shade`): the selected headword row, hover on rows and buttons.
- **Press Ink** (`ink`): text, rules, outline buttons, the thumb notches by day.
- **Faded Ink** (`ink-secondary`): counts, metadata, the status line, unselected tabs.
- **Hairline Grey** (`hairline`): light dividers inside lists and tables, sample tags.
- **Cover Black** (`cover-black`, edge `cover-edge`, type `on-cover` and `on-cover-muted`): the title bar band and its controls.
- **Bookcloth Navy** (`bookcloth-navy`, raised `bookcloth-raised`, cover `night-cover`, field `night-field`): the dark theme's ground, selected row, title bar and search field.
- **Night Paper Ink** (`night-paper-ink`, secondary `night-ink-secondary`, `night-hairline`, `night-rule`): dark-theme type and rules.
- **Cloth Notch** (`cloth-notch`): thumb notches and Library marks in the dark theme.

### Named Rules
**The You-Are-Here Rule.** Vermilion marks the reader's current position and nothing else. If an element is not "where you are now", it is not vermilion: not focus rings, not primary buttons, not links, not warnings.

**The Closed Book Rule.** The dark theme is designed, not inverted: a navy bookcloth ground, cloth-toned notches and paper-coloured type. The brightest solid in the dark window is the current notch, never the chrome.

## Typography

**Display Font:** Charis SIL, with Noto Serif SC / Noto Serif JP for CJK, falling back to Songti SC and Yu Mincho.
**Body Font:** the platform UI face: Segoe UI Variable on Windows, the system font on macOS, with Microsoft YaHei UI, PingFang SC, Hiragino Sans and Yu Gothic UI for CJK.

**Character:** The serif sets the words a reader looks up; the system sans runs the machinery around them. The pairing reads as a printed dictionary inside a native application.

### Hierarchy
- **Display** (700, 38px, 1.15): the headword at the head of the page, inside 【 】 and hung 14px into the left margin so the bracket sits outside the text edge. Japanese text switches to the JP serif.
- **Headline** (700, 24px, 1.15): the headword in the Mini Window.
- **Search** (400, 17px serif): the text being typed in the search field, so the query looks like the headword it will become.
- **Index** (400, 15px serif, 26px rows): the headword column and the headword lists on the empty and no-match pages.
- **Title** (700, 12px sans): each Dictionary's name in its section header, in 【 】.
- **Body** (400, 13px sans, 1.45): controls, the Library, the command palette.
- **Label** (400, 11–11.5px sans, tabular figures): counts, the status line, notch captions, keyboard hints.

Running text inside the page is held to an 860px measure; the rules around it are not.

### Named Rules
**The Two Voices Rule.** Serif is for words from the dictionary and words the reader typed; sans is for everything the application says. A control label in serif, or a headword in sans, breaks the rule.

**The Hands-Off Entry Rule.** Each Dictionary's HTML and CSS render in their own isolated root. OpenDict provides a base text size and local CJK font defaults. The Dictionary's own typography takes precedence.

## Layout

A fixed desktop frame: a 48px cover band across the top, a 264px headword column on the left, the page in the middle, a 168px thumb index on the right, and a 26px status line under the page. The page scrolls; the frame does not.

- **Page:** 18px top, 34px left and 30px right padding. The page head (headword, pronunciation, answer count, actions) sits over a double rule; Dictionary sections follow, separated by single rules. Both rules run the full width of the page to the thumb index.
- **Headword column** starts with Headword index when the search field is empty and Lookup suggestions when it contains a query. History and Word Book use their own titles. A double rule separates the title from the list and its optional note about lemmatization or near matches. The four tabs sit below the list. Manage opens the Library and application settings.
- **Density:** 26px index rows, 36px Dictionary headers, 28px buttons. Groups sit tight; separation comes from rules, not from padding.
- **Responsive:** at 1100px and below the thumb index drops its captions and keeps only the notches. At 860px and below the column narrows to 200px. At 620px and below the column becomes an overlay over the page, opened by a toggle in the cover band or by typing, and put away by choosing a word, the toggle or Esc; the wordmark gives way, while window controls and back/forward stay.
- **Native chrome:** the window controls are always the system's own. On macOS the cover band is the title bar: the traffic lights sit at its left (the wordmark moves right to clear them) and the band drags the window. On Windows and Linux the system title bar sits above the cover band. Where the system has a window material (Mica on Windows 11, sidebar vibrancy on macOS) it shows faintly through the headword column's paper (65%), as through a native sidebar; the page, the cover band and the narrow-window overlay stay solid. Without one (Windows 10, Linux) the window is solid throughout.
- **Empty and no-match pages** are never blank: they offer the headword index, recent lookups, the Word Book, another Group that has the word, or a wildcard pattern.

## Elevation & Depth

The page is flat. Depth comes from the cover band's darkness and from rules, not from shadows. Two things float above the page, and only they cast or imply depth: the command palette, which dims the window behind it with a backdrop, and the Mini Window, which is an operating-system window over another application.

### Shadow Vocabulary
- **Window float** (`box-shadow: 0 16px 44px -10px rgb(0 0 0 / .42), 0 3px 8px rgb(0 0 0 / .16)`): the Mini Window only.
- **Palette backdrop** (`background: rgb(8 10 14 / .42)`): behind the command palette.

### Named Rules
**The Ruled Page Rule.** Separate with rules, never with boxes. A double rule closes the page head; a single rule separates Dictionaries; hairlines divide rows. No cards, no nested containers, no shadows on anything inside the page.

## Shapes

Square-cut, like print. Controls take a 2px corner at most (`sharp`). The one curve in the system is the thumb notch, a half-round tab (23px radius on its left side only, flat against the window edge) that reads as a cut into the fore-edge. The Mini Window takes an 8px corner (`window`) because it is an operating-system window, not a page element.

## Components

### Buttons
- **Shape:** square-cut (2px).
- **Outline (default):** 1px ink border, ink text, 28px high, 10px side padding, icon and label with an optional keyboard hint.
- **Hover / Press:** Paper Shade on hover; the button settles 1px down while pressed.
- **Toggled on** (Add to Word Book when saved): solid ink with paper-coloured text.
- **Icon buttons:** 26px square, Faded Ink at rest, ink on a Paper Shade ground on hover; toggled on is solid ink.

### Inputs / Fields
- **Search field** uses India Paper by day and Night Field after dark. It is 32px high with serif query text and a search icon on the left. An outlined Commands button sits beside the field, with the shortcut shown when space permits.
- **Focus:** a 1px ring in the cover's paper colour. The vermilion caret already says where you are typing.

### Headword column
- **Rows** are 26px high with serif headwords. When two or more Dictionaries file a headword, a count appears at the right in tabular figures.
- **Selected:** Paper Shade ground, bold, and a 6px vermilion square in the margin.
- **Tabs:** sans 12px; the active tab carries a 2px ink rule along its top edge and keeps the same weight as the others.

### Dictionary section
- **Header:** the Dictionary name in 【 】 (Title type), a "sample" tag where relevant, then fold and, in the dark theme, an "as authored" switch at the right.
- **Body:** the Entry, untouched. In the dark theme it is recoloured by default; "as authored" restores the Dictionary's white page inside a 2px-cornered panel.

### Thumb index (signature component)
- **Notch:** 42 × 46px half-round tab, ink by day and Cloth Notch at night, carrying the Dictionary's one-character mark in bold serif; its name sits to the left in Label type.
- **Current:** vermilion notch, bold caption. It follows the scroll position; click or Alt + digit jumps.
- **Motion:** when a Lookup lands, notches seat in from the edge one after another (0.32s, 28ms stagger, exponential ease-out). Hover pulls a notch 3px out.

### Command palette
- **Style:** a 560px India Paper sheet, square-cut, with the input over a double rule and commands as 30px rows. A vermilion check marks the current Group, theme and interface language; keyboard hints sit at the right.
- **Behaviour:** Ctrl K opens it; arrows move; Enter runs; Esc or a click outside closes.

### Mini Window
- **Style:** a 408 × 468px window with an 8px corner and the Window float shadow: its own cover band with the search field, the page in miniature (24px headword), a 30px thumb index, and a footer with Add to Word Book, Open in main window and the Esc hint.
- **Placement:** below the selected text, or above it, or beside it, whichever keeps it fully on screen. The pointer stands for the selection (the system does not say where a selection in another application is): the window opens below the pointer's line, above it, or beside the pointer.
- **Settings:** its two global shortcuts (look up the selection, bring the main window forward) and clipboard watching sit in a "Mini Window" section at the foot of the Library view, as rows over hairlines; a shortcut changes by pressing the new one.

## Do's and Don'ts

### Do:
- **Do** keep vermilion to the current position: selected row, current notch, caret, active-setting checks.
- **Do** run the page's double rule and section rules the full width of the page to the thumb index.
- **Do** set headwords and typed queries in the serif, and every control label in the system sans.
- **Do** show every answering Dictionary on one page, in the Current Group's order, each Entry exactly as its Dictionary styles it.
- **Do** give empty and no-match states somewhere to go: the headword index, recent lookups, another Group, a wildcard pattern.
- **Do** design the dark theme from bookcloth navy and cloth notches, and keep the current notch the brightest solid.

### Don't:
- **Don't** put Entries, settings or lists in cards, boxes or shadowed containers inside the page.
- **Don't** use vermilion for focus rings, primary buttons, links or emphasis.
- Preserve the Dictionary's layout and typography. The reader can switch off dark-theme recolouring per Dictionary.
- **Don't** leave the headword column or the page mostly empty when there are headwords to show.
- **Don't** use gradients, glass effects or glowing borders; the reader asked for none of them.
- **Don't** round controls beyond 2px; only the Mini Window, as a real window, takes 8px.
