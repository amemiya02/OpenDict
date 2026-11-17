# OpenDict

OpenDict is a local-first dictionary reader: it lets a person look words up across the dictionary files they own. This glossary fixes the words we use for the things a user sees and manages.

## Language

### The user's dictionaries

**Dictionary** (词典):
One dictionary file together with the resource and style files that accompany it, treated as a single unit. A file accompanies it when it is named after it (`oald.mdd`, `oald.css`, a subfolder `oald/`), or, when it is the only dictionary file in its folder, when it lies in that folder or in a subfolder that holds no other dictionary file.
_Avoid_: Book, dict file, MDX

**Library** (词库):
Every Dictionary the user has made available to OpenDict.
_Avoid_: Collection, shelf

**Library Folder** (词库文件夹):
A directory the user points OpenDict at. Dictionaries found inside it join the Library; the files stay where they are.
_Avoid_: Import folder, dictionary path

**Group** (分组):
A named, ordered subset of the Library. A Dictionary may belong to several Groups.
_Avoid_: Profile, category, set

**Current Group** (当前分组):
The Group a Lookup runs against. Its order is the order results appear in.

### Looking words up

**Headword** (词头):
The word or phrase a Dictionary files an Entry under.
_Avoid_: Key, keyword, term

**Entry** (词条):
What one Dictionary says about one Headword.
_Avoid_: Article, definition, record

**Lookup** (查词):
Finding the Entries for what the user typed or selected, across the Current Group.
_Avoid_: Search, query

**Normalization** (归一化):
Treating Headwords that differ only in letter case, diacritics, character width, Simplified/Traditional form, or kana type as the same Headword.

**Lemmatization** (形态还原):
Resolving an inflected or conjugated form to the Headword of its base form, e.g. "running" to "run".
_Avoid_: Stemming, deinflection

### Around the lookup

**Word Book** (生词本):
A named list of Headwords the user has saved, each with where it came from and an optional note.
_Avoid_: Favorites, bookmarks, vocabulary list

**Mini Window** (迷你窗口):
The small window summoned by a global shortcut to look up the text currently selected in another application.
_Avoid_: Popup, floating window
