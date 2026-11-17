import type { Catalogue } from "./en";

export const ja = {
  appName: "OpenDict",
  searchLabel: "調べたい語を入力",
  searchPlaceholder: "調べたい語を入力",
  lookUp: "検索",
  pageLabel: "項目",
  entryFrameTitle: (dictionary: string) => `${dictionary} の項目`,
  answered: (n: number) => `辞書 ${n} 冊に項目があります`,
  notFound: (query: string) => `「${query}」は見つかりませんでした。`,
  noLibraryFolder: "ライブラリフォルダがまだ設定されていません。",
  noLibraryFolderHow: (file: string) => `${file} で "libraryFolder" を設定してから、OpenDict を再起動してください。`,
  libraryUnavailable: (reason: string) => `ライブラリを開けませんでした：${reason}`,
  libraryEmpty: (folder: string) => `${folder} に辞書が見つかりませんでした。`,
  libraryCount: (n: number) => `辞書 ${n} 冊`,
  startHint: "見出し語を入力して Enter を押してください。",
  lookupFailed: (reason: string) => `検索に失敗しました：${reason}`,
  updateAvailable: (version: string) => `OpenDict ${version} が公開されています。`,
  updateInstall: "インストールして再起動",
  updateDownloading: (version: string, percent: number | null) =>
    percent === null ? `OpenDict ${version} をダウンロード中…` : `OpenDict ${version} をダウンロード中… ${percent}%`,
  updateInstalling: (version: string) => `OpenDict ${version} をインストール中…`,
  updateFailed: (version: string, reason: string) => `OpenDict ${version} をインストールできませんでした：${reason}`,
} satisfies Catalogue;
