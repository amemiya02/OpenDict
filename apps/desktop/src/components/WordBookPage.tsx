import { useId, useState, type FormEvent, type ReactNode } from "react";
import { useI18n } from "../i18n";
import type { ExportFormat, SavedWord, WordBook, WordBooks } from "../lib/ipc";
import { MOD } from "../lib/platform";
import { OutlineButton } from "./LibraryPage";

/** Kana mark a headword as Japanese, so it is set in the Japanese serif. */
const KANA = /[぀-ヿ]/;

export interface WordBookActions {
  choose: (id: number) => void;
  create: (name: string) => void;
  rename: (id: number, name: string) => void;
  delete: (id: number) => void;
  removeWord: (id: number, headword: string) => void;
  setNote: (id: number, headword: string, note: string) => void;
  export: (id: number, format: ExportFormat) => void;
  lookUp: (headword: string) => void;
}

/**
 * The Word Book view: the chosen Word Book's name over a double rule, the controls for
 * choosing, making, renaming, deleting and exporting Word Books, then its words with
 * their notes. Set like the Library view: rules and hairlines, no boxes.
 */
export function WordBookPage({
  wordBooks,
  busy,
  notice,
  actions,
}: {
  wordBooks: WordBooks;
  busy: boolean;
  notice: string | null;
  actions: WordBookActions;
}) {
  const { t } = useI18n();
  const book = wordBooks.books.find((b) => b.id === wordBooks.current) ?? null;
  /** Which name form is open: a new Word Book, or renaming the chosen one. */
  const [naming, setNaming] = useState<"new" | "rename" | null>(null);
  const [deleting, setDeleting] = useState(false);
  const chooser = useId();

  return (
    <article aria-busy={busy}>
      <header className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
        <h1 tabIndex={-1} className="-ml-3.5 font-serif text-display font-bold [overflow-wrap:anywhere] outline-0">
          【{book ? book.name : t.wordBook}】
        </h1>
        {book ? <span className="text-[12px] text-text-secondary">{t.wordCount(book.words.length)}</span> : null}
      </header>

      <div className="flex flex-wrap items-center gap-1.5 pt-3.5">
        {wordBooks.books.length > 0 && book ? (
          <label htmlFor={chooser} className="flex items-center gap-1.5 text-label text-text-secondary">
            {t.chooseWordBook}
            <select
              id={chooser}
              value={book.id}
              disabled={busy}
              onChange={(event) => actions.choose(Number(event.target.value))}
              className="h-7 max-w-[240px] rounded-sharp border border-rule bg-paper px-1.5 text-body text-text"
            >
              {wordBooks.books.map((b) => (
                <option key={b.id} value={b.id}>
                  {b.name}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        <OutlineButton onClick={() => setNaming("new")} disabled={busy}>
          {t.newWordBook}
        </OutlineButton>
        {book ? (
          <>
            <OutlineButton onClick={() => setNaming("rename")} disabled={busy}>
              {t.renameWordBook}
            </OutlineButton>
            <OutlineButton onClick={() => setDeleting(true)} disabled={busy}>
              {t.deleteWordBook}
            </OutlineButton>
            <OutlineButton onClick={() => actions.export(book.id, "csv")} disabled={busy}>
              {t.exportCsv}
            </OutlineButton>
            <OutlineButton onClick={() => actions.export(book.id, "tsv")} disabled={busy}>
              {t.exportTsv}
            </OutlineButton>
          </>
        ) : null}
        <span role="status" className="min-w-0 text-label [overflow-wrap:anywhere] text-text-secondary">
          {notice}
        </span>
      </div>

      {naming ? (
        <NameForm
          key={naming}
          initial={naming === "rename" && book ? book.name : ""}
          busy={busy}
          onSave={(name) => {
            setNaming(null);
            if (naming === "rename" && book) actions.rename(book.id, name);
            else actions.create(name);
          }}
          onCancel={() => setNaming(null)}
        />
      ) : null}

      {deleting && book ? (
        <div className="flex max-w-[860px] flex-wrap items-center gap-1.5 border-b border-hair py-2">
          <span className="mr-auto">{t.deleteWordBookQuestion(book.name, book.words.length)}</span>
          <OutlineButton onClick={() => setDeleting(false)}>{t.deleteWordBookKeep}</OutlineButton>
          <OutlineButton
            onClick={() => {
              setDeleting(false);
              actions.delete(book.id);
            }}
          >
            {t.deleteWordBookConfirm}
          </OutlineButton>
        </div>
      ) : null}

      {book ? (
        book.words.length > 0 ? (
          <WordTable book={book} busy={busy} actions={actions} />
        ) : (
          <Lead>{t.emptyWordBook(MOD)}</Lead>
        )
      ) : (
        <Lead>
          {t.noWordBooks} {t.noWordBooksHow(MOD)}
        </Lead>
      )}
    </article>
  );
}

/** The name of a new Word Book, or a new name for the chosen one. */
function NameForm({
  initial,
  busy,
  onSave,
  onCancel,
}: {
  initial: string;
  busy: boolean;
  onSave: (name: string) => void;
  onCancel: () => void;
}) {
  const { t } = useI18n();
  const [name, setName] = useState(initial);

  function submit(event: FormEvent) {
    event.preventDefault();
    if (name.trim()) onSave(name.trim());
  }

  return (
    <form onSubmit={submit} className="flex max-w-[860px] flex-wrap items-end gap-1.5 border-b border-hair py-2">
      <label className="flex flex-col gap-0.5 text-label text-text-secondary">
        {t.wordBookName}
        <input
          autoFocus
          value={name}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") onCancel();
          }}
          className="h-7 w-[280px] min-w-0 rounded-sharp border border-hair bg-transparent px-2 font-serif text-[15px] text-text focus-visible:border-rule focus-visible:outline focus-visible:outline-1 focus-visible:outline-rule"
        />
      </label>
      <button
        type="submit"
        disabled={busy || !name.trim()}
        className="h-7 rounded-sharp border border-rule px-2.5 text-body text-text hover:bg-shade active:translate-y-px disabled:opacity-50"
      >
        {t.saveName}
      </button>
      <OutlineButton onClick={onCancel}>{t.cancel}</OutlineButton>
    </form>
  );
}

function WordTable({ book, busy, actions }: { book: WordBook; busy: boolean; actions: WordBookActions }) {
  const { t, locale } = useI18n();
  const head = "h-7 border-b border-rule pr-2.5 text-left text-label font-normal whitespace-nowrap text-text-secondary";
  const when = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" });
  return (
    <table className="mt-3.5 w-full max-w-[860px] border-collapse">
      <thead>
        <tr>
          <th scope="col" className={head}>
            {t.columnHeadword}
          </th>
          <th scope="col" className={`${head} w-full`}>
            {t.columnNote}
          </th>
          <th scope="col" className={head}>
            {t.columnDictionary}
          </th>
          <th scope="col" className={head}>
            {t.columnAdded}
          </th>
          <th scope="col" className={head}>
            <span className="sr-only">{t.remove}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {book.words.map((word) => (
          <WordRow key={word.headword} book={book} word={word} busy={busy} actions={actions} when={when} />
        ))}
      </tbody>
    </table>
  );
}

function WordRow({
  book,
  word,
  busy,
  actions,
  when,
}: {
  book: WordBook;
  word: SavedWord;
  busy: boolean;
  actions: WordBookActions;
  when: Intl.DateTimeFormat;
}) {
  const { t } = useI18n();
  const [note, setNote] = useState(word.note ?? "");
  const cell = "border-b border-hair py-1 pr-2.5 align-middle";

  function keep() {
    if (note.trim() !== (word.note ?? "")) actions.setNote(book.id, word.headword, note);
  }

  return (
    <tr>
      <th scope="row" className={`${cell} text-left font-normal whitespace-nowrap`}>
        <button
          type="button"
          lang={KANA.test(word.headword) ? "ja" : undefined}
          onClick={() => actions.lookUp(word.headword)}
          className="rounded-sharp px-1 font-serif text-index hover:bg-shade"
        >
          {word.headword}
        </button>
      </th>
      <td className={cell}>
        <input
          aria-label={t.noteFor(word.headword)}
          value={note}
          onChange={(event) => setNote(event.target.value)}
          onBlur={keep}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.currentTarget.blur();
          }}
          className="h-6.5 w-full min-w-[160px] rounded-sharp border border-transparent bg-transparent px-1.5 text-body text-text hover:border-hair focus-visible:border-rule focus-visible:outline focus-visible:outline-1 focus-visible:outline-rule"
        />
      </td>
      <td className={`${cell} text-[12px] whitespace-nowrap text-text-secondary`}>{word.dictionaryTitle}</td>
      <td className={`${cell} text-label whitespace-nowrap text-text-secondary tabular-nums`}>{when.format(word.added)}</td>
      <td className={`${cell} pr-0`}>
        <button
          type="button"
          aria-label={t.removeWord(word.headword)}
          disabled={busy}
          onClick={() => actions.removeWord(book.id, word.headword)}
          className="h-6.5 flex-none rounded-sharp px-2 text-[12px] whitespace-nowrap text-text-secondary hover:bg-shade hover:text-text disabled:opacity-50"
        >
          {t.remove}
        </button>
      </td>
    </tr>
  );
}

function Lead({ children }: { children: ReactNode }) {
  return <p className="max-w-[860px] py-lg text-[14px] text-text-secondary">{children}</p>;
}
