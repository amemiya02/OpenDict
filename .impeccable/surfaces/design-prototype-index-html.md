---
version: 1
slug: "design-prototype-index-html"
primary_target: "design/prototype/index.html"
related_targets: ["design/prototype/mini.html"]
---

# Main window and Mini Window prototype

Scope: clickable HTML prototype of the OpenDict main window (`design/prototype/index.html`) and the Mini Window (`design/prototype/mini.html`). Visitor mode: Operate.

Audience and job: a close reader of English or Japanese text who hits an unknown word, looks it up, and returns to the text. Primary task: type or summon a word and read every answering Dictionary on one page. Content: authored sample Entries only, labelled as samples; no real dictionary content.

Constraints: must feel like a desktop application, not a web page; chrome never upstages Entry content; dense, not sparse; every action reachable from the keyboard; interface strings in Simplified Chinese, English and Japanese; Entry content is third-party HTML shown in style-isolated containers.

## Direction contract

THESIS: The window is one well-set page of a dictionary. Every Dictionary in the Current Group answers on a single ruled page, and the book's thumb index is how you move between them. It refuses the grey toolbar-list-webview arrangement and the neon command-launcher clone.

OWN-WORLD: India-paper white (a quiet neutral, not cream), ink black, and one vermilion reserved for "you are here" (selected suggestion, active thumb notch, caret). An ink-black cover band is the title bar. Headwords in bold serif inside 【 】; chrome in the system sans. A double rule under the page head, single hairlines between dictionaries, both running to the fore-edge; no card boxes, radius 2px at most. The Mini Window alone takes an 8px corner, because it is an operating-system window. Dark theme is the closed book: navy bookcloth ground, paper-coloured type, cloth-toned thumb notches, the same vermilion.

STORY: The reader types or summons a word, sees every dictionary's answer at once, each in its own typographic voice, and is back in their text within seconds.

FIRST VIEWPORT: 48px cover band with wordmark, Group switch, back and forward, and a 620px search field. Left 264px column: running-head guide words, the Lemmatization note, a dense serif suggestion list. Centre: 【headword】 at 38px over a double rule, then dictionary sections separated by hairlines. Right edge: black half-round thumb index, one notch per answering Dictionary, the active one vermilion.

SIGNATURE INTERACTION: the thumb index follows the scroll position and jumps on click or Alt+number; notches seat in from the fore-edge when a Lookup lands. Motion grammar: single axis, short, no overshoot; Entries swap by cross-fade.

FORM: the lexicographer's page, position 1 of 7 on the ordered list, chosen by the user over the rolled assignment (position 4); seed:363a92b39ac1e6ad

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Unresolved

- How Entries whose dictionaries assume a white page behave under the dark theme. The prototype proposes recolouring them by default (the dictionary's own colours are inverted, so they change, not merely dim) with a per-dictionary "as authored" switch that keeps the white page; the user decides.
- CJK serif for headwords in the shipped app: bundle a face or rely on system Songti/Mincho. The prototype loads Noto Serif from Google Fonts, which the offline app cannot do.
