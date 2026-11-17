import { useEffect, useEffectEvent, useRef, useState, type ReactNode } from "react";
import { CommandPalette, type Command } from "./components/CommandPalette";
import { EntryFrame } from "./components/EntryFrame";
import { GroupSwitcher } from "./components/GroupSwitcher";
import { COLUMN_ID, HeadwordColumn, optionId, type ColumnItem, type ColumnList } from "./components/HeadwordColumn";
import { MiniWindowSettings } from "./components/MiniWindowSettings";
import { NavButtons } from "./components/NavButtons";
import { RecentLookups, WordList } from "./components/RecentLookups";
import { sectionId, ThumbIndex } from "./components/ThumbIndex";
import { EmptyLibrary, LibraryPage, OutlineButton, type LibraryActions } from "./components/LibraryPage";
import { ReadingSettings } from "./components/ReadingSettings";
import { DataSettings } from "./components/DataSettings";
import { SearchField } from "./components/SearchField";
import { UpdateNotice } from "./components/UpdateNotice";
import { WordBookPage, type WordBookActions } from "./components/WordBookPage";
import { LANGUAGES, useI18n, type Locale } from "./i18n";
import { soundUrl } from "./lib/entryLinks";
import { playSound } from "./lib/sound";
import {
  addLibraryFolder,
  cancelIndexing,
  clearHistory,
  createGroup,
  deleteGroup,
  elsewhere,
  getHistory,
  getLibrary,
  indexLibrary,
  lookUp,
  moveDictionary,
  moveInGroup,
  recordLookup,
  registerDictionary,
  onLibraryChanged,
  removeLibraryFolder,
  renameGroup,
  rescanLibrary,
  setAllowNetwork,
  setAsAuthored,
  setCurrentGroup,
  setDictionaryEnabled,
  setInGroup,
  chooseWordBook,
  createWordBook,
  deleteWordBook,
  exportWordBook,
  getWordBooks,
  removeSavedWord,
  renameWordBook,
  setWordNote,
  toggleWord,
  windowChrome,
  onOpenInMain,
  onWordBooksChanged,
  type Elsewhere,
  type Entry,
  type Group,
  type Library,
  type RegisterError,
  type WindowChrome,
  type Suggestion,
  type WordBooks,
} from "./lib/ipc";
import { useAppearance, type Scheme } from "./lib/useAppearance";
import { hintOf, keysOf, press, shortcutOfEvent, type Shortcut } from "./lib/keymap";
import { MOD } from "./lib/platform";
import { canStep, emptyTrail, step, visit, type Trail } from "./lib/trail";
import { TEXT_SIZES, zoomStep } from "./lib/useAppearance";
import { useHeadwordColumn } from "./lib/useHeadwordColumn";
import { wildcardHint } from "./lib/wildcard";

type PageState =
  | { kind: "idle" }
  /** `group` is the Current Group the Entries came from (null: the whole Library). */
  | { kind: "found"; headword: string; entries: Entry[]; group: string | null }
  /** Nothing found for `query`; `next` are the headwords after where it would stand. */
  | { kind: "missing"; query: string; next: Suggestion[] }
  | { kind: "failed"; reason: string };

type View = "lookup" | "library" | "wordBook";

/** The main window: cover band, headword column, page and status line. */
export function App() {
  const { t, locale, followsSystem, chooseLanguage } = useI18n();
  const [query, setQuery] = useState("");
  const [page, setPage] = useState<PageState>({ kind: "idle" });
  const [view, setView] = useState<View>("lookup");
  const [list, setList] = useState<ColumnList>("suggestions");
  const [library, setLibrary] = useState<Library | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [wordBooks, setWordBooks] = useState<WordBooks | null>(null);
  /** What the status line says about the last Word Book change. */
  const [said, setSaid] = useState<string | null>(null);
  /** What the Word Book view says about the last change made there. */
  const [bookNotice, setBookNotice] = useState<string | null>(null);
  /** Committed Lookups, newest first. */
  const [history, setHistory] = useState<string[]>([]);
  /** The words visited in this window, for Back and Forward. */
  const [trail, setTrailState] = useState(emptyTrail);
  /** The trail as it is now, ahead of rendering: Back may be pressed before a render. */
  const trailRef = useRef(trail);
  /** How many words the reader has chosen, so a slower preview never overtakes a choice. */
  const commits = useRef(0);
  /** The Lookup the reader last committed, while it is under way. */
  const committing = useRef<Promise<void>>(Promise.resolve());
  const [paletteOpen, setPaletteOpen] = useState(false);
  /** The Dictionaries folded on the found page (by page key). */
  const [folded, setFolded] = useState<{ page: string; ids: ReadonlySet<string> }>({ page: "", ids: new Set() });
  const searchRef = useRef<HTMLInputElement>(null);
  /** Where the word on the page stands in History, or -1. */
  const historySelected = page.kind === "found" ? history.indexOf(page.headword) : -1;
  const [platform, setPlatform] = useState<WindowChrome["platform"] | null>(null);
  /** Narrow windows: the reader opened the headword column with the toggle in the cover band. */
  const [columnOpen, setColumnOpen] = useState(false);
  /** Narrow windows: the reader is typing, so the column shows over the page until a word is chosen. */
  const [typing, setTyping] = useState(false);
  /** The Word Book Ctrl/⌘ + D saves into, and where the word on the page stands in it, or -1. */
  const currentBook = wordBooks?.books.find((book) => book.id === wordBooks.current) ?? null;
  const savedHere = (headword: string) => currentBook?.words.some((word) => word.headword === headword) ?? false;
  const wordBookSelected =
    page.kind === "found" && currentBook ? currentBook.words.findIndex((word) => word.headword === page.headword) : -1;
  const pageRef = useRef<HTMLElement>(null);
  const column = useHeadwordColumn();
  const appearance = useAppearance();
  const { refresh } = column;
  /** Counts page requests, so only the latest one lands. */
  const pageRequests = useRef(0);
  /** The text in the search field, for refreshing the column after a Library change. */
  const typed = useRef("");
  /** The Current Group as the shell last reported it (null: the whole Library). */
  const currentGroup = library?.currentGroup ?? null;
  const groupRef = useRef(currentGroup);
  useEffect(() => {
    groupRef.current = currentGroup;
  }, [currentGroup]);

  // The column opens on the headword index, to browse.
  useEffect(() => {
    getLibrary().then(
      (loaded) => {
        setLibrary(loaded);
        refresh(typed.current).catch(() => {});
      },
      (error: unknown) => setPage({ kind: "failed", reason: String(error) }),
    );
    // Indexing runs in the shell's background and reports the Library as it goes. When
    // another Dictionary's index is ready, the column has more to offer.
    let ready = -1;
    const unlisten = onLibraryChanged((changed) => {
      setLibrary(changed);
      const nowReady = changed.dictionaries.filter((d) => d.index.state === "ready").length;
      if (nowReady !== ready) {
        ready = nowReady;
        refresh(typed.current).catch(() => {});
      }
    });
    return () => {
      unlisten.then((stop) => stop(), () => {});
    };
  }, [refresh]);

  useEffect(() => {
    // Without readable Word Books the rest of the app still works; saving reports why not.
    getWordBooks().then((books) => setWordBooks(Array.isArray(books?.books) ? books : null), () => {});
    getHistory().then((words) => setHistory(asWords(words)), () => {});
  }, []);

  // The shell says what it built the window with: the styles leave room for the title bar
  // controls and let the system material show through (see styles.css).
  useEffect(() => {
    windowChrome().then(
      (chrome: WindowChrome | null) => {
        if (!chrome) return; // a fake shell that answers nothing
        const root = document.documentElement;
        root.dataset.platform = chrome.platform;
        if (chrome.material) root.dataset.material = chrome.material;
        else delete root.dataset.material;
        setPlatform(chrome.platform);
      },
      () => {}, // no shell (or an old one): plain solid window
    );
  }, []);

  // Moving between the page and the Library puts the reader at the top of the new view.
  useEffect(() => {
    pageRef.current?.scrollTo?.({ top: 0 });
    if (view !== "lookup") pageRef.current?.querySelector<HTMLElement>("h1")?.focus();
  }, [view]);

  // A new word, or the same word from another Group, opens at the top of its page, not
  // wherever the reader had scrolled the last one to.
  const shown =
    page.kind === "found" ? `found:${pageKey(page)}` : page.kind === "missing" ? `missing:${page.query}` : page.kind;
  useEffect(() => {
    pageRef.current?.scrollTo?.({ top: 0 });
  }, [shown]);

  function setPageNow(next: PageState) {
    pageRequests.current++;
    setPage(next);
  }

  /**
   * Puts `headword`'s Entries on the page. `onFound` runs in the same update as the page
   * when a Dictionary had the word, so nothing sees the page without it.
   */
  async function show(headword: string, onFound?: () => void) {
    const ticket = ++pageRequests.current;
    setView("lookup");
    try {
      const entries = await lookUp(headword);
      if (ticket !== pageRequests.current) return;
      setPage(
        entries.length > 0
          ? { kind: "found", headword, entries, group: groupRef.current }
          : { kind: "missing", query: headword, next: [] },
      );
      if (entries.length > 0) onFound?.();
    } catch (error) {
      if (ticket === pageRequests.current) setPage({ kind: "failed", reason: String(error) });
    }
  }

  /** The reader's query changed: new suggestions, and the first match on the page at once. */
  async function changeQuery(text: string) {
    setList("suggestions");
    setTyping(true);
    const before = commits.current;
    try {
      const answer = await column.refresh(text);
      // The reader chose a word (Enter) while the suggestions were on their way: that word
      // stays on the page, and its Lookup is not overtaken by this preview.
      if (!answer || commits.current !== before) return;
      if (answer.headword) {
        void show(answer.headword);
      } else if (text.trim()) {
        setView("lookup");
        setPageNow({ kind: "missing", query: text.trim(), next: answer.miss ? answer.items : [] });
      } else {
        setPageNow({ kind: "idle" });
      }
    } catch (error) {
      setPageNow({ kind: "failed", reason: String(error) });
    }
  }

  async function move(by: number) {
    if (list === "history") {
      if (history.length === 0) return;
      const next = historySelected < 0 ? (by > 0 ? 0 : history.length - 1) : historySelected + by;
      const headword = history[Math.max(0, Math.min(next, history.length - 1))]!;
      type(headword);
      void show(headword);
      return;
    }
    if (list === "wordBook") {
      const words = currentBook?.words ?? [];
      if (words.length === 0) return;
      const next = wordBookSelected < 0 ? (by > 0 ? 0 : words.length - 1) : wordBookSelected + by;
      const headword = words[Math.max(0, Math.min(next, words.length - 1))]!.headword;
      type(headword);
      void show(headword);
      return;
    }
    const headword = await column.move(by).catch(() => null);
    if (headword) void show(headword);
  }

  function type(text: string) {
    typed.current = text;
    setQuery(text);
  }

  /**
   * The reader chose a word (Enter, a click, a followed link): it goes into the search
   * field and onto the page. A word that was found is recorded in History and becomes
   * the latest step for Back and Forward.
   */
  async function commit(headword: string) {
    commits.current++;
    putColumnAway();
    type(headword);
    const done = show(headword, () => {
      setTrail(visit(trailRef.current, headword));
      recordLookup(headword).then((words) => setHistory(asWords(words)), () => {});
    });
    committing.current = done;
    await done;
  }

  function setTrail(next: Trail) {
    trailRef.current = next;
    setTrailState(next);
  }

  /** A narrow window's column overlay goes once a word is chosen. */
  function putColumnAway() {
    setTyping(false);
    setColumnOpen(false);
  }

  /** An `entry://` link in an Entry: the linked headword, as if the reader had typed and chosen it. */
  function followLink(headword: string) {
    void commit(headword);
    column.refresh(headword).catch(() => {});
  }

  // The Mini Window hands a word over ("Open in main window"), and either window may save
  // a word to the Word Book.
  const openedFromMini = useEffectEvent((headword: string) => followLink(headword));
  useEffect(() => {
    const stops = [onOpenInMain((headword) => openedFromMini(headword)), onWordBooksChanged((books) => setWordBooks(books))].map((listening) =>
      listening.catch(() => null),
    );
    return () => {
      for (const stop of stops) void stop.then((unlisten) => unlisten?.());
    };
  }, []);

  /** The reader chose a way forward from a miss: a pattern to try or a headword. */
  function search(text: string) {
    type(text);
    void changeQuery(text);
  }

  function commitCurrent() {
    const { items, selected, query: answered } = column.state;
    // Until the shell answers the latest text, the suggestions belong to an earlier query:
    // Enter then takes what was typed, not the earlier query's current headword.
    const fresh = answered === typed.current;
    const current =
      list === "history"
        ? history[historySelected]
        : list === "wordBook"
          ? currentBook?.words[wordBookSelected]?.headword
          : fresh
            ? items[selected]?.headword
            : undefined;
    const headword = current ?? typed.current.trim();
    if (headword) void commit(headword);
  }

  function pick(index: number) {
    const headword =
      list === "history" ? history[index] : list === "wordBook" ? currentBook?.words[index]?.headword : column.select(index);
    if (headword) void commit(headword);
  }

  /**
   * Makes `group` the Current Group (null: the whole Library); every Lookup after this runs
   * against it. The word on the page stays when the new Group has it; when it does not, the
   * page says so and points to where it is. With `word`, that word is looked up in the new
   * Group instead (the no-match page's way to another Group). The cover band's switcher
   * calls this, and so can the command palette.
   */
  async function switchGroup(group: string | null, word?: string) {
    setNotice(null);
    let updated: Library;
    try {
      updated = await setCurrentGroup(group);
    } catch (error) {
      setNotice(t.libraryActionFailed(String(error)));
      return;
    }
    groupRef.current = updated.currentGroup;
    setLibrary(updated);
    if (word !== undefined) {
      column.refresh(word).catch(() => {});
      await commit(word);
      return;
    }
    if (view === "lookup" && page.kind === "missing") {
      await changeQuery(typed.current);
      return;
    }
    column.refresh(typed.current).catch(() => {});
    if (view === "lookup" && page.kind === "found") await show(page.headword);
  }

  /**
   * Back (-1) or Forward (1) through the words visited. Retracing is not a new Lookup. A
   * Lookup the reader has just committed counts as visited, even if it has not answered
   * yet: Back right after Enter goes to the word before it.
   */
  async function go(by: -1 | 1) {
    await committing.current;
    const next = step(trailRef.current, by);
    if (!next) return;
    setTrail(next.trail);
    type(next.word);
    column.refresh(next.word).catch(() => {});
    await show(next.word);
  }

  /** Puts the caret in the search field, its text selected, ready for a new word. */
  function focusSearch() {
    const field = searchRef.current;
    if (!field) return;
    field.focus();
    field.select();
  }


  function forgetHistory() {
    clearHistory().then(
      () => setHistory([]),
      () => {}, // History stays as it was: nothing was cleared
    );
  }

  /** Saves the word on the page to the chosen Word Book, or takes it out (Ctrl/⌘ + D). */
  async function toggleSave() {
    if (view !== "lookup" || page.kind !== "found") return;
    const { headword, entries } = page;
    const from = entries[0]!;
    try {
      const updated = await toggleWord(headword, from.dictionaryId, from.dictionaryTitle, t.newWordBookName);
      setWordBooks(updated);
      const book = updated.books.find((b) => b.id === updated.current);
      const now = book?.words.some((word) => word.headword === headword) ?? false;
      if (book) setSaid(now ? t.savedTo(headword, book.name) : t.removedFrom(headword, book.name));
    } catch (error) {
      setSaid(t.wordBookActionFailed(String(error)));
    }
  }

  /** Runs one Word Book command; the shell answers with the Word Books as they now are. */
  async function changeBooks(command: () => Promise<WordBooks>) {
    setBusy(true);
    setBookNotice(null);
    try {
      setWordBooks(await command());
    } catch (error) {
      setBookNotice(t.wordBookActionFailed(String(error)));
    } finally {
      setBusy(false);
    }
  }

  const bookActions: WordBookActions = {
    choose: (id) => void changeBooks(() => chooseWordBook(id)),
    create: (name) => void changeBooks(() => createWordBook(name)),
    rename: (id, name) => void changeBooks(() => renameWordBook(id, name)),
    delete: (id) => void changeBooks(() => deleteWordBook(id)),
    removeWord: (id, headword) => void changeBooks(() => removeSavedWord(id, headword)),
    setNote: (id, headword, note) => void changeBooks(() => setWordNote(id, headword, note)),
    export: async (id, format) => {
      setBusy(true);
      setBookNotice(null);
      try {
        const file = await exportWordBook(id, format);
        if (file) setBookNotice(t.exported(file));
      } catch (error) {
        setBookNotice(t.wordBookActionFailed(String(error)));
      } finally {
        setBusy(false);
      }
    },
    lookUp: (headword) => void commit(headword),
  };

  function showWordBook() {
    // In a narrow window the overlay gives way to the Word Book view, as for the Library.
    setColumnOpen(false);
    setList("wordBook");
    setView("wordBook");
    setBookNotice(null);
  }

  // The window's shortcuts (lib/keymap.ts), wherever the focus is in it. Keys pressed inside
  // an Entry frame reach here too: the frame reports them and EntryFrame presses them again.
  const onShortcut = useEffectEvent((shortcut: Shortcut, event: KeyboardEvent) => {
    switch (shortcut.kind) {
      case "palette":
        event.preventDefault();
        setPaletteOpen(true);
        return;
      case "focusSearch":
        event.preventDefault();
        focusSearch();
        return;
      case "back":
      case "forward":
        // Also keeps WebView2 from navigating the window.
        event.preventDefault();
        void go(shortcut.kind === "back" ? -1 : 1);
        return;
      case "save":
        // Ctrl+D would otherwise bookmark the page in WebView2.
        event.preventDefault();
        void toggleSave();
        return;
    }
  });
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const shortcut = shortcutOfEvent(event);
      if (shortcut) onShortcut(shortcut, event);
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  /** Runs one Library command; the shell answers with the Library as it now is. */
  async function change(command: () => Promise<Library | null>, after?: (library: Library) => void) {
    setBusy(true);
    setNotice(null);
    try {
      const updated = await command();
      if (updated) {
        setLibrary(updated);
        // Which headwords there are, and how many Dictionaries have each, may have changed.
        column.refresh(typed.current).catch(() => {});
        after?.(updated);
      }
    } catch (error) {
      setNotice(t.libraryActionFailed(String(error)));
    } finally {
      setBusy(false);
    }
  }

  /** Changes one Entry on the page, if it is still there. */
  function updateEntry(dictionaryId: string, change: Partial<Entry>) {
    setPage((current) =>
      current.kind === "found"
        ? { ...current, entries: current.entries.map((e) => (e.dictionaryId === dictionaryId ? { ...e, ...change } : e)) }
        : current,
    );
  }

  async function toggleNetwork(entry: Entry) {
    const allow = !entry.allowNetwork;
    try {
      await setAllowNetwork(entry.dictionaryId, allow);
    } catch {
      return; // the switch stays as it was: the preference was not saved
    }
    updateEntry(entry.dictionaryId, { allowNetwork: allow });
  }

  async function toggleAsAuthored(entry: Entry) {
    const asAuthored = !entry.asAuthored;
    try {
      await setAsAuthored(entry.dictionaryId, asAuthored);
    } catch {
      return;
    }
    updateEntry(entry.dictionaryId, { asAuthored });
  }

  const reading = {
    ...appearance,
    /** Every Dictionary at once, the ones on the page included. */
    setAllAsAuthored: async (asAuthored: boolean) => {
      await appearance.setAllAsAuthored(asAuthored);
      setPage((current) =>
        current.kind === "found" ? { ...current, entries: current.entries.map((e) => ({ ...e, asAuthored })) } : current,
      );
    },
  };

  const actions: LibraryActions = {
    addFolder: () => change(addLibraryFolder, () => setView("library")),
    removeFolder: (path) => change(() => removeLibraryFolder(path)),
    rescan: () => change(rescanLibrary),
    setEnabled: (id, enabled) => change(() => setDictionaryEnabled(id, enabled)),
    move: (id, direction) => change(() => moveDictionary(id, direction)),
    register: async (path, email, code) => {
      setBusy(true);
      setNotice(null);
      try {
        const updated = await registerDictionary(path, email, code);
        setLibrary(updated);
        const unlocked = updated.dictionaries.find((dictionary) => dictionary.path === path);
        if (unlocked) setNotice(t.unlocked(unlocked.title));
        return null;
      } catch (error) {
        // The form words the error beside the fields it is about.
        return error as RegisterError;
      } finally {
        setBusy(false);
      }
    },
    index: () => change(indexLibrary),
    createGroup: (name) => change(() => createGroup(name)),
    renameGroup: (id, name) => change(() => renameGroup(id, name)),
    deleteGroup: (id) => change(() => deleteGroup(id)),
    setInGroup: (group, dictionary, member) => change(() => setInGroup(group, dictionary, member)),
    moveInGroup: (group, dictionary, direction) => change(() => moveInGroup(group, dictionary, direction)),
    cancelIndexing: () => {
      cancelIndexing().catch((error: unknown) => setNotice(t.libraryActionFailed(String(error))));
    },
  };

  const groups = library?.groups ?? [];
  const group = groups.find((g) => g.id === currentGroup) ?? null;
  const answeringCount = library ? answering(library, group).length : 0;
  const asked = page.kind === "found" ? Math.max(answeringCount, page.entries.length) : answeringCount;
  const { items, selected, miss, near, lemma } = column.state;
  const note = lemma ? (
    <>
      {t.lemmatized} <b>{lemma.from}</b> → <b>{lemma.to}</b>
    </>
  ) : near ? (
    t.nearNote
  ) : miss && items.length > 0 ? (
    t.missNote
  ) : null;
  const columnSelected = list === "history" ? historySelected : list === "wordBook" ? wordBookSelected : selected;
  const bookWords: ColumnItem[] = (currentBook?.words ?? []).map((word) => ({ headword: word.headword }));
  const columnItems: ColumnItem[] =
    list === "history" ? history.map((headword) => ({ headword })) : list === "wordBook" ? bookWords : items;

  const foldedHere = page.kind === "found" && folded.page === pageKey(page) ? folded.ids : NONE_FOLDED;

  function toggleFold(dictionaryId: string) {
    if (page.kind !== "found") return;
    const ids = new Set(foldedHere);
    if (!ids.delete(dictionaryId)) ids.add(dictionaryId);
    setFolded({ page: pageKey(page), ids });
  }

  function foldAll(fold: boolean) {
    if (page.kind !== "found") return;
    setFolded({ page: pageKey(page), ids: new Set(fold ? page.entries.map((e) => e.dictionaryId) : []) });
  }

  /**
   * Opens the Library view at one of its settings sections, named by its heading, and puts
   * the focus on the control named `control` there (or its first control). The sections
   * load their settings themselves, so this waits a moment for the section to be there.
   */
  function reveal(section: string, control?: string) {
    setColumnOpen(false);
    setView("library");
    let frames = 0;
    const find = () => {
      const heading = [...(pageRef.current?.querySelectorAll("section > h2") ?? [])].find((h) => h.textContent === section);
      const target = heading?.parentElement?.querySelectorAll<HTMLElement>("button, input, select");
      const wanted = target && [...target].find((element) => !control || element.textContent?.trim() === control);
      if (heading && wanted) {
        heading.scrollIntoView?.({ block: "start" });
        wanted.focus();
      } else if (++frames < 120) {
        requestAnimationFrame(find);
      }
    };
    requestAnimationFrame(find);
  }

  /** Every action, for the command palette, as things stand now. */
  function commands(): Command[] {
    const found = view === "lookup" && page.kind === "found" ? page : null;
    const sound = found ? firstSound(found.entries) : null;
    const theme = appearance.appearance.theme;
    const list: Command[] = [{ label: t.cFocus, hint: hintOf({ kind: "focusSearch" }), run: focusSearch }];
    if (found) {
      list.push({ label: savedHere(found.headword) ? t.cUnsave : t.cSave, hint: hintOf({ kind: "save" }), run: () => void toggleSave() });
      if (sound !== null) list.push({ label: t.cPronounce, hint: hintOf({ kind: "pronounce" }), run: () => playSound(sound) });
    }
    if (canStep(trail, -1)) list.push({ label: t.back, hint: hintOf({ kind: "back" }), run: () => void go(-1) });
    if (canStep(trail, 1)) list.push({ label: t.forward, hint: hintOf({ kind: "forward" }), run: () => void go(1) });
    if (found) {
      found.entries.slice(0, 9).forEach((entry, index) => {
        const jump = { kind: "jump", index } as const;
        list.push({ label: t.cJump(entry.dictionaryTitle), hint: hintOf(jump), run: () => press(keysOf(jump)) });
      });
      list.push({ label: t.cFoldAll, run: () => foldAll(true) }, { label: t.cUnfoldAll, run: () => foldAll(false) });
    }
    list.push(
      view === "library"
        ? { label: t.cLookup, run: () => setView("lookup") }
        : { label: t.cLibrary, run: () => setView("library") },
      {
        label: t.cSuggestions,
        run: () => {
          setList("suggestions");
          setView("lookup");
        },
      },
      {
        label: t.cHistory,
        run: () => {
          setList("history");
          setView("lookup");
        },
      },
      { label: t.cWordBook, run: showWordBook },
    );
    if (currentBook) {
      const id = currentBook.id;
      list.push(
        { label: t.cExportCsv, run: () => void bookActions.export(id, "csv") },
        { label: t.cExportTsv, run: () => void bookActions.export(id, "tsv") },
      );
    }
    if (groups.length > 0) {
      list.push({ label: t.cAllDictionaries, checked: currentGroup === null, run: () => void switchGroup(null) });
      for (const g of groups) list.push({ label: t.cGroup(g.name), checked: g.id === currentGroup, run: () => void switchGroup(g.id) });
    }
    const system = window.matchMedia?.("(prefers-color-scheme: dark)").matches ? t.cDark : t.cLight;
    list.push(
      { label: t.cThemeSystem(system), checked: theme === "system", run: () => appearance.setTheme("system") },
      { label: t.cThemeLight, checked: theme === "light", run: () => appearance.setTheme("light") },
      { label: t.cThemeDark, checked: theme === "dark", run: () => appearance.setTheme("dark") },
      appearance.appearance.allAsAuthored
        ? { label: t.cRecolourAll, run: () => void reading.setAllAsAuthored(false).catch(() => {}) }
        : { label: t.cAuthoredAll, run: () => void reading.setAllAsAuthored(true).catch(() => {}) },
    );
    list.push({ label: t.cLanguage(t.languageSystem), checked: followsSystem, run: () => chooseLanguage(null) });
    for (const [language, name] of Object.entries(LANGUAGES) as [Locale, string][]) {
      list.push({ label: t.cLanguage(name), checked: !followsSystem && language === locale, run: () => chooseLanguage(language) });
    }
    const { zoom, textSize } = appearance.appearance;
    list.push(
      { label: t.zoomIn, hint: hintOf({ kind: "zoomIn" }), run: () => appearance.setZoom(zoomStep(zoom, 1)) },
      { label: t.zoomOut, hint: hintOf({ kind: "zoomOut" }), run: () => appearance.setZoom(zoomStep(zoom, -1)) },
      { label: t.zoomReset, hint: hintOf({ kind: "zoomReset" }), run: () => appearance.setZoom(100) },
    );
    if (textSize < TEXT_SIZES.max) list.push({ label: t.largerText, run: () => appearance.setTextSize(textSize + 1) });
    if (textSize > TEXT_SIZES.min) list.push({ label: t.smallerText, run: () => appearance.setTextSize(textSize - 1) });
    list.push(
      { label: t.cAddFolder, run: () => void actions.addFolder() },
      { label: t.cRescan, run: () => void actions.rescan() },
      { label: t.cMiniSettings, run: () => reveal(t.miniWindow) },
      { label: t.cMoveData, run: () => reveal(t.yourData, t.moveData) },
    );
    return list;
  }

  return (
    <div className="grid h-full grid-cols-[264px_minmax(0,1fr)] grid-rows-[48px_minmax(0,1fr)_26px] max-[860px]:grid-cols-[200px_minmax(0,1fr)] max-[620px]:grid-cols-[minmax(0,1fr)]">
      <header
        // On macOS the cover band is the title bar (the system's controls sit at its left).
        data-tauri-drag-region={platform === "macos" ? "deep" : undefined}
        className="col-span-full flex items-center bg-cover text-on-cover select-none"
      >
        <div className="flex w-[264px] flex-none items-center gap-sm pr-3.5 pl-lg max-[860px]:w-[200px] max-[620px]:w-auto max-[620px]:pr-1.5 macos:pl-20">
          <span className="font-serif text-[17px] font-bold max-[620px]:hidden">{t.appName}</span>
          <GroupSwitcher groups={groups} current={currentGroup} onSwitch={(id) => void switchGroup(id)} />
          <button
            type="button"
            aria-label={t.toggleColumn}
            aria-controls={COLUMN_ID}
            aria-expanded={columnOpen || typing}
            // Keep the caret in the search field: the toggle answers what is shown now.
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => {
              setTyping(false);
              setColumnOpen(!(columnOpen || typing));
            }}
            className="hidden size-6.5 place-items-center rounded-sharp text-on-cover-muted hover:bg-cover-edge hover:text-on-cover max-[620px]:grid"
          >
            <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
              <rect x="2.5" y="3" width="11" height="10" />
              <path d="M6.5 3v10" />
            </svg>
          </button>
        </div>
        <NavButtons canGoBack={canStep(trail, -1)} canGoForward={canStep(trail, 1)} onGo={(by) => void go(by)} />
        <div className="flex min-w-0 flex-1 pr-lg">
          <SearchField
            value={query}
            activeOption={view === "lookup" && columnSelected >= 0 ? optionId(columnSelected) : undefined}
            onChange={type}
            onQuery={(text) => void changeQuery(text)}
            onMove={(by) => void move(by)}
            onCommit={commitCurrent}
            onEscape={putColumnAway}
            onBlur={() => setTyping(false)}
            inputRef={searchRef}
            trailing={<PaletteHint onOpen={() => setPaletteOpen(true)} />}
          />
        </div>
      </header>

      <HeadwordColumn
        items={columnItems}
        selected={columnSelected}
        note={list === "suggestions" ? note : null}
        view={view}
        list={list}
        onPick={pick}
        onNeedMore={() => {
          if (list === "suggestions") void column.loadMore().catch(() => {});
        }}
        onShowSuggestions={() => {
          setList("suggestions");
          setView("lookup");
        }}
        onShowHistory={() => {
          setList("history");
          setView("lookup");
        }}
        onClearHistory={forgetHistory}
        onShowWordBook={showWordBook}
        onToggleLibrary={() => {
          setColumnOpen(false);
          setView(view === "library" ? "lookup" : "library");
        }}
        open={columnOpen || typing}
      />

      <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_auto] bg-paper">
        <main ref={pageRef} aria-label={t.pageLabel} className="min-h-0 overflow-y-auto pt-[18px] pr-[30px] pb-12 pl-xl">
          {view === "library" && library ? (
            <>
              <LibraryPage library={library} busy={busy} notice={notice} actions={actions} />
              <ReadingSettings controls={reading} />
              <MiniWindowSettings />
              <DataSettings />
            </>
          ) : view === "wordBook" ? (
            <WordBookPage wordBooks={wordBooks ?? { books: [], current: null }} busy={busy} notice={bookNotice} actions={bookActions} />
          ) : (
            <PageContent
              page={page}
              asked={asked}
              library={library}
              busy={busy}
              notice={notice}
              onAddFolder={actions.addFolder}
              reading={{ scheme: appearance.scheme, textSize: appearance.appearance.textSize }}
              onToggleNetwork={(entry) => void toggleNetwork(entry)}
              onToggleAsAuthored={(entry) => void toggleAsAuthored(entry)}
              onLookUp={followLink}
              onSearch={search}
              groups={groups}
              currentGroup={currentGroup}
              onGoTo={(id, headword) => void switchGroup(id, headword)}
              recent={
                <>
                  <RecentLookups words={history} onPick={(word) => void commit(word)} />
                  {currentBook && (
                    <WordList heading={currentBook.name} words={currentBook.words.map((word) => word.headword)} onPick={(word) => void commit(word)} />
                  )}
                </>
              }
              saved={page.kind === "found" && savedHere(page.headword)}
              onToggleSave={() => void toggleSave()}
              folded={foldedHere}
              onToggleFold={toggleFold}
            />
          )}
        </main>
        {view === "lookup" ? (
          <div className="w-[168px] min-h-0 overflow-hidden max-[1100px]:w-[46px]">
            {page.kind === "found" ? (
              <ThumbIndex
                key={pageKey(page)}
                notches={page.entries.map((e) => ({ dictionaryId: e.dictionaryId, title: e.dictionaryTitle }))}
                unanswered={asked - page.entries.length}
                page={pageRef}
              />
            ) : null}
          </div>
        ) : null}
      </div>

      <footer className="flex items-center bg-paper gap-[18px] overflow-hidden border-t border-rule px-xl text-label whitespace-nowrap text-text-secondary tabular-nums select-none">
        {group ? <span>{t.currentGroupIs(group.name)}</span> : null}
        <span>{library && !library.problem ? t.libraryCount(answeringCount) : null}</span>
        {said ? (
          <span role="status" className="min-w-0 truncate">
            {said}
          </span>
        ) : null}
        <UpdateNotice />
      </footer>
      {paletteOpen ? <CommandPalette commands={commands} onClose={() => setPaletteOpen(false)} /> : null}
    </div>
  );
}

function PageContent({
  page,
  asked,
  library,
  busy,
  notice,
  onAddFolder,
  reading,
  onToggleNetwork,
  onToggleAsAuthored,
  onLookUp,
  onSearch,
  groups,
  currentGroup,
  onGoTo,
  recent,
  saved,
  onToggleSave,
  folded,
  onToggleFold,
}: {
  page: PageState;
  /** How many Dictionaries the Lookup ran against. */
  asked: number;
  library: Library | null;
  busy: boolean;
  notice: string | null;
  onAddFolder: () => void;
  reading: Reading;
  onToggleNetwork: (entry: Entry) => void;
  onToggleAsAuthored: (entry: Entry) => void;
  onLookUp: (headword: string) => void;
  onSearch: (text: string) => void;
  groups: Group[];
  currentGroup: string | null;
  /** Makes `group` current (null: the whole Library) and looks `headword` up there. */
  onGoTo: (group: string | null, headword: string) => void;
  /** Recent lookups, offered on the empty page. */
  recent: ReactNode;
  /** Whether the word on the page is in the chosen Word Book. */
  saved: boolean;
  onToggleSave: () => void;
  /** The Dictionaries folded on the page. */
  folded: ReadonlySet<string>;
  onToggleFold: (dictionaryId: string) => void;
}) {
  const { t } = useI18n();
  switch (page.kind) {
    case "found":
      return (
        <EntryPage
          key={pageKey(page)}
          headword={page.headword}
          entries={page.entries}
          asked={asked}
          reading={reading}
          onToggleNetwork={onToggleNetwork}
          onToggleAsAuthored={onToggleAsAuthored}
          onLookUp={onLookUp}
          saved={saved}
          onToggleSave={onToggleSave}
          folded={folded}
          onToggleFold={onToggleFold}
        />
      );
    case "missing":
      return (
        <NoMatch
          query={page.query}
          next={page.next}
          onSearch={onSearch}
          groups={groups}
          currentGroup={currentGroup}
          onGoTo={onGoTo}
        />
      );
    case "failed":
      return <Lead>{t.lookupFailed(page.reason)}</Lead>;
    case "idle":
      if (library?.problem) return <Lead>{t.libraryUnavailable(library.problem)}</Lead>;
      if (library && library.dictionaries.length === 0) {
        return (
          <EmptyLibrary
            action={
              <span className="flex items-center gap-sm">
                <OutlineButton onClick={onAddFolder} disabled={busy}>
                  {t.addFolder}
                </OutlineButton>
                <span role="status" className="text-label text-text-secondary">
                  {busy ? t.working : notice}
                </span>
              </span>
            }
          />
        );
      }
      return (
        <>
          <Lead muted>{t.startHint}</Lead>
          {recent}
        </>
      );
  }
}

/** What the page needs to know of the reader's appearance settings. */
interface Reading {
  scheme: Scheme;
  textSize: number;
}

/**
 * The page head and one section per answering Dictionary. `asked` is how many Dictionaries
 * the Lookup ran against. An `entry://` link in any Entry looks its headword up; a
 * `sound://` link plays from its own Dictionary.
 */
function EntryPage({
  headword,
  entries,
  asked,
  onLookUp,
  reading,
  onToggleNetwork,
  onToggleAsAuthored,
  saved,
  onToggleSave,
  folded,
  onToggleFold,
}: {
  headword: string;
  entries: Entry[];
  asked: number;
  reading: Reading;
  onToggleNetwork: (entry: Entry) => void;
  onToggleAsAuthored: (entry: Entry) => void;
  onLookUp: (headword: string) => void;
  saved: boolean;
  onToggleSave: () => void;
  folded: ReadonlySet<string>;
  onToggleFold: (dictionaryId: string) => void;
}) {
  const { t } = useI18n();
  const dark = reading.scheme === "dark";
  const pronunciation = entries.find((entry) => entry.pronunciation)?.pronunciation;
  const sound = firstSound(entries);

  // Alt+P: the Pronounce action, wherever focus is in the window (an Entry frame included,
  // which reports the keys pressed in it).
  useEffect(() => {
    if (sound === null) return;
    const url = sound;
    function onKeyDown(event: KeyboardEvent) {
      if (shortcutOfEvent(event)?.kind !== "pronounce") return;
      event.preventDefault();
      playSound(url);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [sound]);

  return (
    <article>
      <div className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
        <h1 className="-ml-3.5 font-serif text-display font-bold [overflow-wrap:anywhere]">【{headword}】</h1>
        {pronunciation ? <span className="font-serif text-[16px] text-text-secondary">{pronunciation}</span> : null}
        <span className="text-[12px] text-text-secondary">{t.answeredOf(entries.length, asked)}</span>
        <div className="ml-auto flex gap-1.5 self-center">
          {sound !== null && <PronounceButton onClick={() => playSound(sound)} />}
          <SaveButton saved={saved} onToggle={onToggleSave} />
        </div>
      </div>
      {entries.map((entry) => {
        const open = !folded.has(entry.dictionaryId);
        // In the dark theme an Entry is recoloured, or keeps its own white page as authored.
        const whitePage = dark && entry.asAuthored;
        const body = `${sectionId(entry.dictionaryId)}-body`;
        return (
          <section
            key={entry.dictionaryId}
            id={sectionId(entry.dictionaryId)}
            tabIndex={-1}
            aria-label={entry.dictionaryTitle}
            className="scroll-mt-3 border-rule outline-offset-4 [&+&]:border-t"
          >
            <div className="flex h-9 items-center gap-sm">
              <h2 className="-ml-[7px] min-w-0 flex-1 truncate text-title font-bold">【{entry.dictionaryTitle}】</h2>
              {dark ? <AsAuthoredSwitch on={entry.asAuthored} onToggle={() => onToggleAsAuthored(entry)} /> : null}
              <NetworkSwitch allowed={entry.allowNetwork} onToggle={() => onToggleNetwork(entry)} />
              <FoldButton open={open} controls={body} onToggle={() => onToggleFold(entry.dictionaryId)} />
            </div>
            {/* Folded, the Entry stays loaded out of sight, so unfolding shows it as it was. */}
            <div id={body} hidden={!open} className="pb-3.5">
              <div className={whitePage ? "max-w-[860px] rounded-sharp bg-india-paper px-3 pt-2.5 pb-3" : undefined}>
                {/* A new frame when the switch changes, so the document reloads under its new policy. */}
                <EntryFrame
                  key={String(entry.allowNetwork)}
                  dictionaryTitle={entry.dictionaryTitle}
                  src={entry.src}
                  onLink={onLookUp}
                  onSound={playSound}
                  textSize={reading.textSize}
                  recolour={dark && !entry.asAuthored}
                />
              </div>
            </div>
          </section>
        );
      })}
    </article>
  );
}

const NONE_FOLDED: ReadonlySet<string> = new Set();

/** The command palette's hint at the right end of the search field: a click opens it too. */
function PaletteHint({ onOpen }: { onOpen: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      aria-keyshortcuts={MOD === "⌘" ? "Meta+K" : "Control+K"}
      // Keep the caret in the search field until the palette takes it.
      onMouseDown={(event) => event.preventDefault()}
      onClick={onOpen}
      className="flex h-6 flex-none items-center gap-1 rounded-sharp px-1 text-[12px] whitespace-nowrap text-field-text-secondary hover:bg-field-hover hover:text-field-text"
    >
      <kbd aria-hidden="true" className="rounded-sharp border border-field-text-secondary/40 px-[5px] font-sans text-[11px] leading-4">
        {MOD}
      </kbd>
      <kbd aria-hidden="true" className="rounded-sharp border border-field-text-secondary/40 px-[5px] font-sans text-[11px] leading-4">
        K
      </kbd>
      <span className="max-[620px]:sr-only">{t.commands}</span>
    </button>
  );
}

/** The URL of the first audio the Entries offer, in page order, or null. */
function firstSound(entries: Entry[]): string | null {
  for (const entry of entries) {
    const url = entry.sound ? soundUrl(entry.src, entry.sound) : null;
    if (url !== null) return url;
  }
  return null;
}

function PronounceButton({ onClick }: { onClick: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      onClick={onClick}
      aria-keyshortcuts="Alt+P"
      className="flex h-7 items-center gap-1.5 rounded-sharp border border-rule px-2.5 text-body text-text hover:bg-shade active:translate-y-px"
    >
      <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
        <path d="M2.5 6v4H5l3.5 3V3L5 6z" />
        <path d="M11 5.5a3.5 3.5 0 0 1 0 5" />
      </svg>
      <span>{t.pronounce}</span>
      <kbd aria-hidden="true" className="font-sans text-[11px] text-text-secondary max-[620px]:hidden">
        Alt P
      </kbd>
    </button>
  );
}

/**
 * The page when nothing was found: it says so, offers another Group that has the word,
 * suggests a wildcard pattern, and lists the headwords that follow where the word would
 * stand, so a miss is never a dead end.
 */
function NoMatch({
  query,
  next,
  onSearch,
  groups,
  currentGroup,
  onGoTo,
}: {
  query: string;
  next: Suggestion[];
  onSearch: (text: string) => void;
  groups: Group[];
  currentGroup: string | null;
  onGoTo: (group: string | null, headword: string) => void;
}) {
  const { t } = useI18n();
  const pattern = wildcardHint(query);
  const [found, setFound] = useState<{ query: string; group: string | null; where: Elsewhere | null } | null>(null);
  useEffect(() => {
    let stale = false;
    elsewhere(query).then(
      (where) => {
        if (!stale) setFound({ query, group: currentGroup, where });
      },
      () => {}, // nowhere to point to
    );
    return () => {
      stale = true;
    };
  }, [query, currentGroup]);
  // Only the answer for this query in this Group counts.
  const where = found && found.query === query && found.group === currentGroup ? found.where : null;
  const whereName = where ? (where.group === null ? t.wholeLibrary : groups.find((g) => g.id === where.group)?.name) : undefined;
  return (
    <article>
      <div className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
        <h1 className="-ml-3.5 font-serif text-display font-bold [overflow-wrap:anywhere]">【{query}】</h1>
        <span className="text-[12px] text-text-secondary">{t.notFoundMeta}</span>
      </div>
      <Lead>{t.notFound(query)}</Lead>
      <div className="flex max-w-[860px] flex-wrap items-center gap-x-2.5 gap-y-sm pb-xs text-text-secondary">
        {where && whereName !== undefined ? (
          <OutlineButton onClick={() => onGoTo(where.group, where.headword)}>
            {t.inGroupElsewhere(whereName, where.headword)}
          </OutlineButton>
        ) : null}
        <span>{t.wildcardTip}</span>
        <OutlineButton onClick={() => onSearch(pattern)}>
          {t.tryPattern} <b className="font-serif text-[14px]">{pattern}</b>
        </OutlineButton>
      </div>
      {next.length > 0 ? (
        <>
          <h2 id="next-in-index" className="mt-5 mb-1.5 text-[13px] font-bold">
            {t.nextInIndex}
          </h2>
          <ul aria-labelledby="next-in-index" className="columns-[150px] gap-x-7 [column-rule:1px_solid_var(--hairline)] pt-0.5 pb-1.5">
            {next.slice(0, 40).map((item) => (
              <li key={item.headword}>
                <button
                  type="button"
                  lang={/[぀-ヿ]/.test(item.headword) ? "ja" : undefined}
                  onClick={() => onSearch(item.headword)}
                  className="-ml-1.5 block h-[26px] w-full truncate rounded-sharp px-1.5 text-left font-serif text-index hover:bg-shade"
                >
                  {item.headword}
                </button>
              </li>
            ))}
          </ul>
        </>
      ) : null}
    </article>
  );
}

/** Add to Word Book, in the page head: solid ink once the word is saved (DESIGN.md: toggled on). */
function SaveButton({ saved, onToggle }: { saved: boolean; onToggle: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      aria-pressed={saved}
      aria-keyshortcuts={MOD === "⌘" ? "Meta+D" : "Control+D"}
      onClick={onToggle}
      className="flex h-7 flex-none items-center gap-1.5 rounded-sharp border border-rule px-2.5 text-body text-text hover:bg-shade active:translate-y-px aria-pressed:bg-text aria-pressed:text-paper"
    >
      <svg className="size-3.5 fill-none stroke-current stroke-[1.3]" viewBox="0 0 14 14" aria-hidden="true">
        <path d="M3.5 1.5h7v11L7 9.8l-3.5 2.7z" fill={saved ? "currentColor" : "none"} />
      </svg>
      <span>{saved ? t.inWordBook : t.addToWordBook}</span>
      <kbd aria-hidden="true" className="font-sans text-label text-current opacity-70">
        {MOD} D
      </kbd>
    </button>
  );
}

/** The dark theme's switch in the section header: pressed when the Entry keeps its own white page. */
function AsAuthoredSwitch({ on, onToggle }: { on: boolean; onToggle: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      aria-pressed={on}
      aria-label={t.asAuthored}
      title={on ? t.shownAsAuthored : t.shownRecoloured}
      onClick={onToggle}
      className="grid size-6.5 flex-none place-items-center rounded-sharp text-text-secondary hover:bg-shade hover:text-text aria-pressed:bg-text aria-pressed:text-paper"
    >
      <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
        <path d="M4 2.5h5.5L12 5v8.5H4z" />
        <path d="M9.5 2.5V5H12" />
      </svg>
    </button>
  );
}

/** An icon button in the section header: pressed when the Dictionary may use the network. */
function NetworkSwitch({ allowed, onToggle }: { allowed: boolean; onToggle: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      aria-pressed={allowed}
      aria-label={t.allowNetwork}
      title={allowed ? t.networkAllowed : t.networkBlocked}
      onClick={onToggle}
      className="grid size-6.5 flex-none place-items-center rounded-sharp text-text-secondary hover:bg-shade hover:text-text aria-pressed:bg-text aria-pressed:text-paper"
    >
      <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="5.5" />
        <path d="M2.5 8h11M8 2.5c-1.8 1.6-2.6 3.4-2.6 5.5s.8 3.9 2.6 5.5c1.8-1.6 2.6-3.4 2.6-5.5S9.8 4.1 8 2.5Z" />
      </svg>
    </button>
  );
}

/** The chevron at the right of a section header: points down while the Entry shows. */
function FoldButton({ open, controls, onToggle }: { open: boolean; controls: string; onToggle: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      aria-expanded={open}
      aria-controls={controls}
      aria-label={open ? t.collapse : t.expand}
      onClick={onToggle}
      className="grid size-6.5 flex-none place-items-center rounded-sharp text-text-secondary hover:bg-shade hover:text-text"
    >
      <svg
        className={`size-4 fill-none stroke-current stroke-[1.2] motion-safe:transition-[rotate] motion-safe:duration-180 motion-safe:ease-out-expo ${open ? "" : "-rotate-90"}`}
        viewBox="0 0 16 16"
        aria-hidden="true"
      >
        <path d="M4 6l4 4 4-4" />
      </svg>
    </button>
  );
}

function Lead({ children, muted = false }: { children: ReactNode; muted?: boolean }) {
  return <p className={`max-w-[860px] py-lg text-[14px] ${muted ? "text-text-secondary" : ""}`}>{children}</p>;
}

/** The enabled Dictionaries Lookups run against: the Group's, or the whole Library's. */
function answering(library: Library, group: Group | null) {
  const enabled = library.dictionaries.filter((d) => d.enabled);
  return group ? enabled.filter((d) => group.dictionaries.includes(d.id)) : enabled;
}

/** A found page is new when its word or its Group is: its frames and notches start afresh. */
function pageKey(page: { headword: string; group: string | null }) {
  return `${page.group ?? ""}\u0000${page.headword}`;
}

/** History as the shell sent it; anything but a list of words counts as none. */
function asWords(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((word): word is string => typeof word === "string") : [];
}
