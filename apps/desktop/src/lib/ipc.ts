import { invoke } from "@tauri-apps/api/core";

// Mirrors of the shapes the Tauri commands return (apps/desktop/src-tauri/src/commands.rs).

export interface Entry {
  /** Stable identifier of the Dictionary this Entry came from. */
  dictionaryId: string;
  /** The Dictionary's title, from its header. */
  dictionaryTitle: string;
  /** The Entry document's URL on the Entry scheme. Its HTML never passes through the interface. */
  src: string;
  /** Whether the reader allowed this Dictionary network access. */
  allowNetwork: boolean;
}

export type LibraryStatus =
  | { kind: "unconfigured"; configFile: string }
  | { kind: "unavailable"; folder: string | null; reason: string }
  | { kind: "ready"; folder: string; dictionaryCount: number };

export function lookUp(headword: string): Promise<Entry[]> {
  return invoke<Entry[]>("look_up", { headword });
}

export function libraryStatus(): Promise<LibraryStatus> {
  return invoke<LibraryStatus>("library_status");
}

export function setAllowNetwork(dictionaryId: string, allow: boolean): Promise<void> {
  return invoke<void>("set_allow_network", { dictionaryId, allow });
}
