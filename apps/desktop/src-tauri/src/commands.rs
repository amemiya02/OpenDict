//! Tauri commands. Each one only forwards to [`AppState`]; no Library or Lookup logic
//! lives here. Commands that read or change the Library run off the main thread
//! (`async`), so the window stays responsive while one of them waits for the Library
//! (a scan puts its result in place, an index is swapped in).

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use opendict_core::ExportFormat;

use crate::mini::settings::Role;
use crate::mini::{self, Mini, MiniSettingsView, Summons};
use crate::state::{
    AppState, AppearanceView, DataDirectoryError, DataDirectoryView, Direction, ElsewhereView,
    EntryView, Language, LibraryView, RegisterError, SuggestionView, SuggestionsView, Theme,
    WordBooksView,
};
use crate::{EXPORT_DIR_ENV, PICK_DATA_FOLDER_ENV};

/// Where the reader's data is kept.
#[tauri::command]
pub fn data_directory(state: State<'_, AppState>) -> DataDirectoryView {
    state.data_directory()
}

/// Asks the reader for a folder for their data with the system dialog. Returns `None`
/// when the reader cancels. The dialog runs on the Rust side, so the interface needs no
/// dialog permission of its own.
#[tauri::command]
pub async fn choose_data_folder(app: AppHandle) -> Result<Option<PathBuf>, String> {
    if let Some(folder) = std::env::var_os(PICK_DATA_FOLDER_ENV) {
        return Ok(Some(folder.into()));
    }
    let Some(folder) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    folder
        .into_path()
        .map(Some)
        .map_err(|error| error.to_string())
}

/// Moves the reader's data into `path` and uses it from then on. Indexing that was
/// stopped for the move starts again.
#[tauri::command]
pub async fn move_data_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<DataDirectoryView, DataDirectoryError> {
    let moved = state.move_data_directory(&path);
    start_indexing(&app, &state);
    if let Ok(view) = &moved {
        mini::reload(&app, &view.path);
    }
    moved
}

/// Uses the data already in `path` from then on, leaving the data used until now where
/// it is.
#[tauri::command]
pub async fn use_data_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<DataDirectoryView, DataDirectoryError> {
    let used = state.use_data_directory(&path);
    start_indexing(&app, &state);
    if let Ok(view) = &used {
        mini::reload(&app, &view.path);
    }
    used
}

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

#[tauri::command(async)]
pub fn look_up(state: State<'_, AppState>, headword: String) -> Result<Vec<EntryView>, String> {
    state.look_up(&headword)
}

#[tauri::command(async)]
pub fn suggest(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<SuggestionsView, String> {
    state.suggest(&query, limit)
}

#[tauri::command(async)]
pub fn headwords_after(
    state: State<'_, AppState>,
    query: String,
    headword: String,
    limit: usize,
) -> Result<Vec<SuggestionView>, String> {
    state.headwords_after(&query, &headword, limit)
}

#[tauri::command(async)]
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
#[tauri::command(async)]
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

#[tauri::command(async)]
pub fn create_group(state: State<'_, AppState>, name: String) -> Result<LibraryView, String> {
    state.create_group(&name)
}

#[tauri::command(async)]
pub fn rename_group(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<LibraryView, String> {
    state.rename_group(&id, &name)
}

#[tauri::command(async)]
pub fn delete_group(state: State<'_, AppState>, id: String) -> Result<LibraryView, String> {
    state.delete_group(&id)
}

#[tauri::command(async)]
pub fn set_in_group(
    state: State<'_, AppState>,
    group: String,
    dictionary: String,
    member: bool,
) -> Result<LibraryView, String> {
    state.set_in_group(&group, &dictionary, member)
}

#[tauri::command(async)]
pub fn move_in_group(
    state: State<'_, AppState>,
    group: String,
    dictionary: String,
    direction: Direction,
) -> Result<LibraryView, String> {
    state.move_in_group(&group, &dictionary, direction)
}

/// Makes `group` the Current Group; `null` makes Lookups run against the whole Library.
#[tauri::command(async)]
pub fn set_current_group(
    state: State<'_, AppState>,
    group: Option<String>,
) -> Result<LibraryView, String> {
    state.set_current_group(group.as_deref())
}

/// Another Group that has `query` when the Current Group does not, for the no-match page.
#[tauri::command(async)]
pub fn elsewhere(
    state: State<'_, AppState>,
    query: String,
) -> Result<Option<ElsewhereView>, String> {
    state.elsewhere(&query)
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

/// The event every window hears when the interface language changes, so a window that is
/// open (the Mini Window, say) switches with the one the reader switched it in.
pub const LANGUAGE_EVENT: &str = "language";

/// Remembers the interface language (`None`: follow the system language) and tells every
/// window.
#[tauri::command]
pub fn set_language(
    app: AppHandle,
    state: State<'_, AppState>,
    language: Option<Language>,
) -> Result<(), String> {
    state.set_language(language)?;
    let _ = app.emit(LANGUAGE_EVENT, language);
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
    app: AppHandle,
    state: State<'_, AppState>,
    headword: String,
    dictionary_id: String,
    dictionary_title: String,
    new_book_name: String,
) -> Result<WordBooksView, String> {
    let view = state.toggle_word(&headword, &dictionary_id, &dictionary_title, &new_book_name)?;
    // The other window (main or Mini) shows the word as saved too.
    let _ = app.emit(WORD_BOOKS_EVENT, &view);
    Ok(view)
}

/// The event that carries the Word Books to every window after a word is saved or taken out.
pub const WORD_BOOKS_EVENT: &str = "word-books";

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

/// The latest summons of the Mini Window, for a Mini Window that loaded after it.
#[tauri::command]
pub fn mini_summons(mini: State<'_, Mini>) -> Option<Summons> {
    mini.last()
}

/// Esc in the Mini Window: it goes, and focus goes back to where the reader was.
#[tauri::command]
pub fn close_mini(app: AppHandle) {
    mini::close(&app);
}

/// "Open in main window" from the Mini Window.
#[tauri::command]
pub fn open_in_main(app: AppHandle, headword: String) {
    mini::open_in_main(&app, &headword);
}

#[tauri::command]
pub fn mini_settings(mini: State<'_, Mini>) -> MiniSettingsView {
    mini.view()
}

/// Changes one of the global shortcuts; `None` turns it off. Async: registering waits for
/// the main thread.
#[tauri::command]
pub async fn set_shortcut(
    app: AppHandle,
    role: Role,
    accelerator: Option<String>,
) -> Result<MiniSettingsView, String> {
    mini::set_shortcut(&app, role, accelerator.as_deref())
}

#[tauri::command]
pub fn set_watch_clipboard(app: AppHandle, on: bool) -> Result<MiniSettingsView, String> {
    mini::set_watch_clipboard(&app, on)
}
