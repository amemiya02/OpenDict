//! Compatibility run against real dictionaries.
//!
//! Reads every `.mdx` and `.mdd` under the folder named by `OPENDICT_TEST_DICTS` and
//! skips (passes) when it is unset. Real dictionaries are copyrighted: they never enter
//! the repository, and this test prints only counts, never their content.
//!
//! `OPENDICT_TEST_DICTS=/path/to/dictionaries cargo test -p opendict-mdict --test compatibility -- --nocapture`

use std::path::{Path, PathBuf};

use opendict_mdict::{Mdd, Mdx, Record};

fn dictionary_files(extension: &str) -> Option<Vec<PathBuf>> {
    let Some(folder) = std::env::var_os("OPENDICT_TEST_DICTS") else {
        eprintln!("OPENDICT_TEST_DICTS is not set; skipping the compatibility run");
        return None;
    };
    let mut found = Vec::new();
    collect(Path::new(&folder), extension, &mut found);
    found.sort();
    assert!(
        !found.is_empty() || extension == "mdd",
        "OPENDICT_TEST_DICTS names a folder with no .{extension} files"
    );
    Some(found)
}

fn collect(folder: &Path, extension: &str, found: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(folder).expect("OPENDICT_TEST_DICTS is a readable folder");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, extension, found);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        {
            found.push(path);
        }
    }
}

/// About this many headwords per dictionary are looked up, spread evenly through it.
const SAMPLES: usize = 500;

#[test]
fn every_mdx_opens_lists_its_headwords_and_looks_them_up() {
    let Some(files) = dictionary_files("mdx") else {
        return;
    };
    check_each(files, check_mdx);
}

#[test]
fn every_mdd_opens_and_yields_its_resources() {
    let Some(files) = dictionary_files("mdd") else {
        return;
    };
    check_each(files, check_mdd);
}

/// Checks every file, reporting each one's result, and fails at the end if any failed.
fn check_each(files: Vec<PathBuf>, check: fn(&Path) -> Result<String, String>) {
    let mut failures = 0;
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        match check(&path) {
            Ok(summary) => eprintln!("ok   {name}: {summary}"),
            Err(problem) => {
                failures += 1;
                eprintln!("FAIL {name}: {problem}");
            }
        }
    }
    assert_eq!(failures, 0, "{failures} files failed; see the log above");
}

fn check_mdx(path: &Path) -> Result<String, String> {
    let mdx = Mdx::open(path).map_err(|e| format!("open: {e}"))?;
    let headwords: Vec<&str> = mdx.headwords().collect();
    if headwords.is_empty() {
        return Err("no headwords".to_owned());
    }
    let step = (headwords.len() / SAMPLES).max(1);
    let (mut texts, mut links, mut dangling) = (0, 0, 0);
    for headword in headwords.iter().step_by(step) {
        let records = mdx.lookup(headword).map_err(|e| format!("lookup: {e}"))?;
        if records.is_empty() {
            return Err("a listed headword has no record".to_owned());
        }
        for record in records {
            match record {
                Record::Text(_) => texts += 1,
                Record::Link(target) => {
                    links += 1;
                    let found = mdx.lookup(&target).map_err(|e| format!("lookup: {e}"))?;
                    if found.is_empty() {
                        dangling += 1;
                    }
                }
            }
        }
    }
    let header = mdx.header();
    Ok(format!(
        "version {}, encoding {:?}, encrypted {}, {} headwords; sampled {texts} entries, {links} redirects ({dangling} to a headword the file lacks)",
        header.engine_version(),
        header.encoding(),
        header.encrypted(),
        headwords.len(),
    ))
}

fn check_mdd(path: &Path) -> Result<String, String> {
    let mdd = Mdd::open(path).map_err(|e| format!("open: {e}"))?;
    let paths: Vec<&str> = mdd.resource_paths().collect();
    let step = (paths.len() / SAMPLES).max(1);
    // Looked up in a file opened afresh, which finds them without reading every key,
    // and compared with the file that has read them all.
    let fresh = Mdd::open(path).map_err(|e| format!("open: {e}"))?;
    let mut bytes = 0;
    for resource in paths.iter().step_by(step) {
        let data = fresh
            .resource(resource)
            .map_err(|e| format!("extract: {e}"))?
            .ok_or("a listed resource does not resolve")?;
        let whole = mdd
            .resource(resource)
            .map_err(|e| format!("extract: {e}"))?;
        if whole.as_deref() != Some(&data[..]) {
            return Err("a resource reads differently found by its key block".into());
        }
        bytes += data.len();
    }
    for missing in ["no-such-resource.png", "\\zzzz\\none.css"] {
        if fresh
            .resource(missing)
            .map_err(|e| format!("extract: {e}"))?
            .is_some()
        {
            return Err(format!("{missing:?} resolves"));
        }
    }
    Ok(format!(
        "version {}, encrypted {}, {} resources; extracted {bytes} bytes from a sample",
        mdd.header().engine_version(),
        mdd.header().encrypted(),
        paths.len(),
    ))
}
