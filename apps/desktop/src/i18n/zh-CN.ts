import type { Catalogue } from "./en";

export const zhCN = {
  appName: "OpenDict",
  searchLabel: "输入要查的词",
  searchPlaceholder: "输入要查的词",
  lookUp: "查词",
  pageLabel: "词条",
  entryFrameTitle: (dictionary: string) => `${dictionary} 的词条`,
  answered: (n: number) => `${n} 本词典有此词条`,
  notFound: (query: string) => `没有找到「${query}」。`,
  noLibraryFolder: "还没有设置词库文件夹。",
  noLibraryFolderHow: (file: string) => `请在 ${file} 里设置 "libraryFolder"，然后重新启动 OpenDict。`,
  libraryUnavailable: (reason: string) => `无法打开词库：${reason}`,
  libraryEmpty: (folder: string) => `在 ${folder} 里没有找到词典。`,
  libraryCount: (n: number) => `${n} 本词典`,
  startHint: "输入词头，按 Enter 查词。",
  lookupFailed: (reason: string) => `查词失败：${reason}`,
  allowNetwork: "允许访问网络",
  networkAllowed: "此词典可以访问网络。点击以禁止。",
  networkBlocked: "此词典无法访问网络。点击以允许。",
} satisfies Catalogue;
