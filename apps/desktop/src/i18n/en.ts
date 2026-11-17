// The English catalogue is the reference: every other catalogue must have the same keys.
export const en = {
  appName: "OpenDict",
  searchLabel: "Type a word",
  searchPlaceholder: "Type a word",
  lookUp: "Look up",
  pageLabel: "Entries",
  entryFrameTitle: (dictionary: string) => `Entry from ${dictionary}`,
  answered: (n: number) => (n === 1 ? "in 1 dictionary" : `in ${n} dictionaries`),
  notFound: (query: string) => `Nothing found for “${query}”.`,
  noLibraryFolder: "No Library Folder is configured yet.",
  noLibraryFolderHow: (file: string) => `Set "libraryFolder" in ${file}, then restart OpenDict.`,
  libraryUnavailable: (reason: string) => `The Library could not be opened: ${reason}`,
  libraryEmpty: (folder: string) => `No dictionaries were found in ${folder}.`,
  libraryCount: (n: number) => (n === 1 ? "1 dictionary" : `${n} dictionaries`),
  startHint: "Type a headword and press Enter.",
  lookupFailed: (reason: string) => `The lookup failed: ${reason}`,
  updateAvailable: (version: string) => `OpenDict ${version} is available.`,
  updateInstall: "Install and restart",
  updateDownloading: (version: string, percent: number | null) =>
    percent === null ? `Downloading OpenDict ${version}…` : `Downloading OpenDict ${version}… ${percent}%`,
  updateInstalling: (version: string) => `Installing OpenDict ${version}…`,
  updateFailed: (version: string, reason: string) => `OpenDict ${version} could not be installed: ${reason}`,
};

export type Catalogue = typeof en;
