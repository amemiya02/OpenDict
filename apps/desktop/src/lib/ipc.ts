import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// Mirrors of the shapes the Tauri commands return (apps/desktop/src-tauri/src/state.rs).

export interface Entry {
  /** The key of the Dictionary this Entry came from. */
  dictionaryId: string;
  /** The Dictionary's title, from its header. */
  dictionaryTitle: string;
  /** The Entry document's URL on the Entry scheme. Its HTML never passes through the interface. */
  src: string;
  /** Whether the reader allowed this Dictionary network access. */
  allowNetwork: boolean;
  /** The first audio the Entry offers (the path of its first `sound://` link), for Pronounce. */
  sound: string | null;
  /** How the headword is said, as this Entry gives it, if it shows that plainly (plain text). */
  pronunciation: string | null;
  /** In the dark theme, whether the Entry keeps its Dictionary's own white page rather than being recoloured. */
  asAuthored: boolean;
}

export type Theme = "system" | "light" | "dark";

/** How the page reads; the shell keeps it in the reader's preferences. */
export interface Appearance {
  theme: Theme;
  /** In the dark theme, every Dictionary shows as authored unless the reader chose otherwise for it. */
  allAsAuthored: boolean;
  /** The base text size of Entries, in CSS pixels: the size of text a Dictionary does not size itself. */
  textSize: number;
  /** Page zoom, in percent. */
  zoom: number;
}

/** One headword in the headword column. */
export interface Suggestion {
  headword: string;
  /** How many enabled Dictionaries file the headword. */
  count: number;
}

/** What the headword column offers for a query (the shape settled by design/prototype/core.js). */
export interface Suggestions {
  /** In index order: the matches, then the headwords that follow them. */
  items: Suggestion[];
  /** The query was an inflected form of `to`. */
  lemma: { from: string; to: string } | null;
  /** No exact or prefix match; the items are close spellings. */
  near: boolean;
  /** Nothing matched; the items are the headwords after where the query would stand. */
  miss: boolean;
  /** The query was empty; the items are the headword index from its start. */
  browse: boolean;
}

export interface LibraryFolder {
  path: string;
  dictionaryCount: number;
  /** Why the folder could not be read, if it could not. */
  problem: string | null;
}

export interface Dictionary {
  /** Stays the same across rescans and restarts. */
  id: string;
  title: string;
  path: string;
  entryCount: number;
  /** Bytes, all of the Dictionary's files together. */
  size: number;
  enabled: boolean;
  /** Whether its index is built; only an indexed Dictionary answers Lookups. */
  index: IndexState;
}

export type IndexState = { state: "ready" } | { state: "pending" } | { state: "failed"; detail: string };

/** The Dictionary being indexed right now. */
export interface Indexing {
  dictionaryId: string;
  title: string;
  /** Headwords read so far, out of `total`. */
  done: number;
  total: number;
  /** Dictionaries waiting after this one. */
  waiting: number;
}

export type FailureReason =
  | { kind: "unreadable"; detail: string }
  | { kind: "notADictionary" }
  | { kind: "unsupportedVersion"; detail: string }
  | { kind: "damaged" }
  | { kind: "needsRegistrationCode" }
  /** A registration code was kept for it, but no longer unlocks it. */
  | { kind: "wrongRegistrationCode" }
  | { kind: "unsupported"; detail: string };

/** Why a registration code was not accepted. */
export type RegisterError =
  | { kind: "wrongCode" }
  | { kind: "codeNeeded" }
  | { kind: "failed"; detail: string };

export interface OpenFailure {
  path: string;
  reason: FailureReason;
}

export interface Library {
  folders: LibraryFolder[];
  /** Every Dictionary, enabled or not, in Library order. */
  dictionaries: Dictionary[];
  failures: OpenFailure[];
  /** The Dictionary being indexed right now, if any. */
  indexing: Indexing | null;
  /** Why the Library could not be loaded at all, if it could not. */
  problem: string | null;
  /** The reader's Groups, in the order they were created. */
  groups: Group[];
  /** The key of the Group Lookups run against; null for the whole Library. */
  currentGroup: string | null;
}

/** A named, ordered subset of the Library. */
export interface Group {
  /** Stays the same when the Group is renamed. */
  id: string;
  name: string;
  /** Keys of its Dictionaries, in Group order. */
  dictionaries: string[];
}

/** Another Group (null: the whole Library) that has a word the Current Group lacks. */
export interface Elsewhere {
  group: string | null;
  /** The headword it offers for what the reader typed. */
  headword: string;
}

/** The window's native chrome, as the shell built it (apps/desktop/src-tauri/src/window.rs). */
export interface WindowChrome {
  platform: "windows" | "macos" | "linux";
  /** The system material behind the window (Mica, macOS vibrancy), if one took. */
  material: "mica" | "vibrancy" | null;
}

export function windowChrome(): Promise<WindowChrome> {
  return invoke("window_chrome");
}

export function lookUp(headword: string): Promise<Entry[]> {
  return invoke<Entry[]>("look_up", { headword });
}

export function suggest(query: string, limit: number): Promise<Suggestions> {
  return invoke<Suggestions>("suggest", { query, limit });
}

/**
 * The headword column for `query` continued: up to `limit` more suggestions after
 * `headword`. A pattern's column continues with its matches, any other through the index.
 */
export function headwordsAfter(query: string, headword: string, limit: number): Promise<Suggestion[]> {
  return invoke<Suggestion[]>("headwords_after", { query, headword, limit });
}

export function getLibrary(): Promise<Library> {
  return invoke<Library>("library");
}

/** Opens the system folder dialog; resolves to null when the reader cancels it. */
export function addLibraryFolder(): Promise<Library | null> {
  return invoke<Library | null>("add_library_folder");
}

export function removeLibraryFolder(path: string): Promise<Library> {
  return invoke<Library>("remove_library_folder", { path });
}

export function rescanLibrary(): Promise<Library> {
  return invoke<Library>("rescan_library");
}

export function setDictionaryEnabled(id: string, enabled: boolean): Promise<Library> {
  return invoke<Library>("set_dictionary_enabled", { id, enabled });
}

/** Indexes the Dictionaries still waiting for their index. */
export function indexLibrary(): Promise<Library> {
  return invoke<Library>("index_library");
}

/** Stops indexing; Dictionaries not yet indexed wait until indexing starts again. */
export function cancelIndexing(): Promise<void> {
  return invoke<void>("cancel_indexing");
}

/** Calls `handler` with the Library whenever the shell reports a change (indexing progress). */
export function onLibraryChanged(handler: (library: Library) => void): Promise<UnlistenFn> {
  return listen<Library>("library", (event) => handler(event.payload));
}

export function moveDictionary(id: string, direction: "up" | "down"): Promise<Library> {
  return invoke<Library>("move_dictionary", { id, direction });
}

/**
 * Unlocks a Dictionary that needs a registration code. `code` may be empty when the
 * Dictionary carries its own. Rejects with a RegisterError when they do not unlock it.
 */
export async function registerDictionary(path: string, email: string, code: string): Promise<Library> {
  try {
    return await invoke<Library>("register_dictionary", { path, email, code });
  } catch (error) {
    const typed = error as Partial<RegisterError> | null;
    if (typed && typeof typed === "object" && typeof typed.kind === "string") throw typed as RegisterError;
    throw { kind: "failed", detail: String(error) } satisfies RegisterError;
  }
}

export function setAllowNetwork(dictionaryId: string, allow: boolean): Promise<void> {
  return invoke<void>("set_allow_network", { dictionaryId, allow });
}

export function getAppearance(): Promise<Appearance> {
  return invoke<Appearance>("appearance");
}

/** Saves the theme; the shell also gives it to the windows. */
export function setTheme(theme: Theme): Promise<void> {
  return invoke<void>("set_theme", { theme });
}

export function setAsAuthored(dictionaryId: string, asAuthored: boolean): Promise<void> {
  return invoke<void>("set_as_authored", { dictionaryId, asAuthored });
}

/** Shows every Dictionary as authored, or recoloured, in the dark theme, replacing choices for single ones. */
export function setAllAsAuthored(asAuthored: boolean): Promise<void> {
  return invoke<void>("set_all_as_authored", { asAuthored });
}

/** Resolves to the size as saved, within the shell's limits. */
export function setTextSize(size: number): Promise<number> {
  return invoke<number>("set_text_size", { size });
}

/** Zooms the window's page; resolves to the zoom as saved, within the shell's limits. */
export function setZoom(percent: number): Promise<number> {
  return invoke<number>("set_zoom", { percent });
}

/** A Headword saved in a Word Book. */
export interface SavedWord {
  headword: string;
  /** The Dictionary the reader was reading when they saved it. */
  dictionaryId: string;
  dictionaryTitle: string;
  /** A note or example sentence. */
  note: string | null;
  /** When it was saved, in milliseconds since 1970 (UTC). */
  added: number;
}

export interface WordBook {
  /** Stays the same whatever the Word Book is renamed to. */
  id: number;
  name: string;
  /** Newest first. */
  words: SavedWord[];
}

export interface WordBooks {
  /** In the order they were made. */
  books: WordBook[];
  /** The Word Book Ctrl/⌘ + D saves into, or null while there are none. */
  current: number | null;
}

export type ExportFormat = "csv" | "tsv";

export function getWordBooks(): Promise<WordBooks> {
  return invoke<WordBooks>("word_books");
}

/**
 * Saves the word to the chosen Word Book, or takes it out if it is there. `newBookName`
 * names the Word Book made for the very first word.
 */
export function toggleWord(headword: string, dictionaryId: string, dictionaryTitle: string, newBookName: string): Promise<WordBooks> {
  return invoke<WordBooks>("toggle_word", { headword, dictionaryId, dictionaryTitle, newBookName });
}

export function createWordBook(name: string): Promise<WordBooks> {
  return invoke<WordBooks>("create_word_book", { name });
}

export function renameWordBook(id: number, name: string): Promise<WordBooks> {
  return invoke<WordBooks>("rename_word_book", { id, name });
}

export function deleteWordBook(id: number): Promise<WordBooks> {
  return invoke<WordBooks>("delete_word_book", { id });
}

/** Makes the Word Book the one new words go into. */
export function chooseWordBook(id: number): Promise<WordBooks> {
  return invoke<WordBooks>("choose_word_book", { id });
}

export function removeSavedWord(id: number, headword: string): Promise<WordBooks> {
  return invoke<WordBooks>("remove_saved_word", { id, headword });
}

/** Sets a saved word's note; an empty note removes it. */
export function setWordNote(id: number, headword: string, note: string): Promise<WordBooks> {
  return invoke<WordBooks>("set_word_note", { id, headword, note });
}

/** Asks where to save with the system dialog and exports there; resolves to the file, or null when cancelled. */
export function exportWordBook(id: number, format: ExportFormat): Promise<string | null> {
  return invoke<string | null>("export_word_book", { id, format });
}

/** The headwords of committed Lookups, newest first. */
export function getHistory(): Promise<string[]> {
  return invoke<string[]>("history");
}

/** Records a committed Lookup (Enter, a click, a followed link); resolves to History as it now is. */
export function recordLookup(headword: string): Promise<string[]> {
  return invoke<string[]>("record_lookup", { headword });
}

export function clearHistory(): Promise<void> {
  return invoke<void>("clear_history");
}

export function createGroup(name: string): Promise<Library> {
  return invoke<Library>("create_group", { name });
}

export function renameGroup(id: string, name: string): Promise<Library> {
  return invoke<Library>("rename_group", { id, name });
}

/** Deletes a Group; its Dictionaries stay in the Library. */
export function deleteGroup(id: string): Promise<Library> {
  return invoke<Library>("delete_group", { id });
}

/** Puts a Dictionary into a Group (at the end of its order) or takes it out. */
export function setInGroup(group: string, dictionary: string, member: boolean): Promise<Library> {
  return invoke<Library>("set_in_group", { group, dictionary, member });
}

export function moveInGroup(group: string, dictionary: string, direction: "up" | "down"): Promise<Library> {
  return invoke<Library>("move_in_group", { group, dictionary, direction });
}

/** Makes `group` the Current Group; null makes Lookups run against the whole Library. */
export function setCurrentGroup(group: string | null): Promise<Library> {
  return invoke<Library>("set_current_group", { group });
}

/** Another Group that has `query` when the Current Group does not, or null. */
export function elsewhere(query: string): Promise<Elsewhere | null> {
  return invoke<Elsewhere | null>("elsewhere", { query });
}

/** Where the reader's data (History, Word Books, settings, indexes) is kept. */
export interface DataDirectory {
  path: string;
  /** Whether it is where a new reader's data starts. */
  isDefault: boolean;
  /** Why it cannot be used, if it cannot (it may be on a drive that is not connected). */
  problem: string | null;
}

/** Why the data directory did not move; the data stays where it was. */
export type DataDirectoryError = { kind: "holdsData"; detail: string } | { kind: "failed"; detail: string };

export function getDataDirectory(): Promise<DataDirectory> {
  return invoke<DataDirectory>("data_directory");
}

/** Asks the reader for a folder with the system dialog; null when they cancel. */
export function chooseDataFolder(): Promise<string | null> {
  return invoke<string | null>("choose_data_folder");
}

/** Moves the reader's data into `path` and uses it from then on. Rejects with a DataDirectoryError. */
export async function moveDataDirectory(path: string): Promise<DataDirectory> {
  try {
    return await invoke<DataDirectory>("move_data_directory", { path });
  } catch (error) {
    throw dataDirectoryError(error);
  }
}

/** Uses the data already in `path` (or starts afresh there) from then on. Rejects with a DataDirectoryError. */
export async function switchDataDirectory(path: string): Promise<DataDirectory> {
  try {
    return await invoke<DataDirectory>("use_data_directory", { path });
  } catch (error) {
    throw dataDirectoryError(error);
  }
}

function dataDirectoryError(error: unknown): DataDirectoryError {
  const typed = error as Partial<DataDirectoryError> | null;
  if (typed && typeof typed === "object" && (typed.kind === "holdsData" || typed.kind === "failed")) return typed as DataDirectoryError;
  return { kind: "failed", detail: String(error) };
}
