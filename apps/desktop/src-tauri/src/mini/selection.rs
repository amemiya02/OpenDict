//! Reading the text the reader has selected in another application: simulate a copy,
//! read what it put on the clipboard, and put back what was there before.

use std::time::Duration;

/// The system clipboard, as capture needs it.
pub trait Clipboard {
    /// Everything on the clipboard, kept so it can be put back.
    type Saved;
    /// A number that changes whenever the clipboard's contents change.
    fn change_count(&mut self) -> u64;
    fn save(&mut self) -> Result<Self::Saved, String>;
    /// The text on the clipboard, if it holds any.
    fn text(&mut self) -> Option<String>;
    fn restore(&mut self, saved: Self::Saved) -> Result<(), String>;
}

/// The keyboard of the application in front.
pub trait Keyboard {
    /// Sends the platform's copy shortcut to the application in front.
    fn copy(&mut self) -> Result<(), String>;
}

/// How long the application in front gets to answer the copy.
pub const COPY_WAIT: Duration = Duration::from_millis(500);
/// How often the clipboard is checked meanwhile.
pub const POLL: Duration = Duration::from_millis(10);
/// The longest query a selection makes; a longer selection is cut at a word if it can be.
pub const MAX_CHARS: usize = 100;

/// Copies the selection of the application in front and returns it as a query, then puts
/// back whatever the clipboard held before. `None` when nothing was selected (the copy
/// changed nothing), when the selection holds no text, or when the clipboard could not be
/// saved (nothing is copied then, so nothing of the reader's is lost). `sleep` waits.
pub fn capture<C: Clipboard, K: Keyboard>(
    clipboard: &mut C,
    keyboard: &mut K,
    sleep: &mut dyn FnMut(Duration),
) -> Option<String> {
    let saved = match clipboard.save() {
        Ok(saved) => saved,
        Err(error) => {
            eprintln!("OpenDict: the clipboard could not be saved, so nothing was copied ({error})");
            return None;
        }
    };
    let before = clipboard.change_count();
    if let Err(error) = keyboard.copy() {
        eprintln!("OpenDict: the copy could not be sent ({error})");
        return None;
    }
    let mut waited = Duration::ZERO;
    while clipboard.change_count() == before {
        if waited >= COPY_WAIT {
            return None; // nothing selected: the clipboard is as it was
        }
        sleep(POLL);
        waited += POLL;
    }
    let text = clipboard.text();
    if let Err(error) = clipboard.restore(saved) {
        eprintln!("OpenDict: the clipboard could not be put back ({error})");
    }
    text.as_deref().and_then(query_from)
}

/// The selection as a query: whitespace runs (line breaks too) become one space, and a
/// long selection is cut to [`MAX_CHARS`].
pub fn query_from(selection: &str) -> Option<String> {
    let words: Vec<&str> = selection.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let mut query = String::new();
    for word in words {
        let length = query.chars().count();
        let room = MAX_CHARS.saturating_sub(length + usize::from(length > 0));
        if word.chars().count() > room {
            if length == 0 {
                query = word.chars().take(MAX_CHARS).collect();
            }
            break;
        }
        if length > 0 {
            query.push(' ');
        }
        query.push_str(word);
    }
    Some(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A clipboard holding several formats, like a real one.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Contents {
        text: Option<String>,
        html: Option<String>,
    }

    struct World {
        contents: Contents,
        count: u64,
        /// What the application in front puts on the clipboard when told to copy, and
        /// after how many checks.
        selection: Option<String>,
        answers_after: u32,
        copying: Option<u32>,
        copies: u32,
        restores: u32,
        writable: bool,
    }

    #[derive(Clone)]
    struct Fake(Rc<RefCell<World>>);

    impl Fake {
        fn new(before: Contents, selection: Option<&str>) -> Fake {
            Fake(Rc::new(RefCell::new(World {
                contents: before,
                count: 7,
                selection: selection.map(str::to_owned),
                answers_after: 3,
                copying: None,
                copies: 0,
                restores: 0,
                writable: true,
            })))
        }
    }

    impl Clipboard for Fake {
        type Saved = Contents;
        fn change_count(&mut self) -> u64 {
            let mut world = self.0.borrow_mut();
            if let Some(left) = world.copying {
                if left == 0 {
                    world.copying = None;
                    let text = world.selection.clone();
                    world.contents = Contents { text, html: None };
                    world.count += 1;
                } else {
                    world.copying = Some(left - 1);
                }
            }
            world.count
        }
        fn save(&mut self) -> Result<Contents, String> {
            let world = self.0.borrow();
            if world.writable {
                Ok(world.contents.clone())
            } else {
                Err("the clipboard is busy".to_owned())
            }
        }
        fn text(&mut self) -> Option<String> {
            self.0.borrow().contents.text.clone()
        }
        fn restore(&mut self, saved: Contents) -> Result<(), String> {
            let mut world = self.0.borrow_mut();
            world.contents = saved;
            world.count += 1;
            world.restores += 1;
            Ok(())
        }
    }

    impl Keyboard for Fake {
        fn copy(&mut self) -> Result<(), String> {
            let mut world = self.0.borrow_mut();
            world.copies += 1;
            if world.selection.is_some() {
                world.copying = Some(world.answers_after);
            }
            Ok(())
        }
    }

    fn before() -> Contents {
        Contents {
            text: Some("what the reader copied".to_owned()),
            html: Some("<b>what the reader copied</b>".to_owned()),
        }
    }

    fn run(fake: &Fake) -> (Option<String>, Duration) {
        let mut waited = Duration::ZERO;
        let mut clipboard = fake.clone();
        let mut keyboard = fake.clone();
        let text = capture(&mut clipboard, &mut keyboard, &mut |d| waited += d);
        (text, waited)
    }

    #[test]
    fn the_selection_is_returned_and_the_clipboard_put_back_whole() {
        let fake = Fake::new(before(), Some("serendipity"));
        let (text, _) = run(&fake);
        assert_eq!(text.as_deref(), Some("serendipity"));
        let world = fake.0.borrow();
        assert_eq!(world.contents, before());
        assert_eq!(world.restores, 1);
    }

    #[test]
    fn with_nothing_selected_there_is_no_query_and_the_clipboard_is_left_alone() {
        let fake = Fake::new(before(), None);
        let (text, waited) = run(&fake);
        assert_eq!(text, None);
        assert!(waited >= COPY_WAIT, "it waited for the copy: {waited:?}");
        let world = fake.0.borrow();
        assert_eq!(world.contents, before());
        assert_eq!(world.restores, 0, "nothing to put back");
    }

    #[test]
    fn an_empty_clipboard_is_empty_again_afterwards() {
        let empty = Contents {
            text: None,
            html: None,
        };
        let fake = Fake::new(empty.clone(), Some("食べた"));
        assert_eq!(run(&fake).0.as_deref(), Some("食べた"));
        assert_eq!(fake.0.borrow().contents, empty);
    }

    #[test]
    fn a_slow_application_is_waited_for() {
        let fake = Fake::new(before(), Some("slow"));
        fake.0.borrow_mut().answers_after = 30;
        let (text, waited) = run(&fake);
        assert_eq!(text.as_deref(), Some("slow"));
        assert!(waited < COPY_WAIT, "no longer than needed: {waited:?}");
    }

    #[test]
    fn a_selection_without_text_is_no_query_but_the_clipboard_is_still_put_back() {
        let fake = Fake::new(before(), Some("  \n\t "));
        assert_eq!(run(&fake).0, None);
        assert_eq!(fake.0.borrow().contents, before());
    }

    #[test]
    fn if_the_clipboard_cannot_be_saved_nothing_is_copied() {
        let fake = Fake::new(before(), Some("serendipity"));
        fake.0.borrow_mut().writable = false;
        assert_eq!(run(&fake).0, None);
        let world = fake.0.borrow();
        assert_eq!(world.copies, 0);
        assert_eq!(world.contents, before());
    }

    #[test]
    fn a_selection_becomes_a_query_on_one_line() {
        assert_eq!(
            query_from("  happy\r\n  accident \t").as_deref(),
            Some("happy accident")
        );
        assert_eq!(query_from(" \n "), None);
    }

    #[test]
    fn a_long_selection_is_cut_at_a_word() {
        let long = "word ".repeat(40);
        let query = query_from(&long).unwrap();
        assert!(query.chars().count() <= MAX_CHARS, "{query}");
        assert!(query.ends_with("word"), "{query}");
        // Text without spaces (Japanese, Chinese) is cut at the limit.
        let japanese = "あ".repeat(150);
        assert_eq!(query_from(&japanese).unwrap().chars().count(), MAX_CHARS);
        // A first word that fills the query exactly leaves no room for the next.
        let full = format!("{} more", "a".repeat(MAX_CHARS));
        assert_eq!(query_from(&full).unwrap(), "a".repeat(MAX_CHARS));
    }
}
