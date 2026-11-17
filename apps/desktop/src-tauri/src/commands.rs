//! Tauri commands. Each one only forwards to [`AppState`]; no Library or Lookup logic
//! lives here. Commands that scan Library Folders are `async` so they run off the main
//! thread and the window stays responsive.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use crate::state::{
    AppState, Direction, EntryView, LibraryView, RegisterError, SuggestionView, SuggestionsView,
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
