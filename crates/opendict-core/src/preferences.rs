use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The reader's preferences, kept in one JSON file: per-Dictionary choices and how the
/// page reads (theme, text size, zoom). Every change is written straight away.
#[derive(Debug, Clone)]
pub struct Preferences {
    file: PathBuf,
    stored: Stored,
}

/// The interface's colour theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Light or dark as the operating system is.
    #[default]
    System,
    Light,
    Dark,
}

/// The interface language: one of the three the app has catalogues for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    /// Simplified Chinese.
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "ja")]
    Ja,
}

/// The base text size of Entries, in CSS pixels: the size of text a Dictionary does not
/// size itself.
pub const DEFAULT_TEXT_SIZE: u32 = 16;
const TEXT_SIZES: std::ops::RangeInclusive<u32> = 10..=32;
/// Page zoom, in percent.
pub const DEFAULT_ZOOM: u32 = 100;
const ZOOMS: std::ops::RangeInclusive<u32> = 50..=200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    /// Keyed by Dictionary id.
    #[serde(default)]
    dictionaries: BTreeMap<String, DictionaryPreferences>,
    #[serde(default)]
    theme: Theme,
    /// In the dark theme, every Dictionary shows as authored unless the reader chose
    /// otherwise for it.
    #[serde(default)]
    all_as_authored: bool,
    #[serde(default = "default_text_size")]
    text_size: u32,
    #[serde(default = "default_zoom")]
    zoom: u32,
    /// The interface language the reader chose; unset follows the system language.
    #[serde(
        default,
        deserialize_with = "known_language",
        skip_serializing_if = "Option::is_none"
    )]
    language: Option<Language>,
}

/// A language the app has no catalogue for (say, written by a later version) counts as
/// no choice, so the interface follows the system language again.
fn known_language<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Language>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| serde_json::from_value(value).ok()))
}

impl Default for Stored {
    fn default() -> Self {
        Stored {
            dictionaries: BTreeMap::new(),
            theme: Theme::default(),
            all_as_authored: false,
            text_size: DEFAULT_TEXT_SIZE,
            zoom: DEFAULT_ZOOM,
            language: None,
        }
    }
}

fn default_text_size() -> u32 {
    DEFAULT_TEXT_SIZE
}

fn default_zoom() -> u32 {
    DEFAULT_ZOOM
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryPreferences {
    /// Whether the Dictionary's Entries may load from or talk to the network. Off unless
    /// the reader turns it on.
    #[serde(default)]
    allow_network: bool,
    /// In the dark theme, whether the Dictionary keeps its own white page instead of being
    /// recoloured. Unset follows the global switch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    as_authored: Option<bool>,
}

impl Preferences {
    /// Reads the preferences from `file`. A missing file means every default.
    pub fn load(file: &Path) -> Result<Preferences> {
        let stored = match std::fs::read_to_string(file) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| problem(file, error))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Stored::default(),
            Err(error) => return Err(problem(file, error)),
        };
        Ok(Preferences {
            file: file.to_owned(),
            stored,
        })
    }

    /// Whether the Dictionary with `dictionary_id` may use the network.
    pub fn allows_network(&self, dictionary_id: &str) -> bool {
        self.stored
            .dictionaries
            .get(dictionary_id)
            .is_some_and(|d| d.allow_network)
    }

    /// Allows or forbids network access for one Dictionary, and saves.
    pub fn set_allows_network(&mut self, dictionary_id: &str, allow: bool) -> Result<()> {
        self.change_dictionary(dictionary_id, |d| d.allow_network = allow)
    }

    pub fn theme(&self) -> Theme {
        self.stored.theme
    }

    pub fn set_theme(&mut self, theme: Theme) -> Result<()> {
        self.stored.theme = theme;
        self.save()
    }

    /// Whether, in the dark theme, the Dictionary shows as authored on its white page
    /// rather than recoloured.
    pub fn as_authored(&self, dictionary_id: &str) -> bool {
        self.stored
            .dictionaries
            .get(dictionary_id)
            .and_then(|d| d.as_authored)
            .unwrap_or(self.stored.all_as_authored)
    }

    /// Shows one Dictionary as authored, or recoloured, in the dark theme, and saves.
    pub fn set_as_authored(&mut self, dictionary_id: &str, as_authored: bool) -> Result<()> {
        let choice = (as_authored != self.stored.all_as_authored).then_some(as_authored);
        self.change_dictionary(dictionary_id, |d| d.as_authored = choice)
    }

    /// Whether every Dictionary shows as authored in the dark theme by default.
    pub fn all_as_authored(&self) -> bool {
        self.stored.all_as_authored
    }

    /// Shows every Dictionary, including ones added later, as authored or recoloured in
    /// the dark theme, replacing the choices made for single Dictionaries, and saves.
    pub fn set_all_as_authored(&mut self, as_authored: bool) -> Result<()> {
        self.stored.all_as_authored = as_authored;
        for preferences in self.stored.dictionaries.values_mut() {
            preferences.as_authored = None;
        }
        self.stored
            .dictionaries
            .retain(|_, d| *d != DictionaryPreferences::default());
        self.save()
    }

    /// The base text size of Entries, in CSS pixels.
    pub fn text_size(&self) -> u32 {
        self.stored
            .text_size
            .clamp(*TEXT_SIZES.start(), *TEXT_SIZES.end())
    }

    /// Sets the base text size, kept within readable limits, saves, and returns the size
    /// as set.
    pub fn set_text_size(&mut self, pixels: u32) -> Result<u32> {
        self.stored.text_size = pixels.clamp(*TEXT_SIZES.start(), *TEXT_SIZES.end());
        self.save()?;
        Ok(self.stored.text_size)
    }

    /// Page zoom, in percent.
    pub fn zoom(&self) -> u32 {
        self.stored.zoom.clamp(*ZOOMS.start(), *ZOOMS.end())
    }

    /// Sets the page zoom, kept within limits, saves, and returns the zoom as set.
    pub fn set_zoom(&mut self, percent: u32) -> Result<u32> {
        self.stored.zoom = percent.clamp(*ZOOMS.start(), *ZOOMS.end());
        self.save()?;
        Ok(self.stored.zoom)
    }

    /// The interface language the reader chose, or `None` to follow the system language.
    pub fn language(&self) -> Option<Language> {
        self.stored.language
    }

    /// Sets the interface language (`None`: follow the system language), and saves.
    pub fn set_language(&mut self, language: Option<Language>) -> Result<()> {
        self.stored.language = language;
        self.save()
    }

    /// Changes one Dictionary's preferences, forgets them when they are all defaults
    /// again, and saves.
    fn change_dictionary(
        &mut self,
        dictionary_id: &str,
        change: impl FnOnce(&mut DictionaryPreferences),
    ) -> Result<()> {
        let entry = self
            .stored
            .dictionaries
            .entry(dictionary_id.to_owned())
            .or_default();
        change(entry);
        if *entry == DictionaryPreferences::default() {
            self.stored.dictionaries.remove(dictionary_id);
        }
        self.save()
    }

    /// Writes to a temporary file first so a crash never leaves a half-written file.
    fn save(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(&self.stored).expect("preferences serialise");
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder).map_err(|error| problem(&self.file, error))?;
        }
        let temporary = self.file.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(|error| problem(&self.file, error))?;
        std::fs::rename(&temporary, &self.file).map_err(|error| problem(&self.file, error))
    }
}

fn problem(file: &Path, error: impl ToString) -> Error {
    Error::Preferences {
        file: file.to_owned(),
        reason: error.to_string(),
    }
}
