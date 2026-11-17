import { invoke } from "@tauri-apps/api/core";

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
  /** How the headword is said, as this Entry gives it, if it shows that plainly (plain text). */
  pronunciation: string | null;
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
  /** Why the Library could not be loaded at all, if it could not. */
  problem: string | null;
}

export function lookUp(headword: string): Promise<Entry[]> {
  return invoke<Entry[]>("look_up", { headword });
}

export function suggest(query: string, limit: number): Promise<Suggestions> {
  return invoke<Suggestions>("suggest", { query, limit });
}

/** The headword column continued: up to `limit` headwords after `headword` in index order. */
export function headwordsAfter(headword: string, limit: number): Promise<Suggestion[]> {
  return invoke<Suggestion[]>("headwords_after", { headword, limit });
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
