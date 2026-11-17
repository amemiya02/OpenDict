//! Finding Dictionaries on disk: which files make up one Dictionary, and the key that
//! identifies it from one scan to the next. Nothing here writes to a Library Folder.

use std::io::Read;
use std::path::{Path, PathBuf};

/// The files that make up one Dictionary, as found in a Library Folder.
pub(crate) struct Unit {
    pub main: PathBuf,
    /// The MDD resource files: `name.mdd`, then `name.1.mdd`, `name.2.mdd`, ...
    pub resources: Vec<PathBuf>,
    /// The stylesheets and scripts beside the main file.
    pub companions: Vec<PathBuf>,
}

impl Unit {
    /// Gathers the files that belong with `main` from the files of its folder.
    pub fn gather(main: &Path, files: &[PathBuf]) -> Unit {
        let siblings = files
            .iter()
            .filter(|file| file.parent() == main.parent() && *file != main);
        let stem = main
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let mut resources = Vec::new();
        let mut companions = Vec::new();
        for file in siblings {
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if let Some(part) = resource_part(&name, &stem) {
                resources.push((part, file.clone()));
            } else if name.ends_with(".css") || name.ends_with(".js") {
                companions.push(file.clone());
            }
        }
        resources.sort();
        Unit {
            main: main.to_owned(),
            resources: resources.into_iter().map(|(_, file)| file).collect(),
            companions,
        }
    }

    /// Every file of the Dictionary, main file first.
    pub fn files(&self) -> impl Iterator<Item = &PathBuf> {
        std::iter::once(&self.main)
            .chain(&self.resources)
            .chain(&self.companions)
    }
}

/// For `name.mdd` the part number 0, for `name.3.mdd` 3; `None` for any other file.
fn resource_part(file_name: &str, stem: &str) -> Option<u32> {
    let rest = file_name.strip_prefix(stem)?.strip_suffix(".mdd")?;
    if rest.is_empty() {
        return Some(0);
    }
    let number = rest.strip_prefix('.')?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
}

/// How many leading bytes of the main file make up its content signature. For MDict this
/// covers the header, which names the Dictionary and changes with each edition.
const SIGNATURE_BYTES: u64 = 4096;

/// The key a Dictionary keeps across rescans: derived from its file's path plus a content
/// signature, so the same file keeps its key and a different Dictionary put at the same
/// path gets a new one.
pub(crate) fn identity(main: &Path) -> std::io::Result<String> {
    let path = std::fs::canonicalize(main)?;
    let mut head = Vec::new();
    std::fs::File::open(main)?
        .take(SIGNATURE_BYTES)
        .read_to_end(&mut head)?;

    let mut hash = Fnv1a::new();
    hash.write(path.to_string_lossy().as_bytes());
    hash.write(&[0]);
    hash.write(&head);
    Ok(format!("{:016x}", hash.finish()))
}

/// 64-bit FNV-1a: tiny, and stable across Rust releases (unlike `DefaultHasher`), which
/// matters because keys are stored.
struct Fnv1a(u64);

impl Fnv1a {
    fn new() -> Self {
        Fnv1a(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// Every file under `folder`, recursively, sorted by path.
pub(crate) fn files_under(folder: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect(folder, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect(folder: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect(&entry.path(), out)?;
        } else if file_type.is_file() {
            out.push(entry.path());
        }
    }
    Ok(())
}
