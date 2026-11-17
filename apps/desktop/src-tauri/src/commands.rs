//! Tauri commands. Each one only forwards to [`AppState`]; no Library or Lookup logic
//! lives here. Commands that scan Library Folders are `async` so they run off the main
//! thread and the window stays responsive.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use opendict_core::ExportFormat;

use crate::EXPORT_DIR_ENV;
use crate::state::{
    AppState, AppearanceView, Direction, EntryView, LibraryView, RegisterError, SuggestionView,
    SuggestionsView, Theme, WordBooksView,
};

/// Unlocks a Dictionary that needs a registration code with the code and e-mail the
/// reader entered; they are kept locally when they work. The unlocked Dictionary is
/// indexed in the background.
#[tauri::command]
pub async fn register_dictionary(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
    email: String,
    code: String,
) -> Result<LibraryView, RegisterError> {
    let view = state.register(&path, &email, &code)?;
    start_indexing(&app, &state);
    Ok(view)
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
    query: String,
    headword: String,
    limit: usize,
) -> Result<Vec<SuggestionView>, String> {
    state.headwords_after(&query, &headword, limit)
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
    let view = state.add_folder(&folder)?;
    start_indexing(&app, &state);
    Ok(Some(view))
}

#[tauri::command]
pub async fn remove_library_folder(
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<LibraryView, String> {
    state.remove_folder(&path)
}

#[tauri::command]
pub async fn rescan_library(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LibraryView, String> {
    let view = state.rescan()?;
    start_indexing(&app, &state);
    Ok(view)
}

/// Indexes the Dictionaries still waiting for their index (after a cancel, say).
#[tauri::command]
pub fn index_library(app: AppHandle, state: State<'_, AppState>) -> LibraryView {
    start_indexing(&app, &state);
    state.library()
}

/// Stops indexing; the Dictionaries not yet indexed wait until it is started again.
#[tauri::command]
pub fn cancel_indexing(state: State<'_, AppState>) {
    state.cancel_indexing();
}

/// The event that carries the Library view to the interface while indexing runs.
pub const LIBRARY_EVENT: &str = "library";

/// Starts indexing in the background, passing each change of the Library view on to
/// the interface as a [`LIBRARY_EVENT`].
pub fn start_indexing(app: &AppHandle, state: &AppState) {
    let app = app.clone();
    state.start_indexing(move |view| {
        let _ = app.emit(LIBRARY_EVENT, view);
    });
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

/// The headwords of committed Lookups, newest first.
#[tauri::command]
pub fn history(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.history()
}

/// Records a committed Lookup (Enter, a click, a followed link); returns History as it
/// is afterwards.
#[tauri::command]
pub fn record_lookup(state: State<'_, AppState>, headword: String) -> Result<Vec<String>, String> {
    state.record_lookup(&headword)
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.clear_history()
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
pub fn appearance(state: State<'_, AppState>) -> AppearanceView {
    state.appearance()
}

/// Remembers the theme and gives it to the windows, so their native frames and the
/// interface's `prefers-color-scheme` follow it.
#[tauri::command]
pub fn set_theme(app: AppHandle, state: State<'_, AppState>, theme: Theme) -> Result<(), String> {
    state.set_theme(theme)?;
    apply_theme(&app, theme);
    Ok(())
}

#[tauri::command]
pub fn set_as_authored(
    state: State<'_, AppState>,
    dictionary_id: String,
    as_authored: bool,
) -> Result<(), String> {
    state.set_as_authored(&dictionary_id, as_authored)
}

#[tauri::command]
pub fn set_all_as_authored(state: State<'_, AppState>, as_authored: bool) -> Result<(), String> {
    state.set_all_as_authored(as_authored)
}

/// Remembers the base text size of Entries; returns it as set, within limits.
#[tauri::command]
pub fn set_text_size(state: State<'_, AppState>, size: u32) -> Result<u32, String> {
    state.set_text_size(size)
}

/// Remembers the page zoom and zooms the windows; returns it as set, within limits.
#[tauri::command]
pub fn set_zoom(app: AppHandle, state: State<'_, AppState>, percent: u32) -> Result<u32, String> {
    let zoom = state.set_zoom(percent)?;
    apply_zoom(&app, zoom);
    Ok(zoom)
}

/// Gives the windows the reader's theme; `System` lets them follow the operating system.
/// Each window takes it itself, so its native frame and material (Mica) match the page and
/// its webview's `prefers-color-scheme` follows.
pub fn apply_theme(app: &AppHandle, theme: Theme) {
    let theme = match theme {
        Theme::System => None,
        Theme::Light => Some(tauri::Theme::Light),
        Theme::Dark => Some(tauri::Theme::Dark),
    };
    app.set_theme(theme);
    for window in app.webview_windows().values() {
        let _ = window.set_theme(theme);
    }
}

/// Zooms every window's page, Entry frames included, to `percent`.
pub fn apply_zoom(app: &AppHandle, percent: u32) {
    for window in app.webview_windows().values() {
        // A platform without page zoom keeps the page at its own size.
        let _ = window.set_zoom(f64::from(percent) / 100.0);
    }
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
