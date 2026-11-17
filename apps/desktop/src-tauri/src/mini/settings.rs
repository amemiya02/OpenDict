//! The Mini Window's settings: its two global shortcuts and clipboard watching, kept in
//! `mini-window.json` in the data directory. They belong to the desktop shell, not to the
//! core, which knows nothing of global shortcuts.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::{Modifiers, Shortcut};

/// The settings file's name in the data directory.
pub const FILE_NAME: &str = "mini-window.json";

/// The two global shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// Copies the selection in any application and looks it up in the Mini Window.
    LookUpSelection,
    /// Brings the main window forward.
    ShowMainWindow,
}

/// The Mini Window's settings, saved as soon as they change.
#[derive(Debug, Clone)]
pub struct MiniSettings {
    file: PathBuf,
    stored: Stored,
}

/// As the file holds them. A shortcut is an accelerator such as `Control+Alt+KeyD`; an
/// empty one is turned off. A shortcut the file does not mention has its default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    #[serde(default = "default_look_up_selection")]
    look_up_selection: String,
    #[serde(default = "default_show_main_window")]
    show_main_window: String,
    /// Looks up whatever the reader copies. Off unless the reader turns it on.
    #[serde(default)]
    watch_clipboard: bool,
}

impl Default for Stored {
    fn default() -> Self {
        Stored {
            look_up_selection: default_look_up_selection(),
            show_main_window: default_show_main_window(),
            watch_clipboard: false,
        }
    }
}

/// ⌘ on macOS, Ctrl elsewhere: the modifier the platform's own shortcuts use.
#[cfg(target_os = "macos")]
const PRIMARY: &str = "Super";
#[cfg(not(target_os = "macos"))]
const PRIMARY: &str = "Control";

fn default_look_up_selection() -> String {
    format!("{PRIMARY}+Alt+KeyD")
}

fn default_show_main_window() -> String {
    format!("{PRIMARY}+Alt+KeyO")
}

/// The settings as the interface shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Accelerators such as `Control+Alt+KeyD`; null when turned off.
    pub look_up_selection: Option<String>,
    pub show_main_window: Option<String>,
    pub watch_clipboard: bool,
}

impl MiniSettings {
    /// Reads the settings from `file`. A missing file means every default; a file that
    /// cannot be read means every default too, with the reason why (the next change
    /// replaces it).
    pub fn load(file: &Path) -> (MiniSettings, Option<String>) {
        let (stored, problem) = match std::fs::read_to_string(file) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(stored) => (stored, None),
                Err(error) => (Stored::default(), Some(problem(file, error))),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Stored::default(), None),
            Err(error) => (Stored::default(), Some(problem(file, error))),
        };
        let settings = MiniSettings {
            file: file.to_owned(),
            stored,
        };
        (settings, problem)
    }

    /// The shortcut for `role`, or `None` when it is turned off (or the file holds one
    /// that cannot be read).
    pub fn shortcut(&self, role: Role) -> Option<Shortcut> {
        let accelerator = self.accelerator(role);
        if accelerator.is_empty() {
            return None;
        }
        Shortcut::from_str(accelerator).ok()
    }

    fn accelerator(&self, role: Role) -> &str {
        match role {
            Role::LookUpSelection => &self.stored.look_up_selection,
            Role::ShowMainWindow => &self.stored.show_main_window,
        }
    }

    pub fn watch_clipboard(&self) -> bool {
        self.stored.watch_clipboard
    }

    pub fn view(&self) -> SettingsView {
        SettingsView {
            look_up_selection: self.shortcut(Role::LookUpSelection).map(accelerator),
            show_main_window: self.shortcut(Role::ShowMainWindow).map(accelerator),
            watch_clipboard: self.stored.watch_clipboard,
        }
    }

    /// Changes the shortcut for `role` and saves; `None` (or an empty accelerator) turns
    /// it off. A shortcut needs a modifier and one other key, and the two shortcuts must
    /// differ. Nothing changes if the shortcut is refused.
    pub fn set_shortcut(&mut self, role: Role, accelerator: Option<&str>) -> Result<(), String> {
        let text = match accelerator.map(str::trim).filter(|text| !text.is_empty()) {
            None => String::new(),
            Some(text) => {
                let shortcut = parse(text)?;
                let other = match role {
                    Role::LookUpSelection => Role::ShowMainWindow,
                    Role::ShowMainWindow => Role::LookUpSelection,
                };
                if self.shortcut(other) == Some(shortcut) {
                    return Err(format!("{text} is already the other shortcut"));
                }
                self::accelerator(shortcut)
            }
        };
        let mut stored = self.stored.clone();
        match role {
            Role::LookUpSelection => stored.look_up_selection = text,
            Role::ShowMainWindow => stored.show_main_window = text,
        }
        self.save(stored)
    }

    pub fn set_watch_clipboard(&mut self, on: bool) -> Result<(), String> {
        let stored = Stored {
            watch_clipboard: on,
            ..self.stored.clone()
        };
        self.save(stored)
    }

    /// Writes to a temporary file first so a crash never leaves a half-written file. The
    /// settings change only once they are saved.
    fn save(&mut self, stored: Stored) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&stored).expect("settings serialise");
        let write = || -> std::io::Result<()> {
            if let Some(folder) = self.file.parent() {
                std::fs::create_dir_all(folder)?;
            }
            let temporary = self.file.with_extension("json.tmp");
            std::fs::write(&temporary, text)?;
            std::fs::rename(&temporary, &self.file)
        };
        write().map_err(|error| problem(&self.file, error))?;
        self.stored = stored;
        Ok(())
    }
}

/// A shortcut the reader may use: a modifier and one other key.
fn parse(text: &str) -> Result<Shortcut, String> {
    let shortcut = Shortcut::from_str(text).map_err(|error| format!("{text} is not a shortcut ({error})"))?;
    if shortcut.mods.is_empty() {
        return Err(format!("{text} needs Ctrl, Alt, Shift or ⌘ with it"));
    }
    Ok(shortcut)
}

/// The accelerator for `shortcut`, modifiers in a fixed order, then the key's code.
pub fn accelerator(shortcut: Shortcut) -> String {
    let mut parts = Vec::new();
    for (modifier, name) in [
        (Modifiers::CONTROL, "Control"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
        (Modifiers::SUPER, "Super"),
    ] {
        if shortcut.mods.contains(modifier) {
            parts.push(name.to_owned());
        }
    }
    parts.push(shortcut.key.to_string());
    parts.join("+")
}

fn problem(file: &Path, error: impl ToString) -> String {
    format!("{} could not be read or saved: {}", file.display(), error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_in(dir: &tempfile::TempDir) -> MiniSettings {
        let (settings, problem) = MiniSettings::load(&dir.path().join(FILE_NAME));
        assert_eq!(problem, None);
        settings
    }

    #[test]
    fn by_default_ctrl_alt_d_looks_up_ctrl_alt_o_shows_the_main_window_and_the_clipboard_is_not_watched()
    {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(&dir);
        assert_eq!(
            settings.shortcut(Role::LookUpSelection),
            Some(Shortcut::from_str("CommandOrControl+Alt+D").unwrap())
        );
        assert_eq!(
            settings.shortcut(Role::ShowMainWindow),
            Some(Shortcut::from_str("CommandOrControl+Alt+O").unwrap())
        );
        assert!(!settings.watch_clipboard());
        assert!(!dir.path().join(FILE_NAME).exists(), "nothing is written until a change");
    }

    #[test]
    fn a_changed_shortcut_is_kept_in_the_data_directory() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = settings_in(&dir);
        settings
            .set_shortcut(Role::LookUpSelection, Some("shift+ctrl+F9"))
            .unwrap();
        assert_eq!(
            settings.view().look_up_selection.as_deref(),
            Some("Control+Shift+F9")
        );

        let again = settings_in(&dir);
        assert_eq!(
            again.shortcut(Role::LookUpSelection),
            Some(Shortcut::from_str("Ctrl+Shift+F9").unwrap())
        );
        assert_eq!(
            again.shortcut(Role::ShowMainWindow),
            Some(Shortcut::from_str("CommandOrControl+Alt+O").unwrap()),
            "the other shortcut keeps its default"
        );
    }

    #[test]
    fn a_shortcut_without_a_modifier_or_not_a_shortcut_at_all_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = settings_in(&dir);
        for refused in ["KeyD", "Control+Alt+", "Control+Alt+NoSuchKey", "Alt+D+E"] {
            assert!(
                settings
                    .set_shortcut(Role::LookUpSelection, Some(refused))
                    .is_err(),
                "{refused}"
            );
        }
        assert_eq!(
            settings.shortcut(Role::LookUpSelection),
            Some(Shortcut::from_str("CommandOrControl+Alt+D").unwrap())
        );
        assert!(!dir.path().join(FILE_NAME).exists());
    }

    #[test]
    fn the_two_shortcuts_must_differ() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = settings_in(&dir);
        let error = settings
            .set_shortcut(Role::ShowMainWindow, Some("CommandOrControl+Alt+KeyD"))
            .unwrap_err();
        assert!(error.contains("other shortcut"), "{error}");
        assert_eq!(
            settings.shortcut(Role::ShowMainWindow),
            Some(Shortcut::from_str("CommandOrControl+Alt+O").unwrap())
        );
    }

    #[test]
    fn a_shortcut_can_be_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = settings_in(&dir);
        settings.set_shortcut(Role::ShowMainWindow, None).unwrap();
        assert_eq!(settings.shortcut(Role::ShowMainWindow), None);
        assert_eq!(settings.view().show_main_window, None);
        assert_eq!(settings_in(&dir).shortcut(Role::ShowMainWindow), None);
    }

    #[test]
    fn clipboard_watching_is_kept_once_turned_on() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = settings_in(&dir);
        settings.set_watch_clipboard(true).unwrap();
        assert!(settings.view().watch_clipboard);
        assert!(settings_in(&dir).watch_clipboard());
    }

    #[test]
    fn a_damaged_file_gives_the_defaults_and_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE_NAME);
        std::fs::write(&file, "{ not json").unwrap();
        let (settings, problem) = MiniSettings::load(&file);
        assert!(problem.is_some());
        assert!(!settings.watch_clipboard());
        assert_eq!(
            settings.shortcut(Role::LookUpSelection),
            Some(Shortcut::from_str("CommandOrControl+Alt+D").unwrap())
        );
    }

    #[test]
    fn the_interface_names_the_shortcuts_in_camel_case() {
        assert_eq!(
            serde_json::from_str::<Role>("\"lookUpSelection\"").unwrap(),
            Role::LookUpSelection
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"showMainWindow\"").unwrap(),
            Role::ShowMainWindow
        );
    }
}
