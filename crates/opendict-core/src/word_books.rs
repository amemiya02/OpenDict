//! Word Books: named lists of the Headwords a reader saved, each with the Dictionary it came
//! from, when it was saved and an optional note. All of them are kept in one JSON file,
//! written whole after every change.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Stays the same for as long as the Word Book exists, whatever it is renamed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WordBookId(pub u64);

/// One named Word Book. Its words are newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordBook {
    pub id: WordBookId,
    pub name: String,
    pub words: Vec<SavedWord>,
}

/// A Headword in a Word Book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedWord {
    pub headword: String,
    /// The Dictionary the reader was reading when they saved it.
    pub saved_from: SavedFrom,
    /// A note or example sentence; `None` when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(with = "unix_millis")]
    pub added: SystemTime,
}

/// The Dictionary a word was saved from. The title is kept too, so the word still says
/// where it came from after that Dictionary leaves the Library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFrom {
    pub dictionary_id: String,
    pub dictionary_title: String,
}

/// Every Word Book, and which one Ctrl/⌘ + D adds to.
#[derive(Debug, Clone)]
pub struct WordBooks {
    file: PathBuf,
    stored: Stored,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    /// In the order they were made.
    #[serde(default)]
    books: Vec<WordBook>,
    /// The Word Book new words go into.
    #[serde(default)]
    current: Option<WordBookId>,
    /// The id the next Word Book gets; ids are never reused.
    #[serde(default)]
    next_id: u64,
}

impl WordBooks {
    /// Reads the Word Books kept in `file`. A missing file means there are none yet.
    pub fn load(file: &Path) -> Result<WordBooks> {
        let stored = match std::fs::read_to_string(file) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| problem(file, error))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Stored::default(),
            Err(error) => return Err(problem(file, error)),
        };
        Ok(WordBooks {
            file: file.to_owned(),
            stored,
        })
    }

    /// Every Word Book, in the order they were made.
    pub fn books(&self) -> &[WordBook] {
        &self.stored.books
    }

    pub fn book(&self, id: WordBookId) -> Option<&WordBook> {
        self.stored.books.iter().find(|book| book.id == id)
    }

    /// The Word Book new words go into: the one the reader chose last, or `None` while
    /// there are no Word Books.
    pub fn current(&self) -> Option<&WordBook> {
        self.stored.current.and_then(|id| self.book(id))
    }

    /// Makes a new, empty Word Book called `name` and chooses it. Returns its id.
    pub fn create(&mut self, name: &str) -> Result<WordBookId> {
        let name = valid_name(name)?;
        let mut stored = self.stored.clone();
        let id = WordBookId(stored.next_id);
        stored.next_id += 1;
        stored.books.push(WordBook {
            id,
            name,
            words: Vec::new(),
        });
        stored.current = Some(id);
        self.commit(stored)?;
        Ok(id)
    }

    /// Gives the Word Book a new name.
    pub fn rename(&mut self, id: WordBookId, name: &str) -> Result<()> {
        let name = valid_name(name)?;
        let mut stored = self.stored.clone();
        book_mut(&mut stored, id)?.name = name;
        self.commit(stored)
    }

    /// Deletes the Word Book and every word in it. If it was the chosen one, the first
    /// Word Book left is chosen instead.
    pub fn delete(&mut self, id: WordBookId) -> Result<()> {
        let mut stored = self.stored.clone();
        book_mut(&mut stored, id)?;
        stored.books.retain(|book| book.id != id);
        if stored.current == Some(id) {
            stored.current = stored.books.first().map(|book| book.id);
        }
        self.commit(stored)
    }

    /// Makes `id` the Word Book new words go into.
    pub fn choose(&mut self, id: WordBookId) -> Result<()> {
        let mut stored = self.stored.clone();
        book_mut(&mut stored, id)?;
        stored.current = Some(id);
        self.commit(stored)
    }

    /// Saves `headword` to the Word Book, or removes it if it is there already. Returns
    /// whether the word is in the Word Book afterwards.
    pub fn toggle(
        &mut self,
        book: WordBookId,
        headword: &str,
        saved_from: SavedFrom,
        at: SystemTime,
    ) -> Result<bool> {
        let mut stored = self.stored.clone();
        let words = &mut book_mut(&mut stored, book)?.words;
        let saved = match words.iter().position(|word| word.headword == headword) {
            Some(at) => {
                words.remove(at);
                false
            }
            None => {
                words.insert(
                    0,
                    SavedWord {
                        headword: headword.to_owned(),
                        saved_from,
                        note: None,
                        added: at,
                    },
                );
                true
            }
        };
        self.commit(stored)?;
        Ok(saved)
    }

    /// Takes `headword` out of the Word Book. A word that is not there is left alone.
    pub fn remove(&mut self, book: WordBookId, headword: &str) -> Result<()> {
        let mut stored = self.stored.clone();
        book_mut(&mut stored, book)?
            .words
            .retain(|word| word.headword != headword);
        self.commit(stored)
    }

    /// Sets the note on a saved word; a blank note removes it.
    pub fn set_note(&mut self, book: WordBookId, headword: &str, note: &str) -> Result<()> {
        let mut stored = self.stored.clone();
        let word = book_mut(&mut stored, book)?
            .words
            .iter_mut()
            .find(|word| word.headword == headword)
            .ok_or_else(|| Error::NotInWordBook(headword.to_owned()))?;
        let note = note.trim();
        word.note = (!note.is_empty()).then(|| note.to_owned());
        self.commit(stored)
    }

    /// Writes the Word Book to `file` for Anki or a spreadsheet: a header row (Headword,
    /// Note, Dictionary, Added), then one row per word in the Word Book's order. Times are
    /// UTC in ISO 8601. Fields holding the separator, a quote or a line break are quoted.
    pub fn export(&self, book: WordBookId, format: ExportFormat, file: &Path) -> Result<()> {
        let book = self.book(book).ok_or(Error::UnknownWordBook(book.0))?;
        let separator = match format {
            ExportFormat::Csv => ',',
            ExportFormat::Tsv => '\t',
        };
        let mut text = String::new();
        let mut row = |fields: [&str; 4]| {
            for (at, field) in fields.into_iter().enumerate() {
                if at > 0 {
                    text.push(separator);
                }
                if field.contains([separator, '"', '\n', '\r']) {
                    text.push('"');
                    text.push_str(&field.replace('"', "\"\""));
                    text.push('"');
                } else {
                    text.push_str(field);
                }
            }
            text.push('\n');
        };
        row(["Headword", "Note", "Dictionary", "Added"]);
        for word in &book.words {
            row([
                &word.headword,
                word.note.as_deref().unwrap_or(""),
                &word.saved_from.dictionary_title,
                &iso_8601(word.added),
            ]);
        }
        let failed = |error: std::io::Error| Error::Export {
            file: file.to_owned(),
            reason: error.to_string(),
        };
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder).map_err(failed)?;
        }
        std::fs::write(file, text).map_err(failed)
    }

    /// Writes `stored` and keeps it, or keeps what there was if it cannot be written.
    fn commit(&mut self, stored: Stored) -> Result<()> {
        let text = serde_json::to_string_pretty(&stored).expect("Word Books serialise");
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder).map_err(|error| problem(&self.file, error))?;
        }
        // A temporary file first, so a crash never leaves half a file.
        let temporary = self.file.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(|error| problem(&self.file, error))?;
        std::fs::rename(&temporary, &self.file).map_err(|error| problem(&self.file, error))?;
        self.stored = stored;
        Ok(())
    }
}

/// The two kinds of file a Word Book exports to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    /// Comma-separated values.
    Csv,
    /// Tab-separated values.
    Tsv,
}

/// `time` in UTC to the second, as `2026-11-17T09:30:00Z`.
fn iso_8601(time: SystemTime) -> String {
    let seconds = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, of_day) = (seconds / 86_400, seconds % 86_400);
    // Days to a civil date (Howard Hinnant's algorithm), for days since 1970-01-01.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

fn book_mut(stored: &mut Stored, id: WordBookId) -> Result<&mut WordBook> {
    stored
        .books
        .iter_mut()
        .find(|book| book.id == id)
        .ok_or(Error::UnknownWordBook(id.0))
}

fn valid_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::BlankWordBookName);
    }
    Ok(name.to_owned())
}

fn problem(file: &Path, error: impl ToString) -> Error {
    Error::WordBooks {
        file: file.to_owned(),
        reason: error.to_string(),
    }
}

/// Times are kept as milliseconds since the Unix epoch.
mod unix_millis {
    use super::{Duration, SystemTime};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error> {
        let millis = time
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        serializer.serialize_u64(u64::try_from(millis).unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<SystemTime, D::Error> {
        let millis = u64::deserialize(deserializer)?;
        Ok(SystemTime::UNIX_EPOCH + Duration::from_millis(millis))
    }
}
