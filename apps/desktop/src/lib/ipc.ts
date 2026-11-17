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
  /** The first audio the Entry offers (the path of its first `sound://` link), for Pronounce. */
  pronunciation: string | null;
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
  | { kind: "unsupported"; detail: string };

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

export function setAllowNetwork(dictionaryId: string, allow: boolean): Promise<void> {
  return invoke<void>("set_allow_network", { dictionaryId, allow });
}
