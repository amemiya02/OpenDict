//! Tauri commands. Each one only forwards to [`AppState`]; no Library or Lookup logic
//! lives here. Commands that scan Library Folders are `async` so they run off the main
//! thread and the window stays responsive.

use std::path::PathBuf;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use opendict_core::ExportFormat;

use crate::EXPORT_DIR_ENV;
use crate::state::{
    AppState, Direction, EntryView, LibraryView, RegisterError, SuggestionView, SuggestionsView,
    WordBooksView,
};

/// Unlocks a Dictionary that needs a registration code with the code and e-mail the
/// reader entered; they are kept locally when they work.
#[tauri::command]
pub async fn register_dictionary(
    state: State<'_, AppState>,
    path: PathBuf,
    email: String,
    code: String,
) -> Result<LibraryView, RegisterError> {
    state.register(&path, &email, &code)
}

#[tauri::command]
pub fn look_up(state: State<'_, AppState>, headword: String) -> Result<Vec<EntryView>, String> {
    state.look_up(&headword)
}

#[tauri::command]
pub fn suggest(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<SuggestionsView, String> {
    state.suggest(&query, limit)
}

#[tauri::command]
pub fn headwords_after(
    state: State<'_, AppState>,
    headword: String,
    limit: usize,
) -> Result<Vec<SuggestionView>, String> {
    state.headwords_after(&headword, limit)
}

#[tauri::command]
pub fn library(state: State<'_, AppState>) -> LibraryView {
    state.library()
}

/// Asks the reader for a folder with the system dialog and adds it as a Library Folder.
/// Returns `None` when the reader cancels. The dialog runs on the Rust side, so the
/// interface needs no file-system or dialog permissions of its own.
#[tauri::command]
pub async fn add_library_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<LibraryView>, String> {
    let Some(folder) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let folder = folder.into_path().map_err(|error| error.to_string())?;
    state.add_folder(&folder).map(Some)
}

#[tauri::command]
pub async fn remove_library_folder(
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<LibraryView, String> {
    state.remove_folder(&path)
}

#[tauri::command]
pub async fn rescan_library(state: State<'_, AppState>) -> Result<LibraryView, String> {
    state.rescan()
}

#[tauri::command]
pub async fn set_dictionary_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<LibraryView, String> {
    state.set_enabled(&id, enabled)
}

#[tauri::command]
pub async fn move_dictionary(
    state: State<'_, AppState>,
    id: String,
    direction: Direction,
) -> Result<LibraryView, String> {
    state.move_dictionary(&id, direction)
}

#[tauri::command]
pub fn set_allow_network(
    state: State<'_, AppState>,
    dictionary_id: String,
    allow: bool,
) -> Result<(), String> {
    state.set_allow_network(&dictionary_id, allow)
}

#[tauri::command]
pub fn word_books(state: State<'_, AppState>) -> Result<WordBooksView, String> {
    state.word_books()
}

/// Saves the word to the chosen Word Book, or takes it out (Ctrl/⌘ + D). `new_book_name`,
/// in the reader's language, names the Word Book made for the first word.
#[tauri::command]
pub fn toggle_word(
    state: State<'_, AppState>,
    headword: String,
    dictionary_id: String,
    dictionary_title: String,
    new_book_name: String,
) -> Result<WordBooksView, String> {
    state.toggle_word(&headword, &dictionary_id, &dictionary_title, &new_book_name)
}

#[tauri::command]
pub fn create_word_book(state: State<'_, AppState>, name: String) -> Result<WordBooksView, String> {
    state.create_word_book(&name)
}

#[tauri::command]
pub fn rename_word_book(
    state: State<'_, AppState>,
    id: u64,
    name: String,
) -> Result<WordBooksView, String> {
    state.rename_word_book(id, &name)
}

#[tauri::command]
pub fn delete_word_book(state: State<'_, AppState>, id: u64) -> Result<WordBooksView, String> {
    state.delete_word_book(id)
}

#[tauri::command]
pub fn choose_word_book(state: State<'_, AppState>, id: u64) -> Result<WordBooksView, String> {
    state.choose_word_book(id)
}

#[tauri::command]
pub fn remove_saved_word(
    state: State<'_, AppState>,
    id: u64,
    headword: String,
) -> Result<WordBooksView, String> {
    state.remove_saved_word(id, &headword)
}

#[tauri::command]
pub fn set_word_note(
    state: State<'_, AppState>,
    id: u64,
    headword: String,
    note: String,
) -> Result<WordBooksView, String> {
    state.set_word_note(id, &headword, &note)
}

/// Asks the reader where to save the Word Book with the system dialog, then exports it
/// there. Returns the file, or `None` when the reader cancels. With `OPENDICT_EXPORT_DIR`
/// set (tests), the file goes into that folder without asking.
#[tauri::command]
pub async fn export_word_book(
    app: AppHandle,
    state: State<'_, AppState>,
    id: u64,
    format: ExportFormat,
) -> Result<Option<PathBuf>, String> {
    let name = state
        .word_book_name(id)
        .ok_or_else(|| format!("there is no Word Book {id}"))?;
    let extension = match format {
        ExportFormat::Csv => "csv",
        ExportFormat::Tsv => "tsv",
    };
    let file_name = format!("{}.{extension}", file_name(&name));
    let file = match std::env::var_os(EXPORT_DIR_ENV) {
        Some(folder) => PathBuf::from(folder).join(file_name),
        None => {
            let Some(file) = app
                .dialog()
                .file()
                .set_file_name(&file_name)
                .add_filter(extension.to_uppercase(), &[extension])
                .blocking_save_file()
            else {
                return Ok(None);
            };
            file.into_path().map_err(|error| error.to_string())?
        }
    };
    state.export_word_book(id, format, &file)?;
    Ok(Some(file))
}

/// The Word Book's name with the characters file systems refuse replaced.
fn file_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let safe = safe.trim().trim_end_matches('.');
    if safe.is_empty() {
        "Word Book".to_owned()
    } else {
        safe.to_owned()
    }
}
