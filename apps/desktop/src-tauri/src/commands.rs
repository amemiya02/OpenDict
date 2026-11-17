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
