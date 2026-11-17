//! Selection capture on macOS and Linux. Not run on either yet (built and tested on
//! Windows only).
//!
//! - macOS: a simulated ⌘C (which needs the Accessibility permission), the text read from
//!   the pasteboard, and the pasteboard's text put back. Other kinds of content on the
//!   pasteboard (pictures, files) are not put back.
//! - Linux (X11): the selection is X11's PRIMARY selection, which holds whatever is
//!   selected, so nothing is copied and the clipboard is never touched.

#[cfg(target_os = "macos")]
pub use macos::selected_text;

#[cfg(target_os = "linux")]
pub use linux::selected_text;

use std::hash::{DefaultHasher, Hash, Hasher};

/// The text on the clipboard, for clipboard watching.
pub fn clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// Stands in for a change count (which arboard does not offer): the clipboard text's hash.
pub fn clipboard_count() -> u64 {
    let mut hasher = DefaultHasher::new();
    clipboard_text().hash(&mut hasher);
    hasher.finish()
}

#[cfg(target_os = "macos")]
mod macos {
    use std::time::Duration;

    use enigo::{Direction, Enigo, Key, Keyboard as _, Settings};

    use crate::mini::selection::{self, Clipboard, Keyboard};

    struct Pasteboard(arboard::Clipboard);

    impl Clipboard for Pasteboard {
        type Saved = Option<String>;

        /// The pasteboard's change count is not offered by arboard; its text stands in. A
        /// selection that is exactly the text already on the pasteboard reads as nothing
        /// selected.
        fn change_count(&mut self) -> u64 {
            super::clipboard_count()
        }

        fn save(&mut self) -> Result<Option<String>, String> {
            Ok(self.0.get_text().ok())
        }

        fn text(&mut self) -> Option<String> {
            self.0.get_text().ok()
        }

        fn restore(&mut self, saved: Option<String>) -> Result<(), String> {
            match saved {
                Some(text) => self.0.set_text(text),
                None => self.0.clear(),
            }
            .map_err(|error| error.to_string())
        }
    }

    struct CommandC;

    impl Keyboard for CommandC {
        fn copy(&mut self) -> Result<(), String> {
            // The reader's fingers leave the shortcut's keys.
            std::thread::sleep(Duration::from_millis(200));
            let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
            enigo
                .key(Key::Meta, Direction::Press)
                .and_then(|()| enigo.key(Key::Unicode('c'), Direction::Click))
                .and_then(|()| enigo.key(Key::Meta, Direction::Release))
                .map_err(|error| error.to_string())
        }
    }

    pub fn selected_text() -> Option<String> {
        let mut pasteboard = Pasteboard(arboard::Clipboard::new().ok()?);
        selection::capture(&mut pasteboard, &mut CommandC, &mut std::thread::sleep)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use arboard::{GetExtLinux, LinuxClipboardKind};

    use crate::mini::selection;

    pub fn selected_text() -> Option<String> {
        let mut clipboard = arboard::Clipboard::new().ok()?;
        let text = clipboard
            .get()
            .clipboard(LinuxClipboardKind::Primary)
            .text()
            .ok()?;
        selection::query_from(&text)
    }
}
