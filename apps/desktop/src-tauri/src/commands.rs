//! Tauri commands. Each one only forwards to [`AppState`]; no Lookup logic lives here.

use tauri::State;

use crate::state::{AppState, EntryView, LibraryStatus};

#[tauri::command]
pub fn look_up(state: State<'_, AppState>, headword: String) -> Result<Vec<EntryView>, String> {
    state.look_up(&headword)
}

#[tauri::command]
pub fn library_status(state: State<'_, AppState>) -> LibraryStatus {
    state.library_status()
}

#[tauri::command]
pub fn set_allow_network(
    state: State<'_, AppState>,
    dictionary_id: String,
    allow: bool,
) -> Result<(), String> {
    state.set_allow_network(&dictionary_id, allow)
}
