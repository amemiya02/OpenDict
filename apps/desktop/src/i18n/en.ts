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
  allowNetwork: "Allow network access",
  networkAllowed: "This dictionary may use the network. Click to block it.",
  networkBlocked: "This dictionary is kept off the network. Click to allow it.",
};

export type Catalogue = typeof en;
