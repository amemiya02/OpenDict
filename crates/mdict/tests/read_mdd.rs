//! Reading MDD resource files through the public interface of `opendict-mdict`.

use opendict_mdict::{Error, Mdd};
use opendict_mdict_fixture::{Damage, MddBuilder};
use tempfile::TempDir;

fn write(builder: &MddBuilder) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.mdd");
    builder.write_to(&path).unwrap();
    (dir, path)
}

/// Bytes that look like a PNG and contain NULs, so a reader that stops at NUL fails.
const IMAGE: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01";
const STYLE: &[u8] = b".hw { font-weight: bold; }";
const SOUND: &[u8] = b"Speex   \0\x01\x02\x03";

fn sample() -> MddBuilder {
    MddBuilder::new()
        .resource("\\img\\a.png", IMAGE)
        .resource("\\style.css", STYLE)
        .resource("\\sound\\word.spx", SOUND)
        .keys_per_block(2)
        .records_per_block(2)
}

#[test]
fn lists_resource_paths_in_file_order() {
    let (_dir, path) = write(&sample());

    let mdd = Mdd::open(&path).unwrap();

    assert_eq!(
        mdd.resource_paths().collect::<Vec<_>>(),
        ["\\img\\a.png", "\\style.css", "\\sound\\word.spx"]
    );
}

#[test]
fn extracts_a_resource_byte_for_byte_by_its_stored_path() {
    let (_dir, path) = write(&sample());

    let mdd = Mdd::open(&path).unwrap();

    assert_eq!(
        mdd.resource("\\img\\a.png").unwrap().as_deref(),
        Some(IMAGE)
    );
    assert_eq!(mdd.resource("\\style.css").unwrap().as_deref(), Some(STYLE));
    assert_eq!(
        mdd.resource("\\sound\\word.spx").unwrap().as_deref(),
        Some(SOUND)
    );
}

#[test]
fn resolves_a_path_as_html_refers_to_it() {
    let (_dir, path) = write(&sample());

    let mdd = Mdd::open(&path).unwrap();

    // Forward slashes, no leading separator, different letter case.
    assert_eq!(mdd.resource("img/a.png").unwrap().as_deref(), Some(IMAGE));
    assert_eq!(mdd.resource("/IMG/A.PNG").unwrap().as_deref(), Some(IMAGE));
    assert_eq!(mdd.resource("Style.CSS").unwrap().as_deref(), Some(STYLE));
}

#[test]
fn finds_nothing_for_a_path_the_file_does_not_have() {
    let (_dir, path) = write(&sample());

    let mdd = Mdd::open(&path).unwrap();

    assert_eq!(mdd.resource("img/b.png").unwrap(), None);
    assert_eq!(mdd.resource("a.png").unwrap(), None);
}

#[test]
fn reads_the_header_of_a_resource_file() {
    let (_dir, path) = write(&sample());

    let mdd = Mdd::open(&path).unwrap();

    assert_eq!(mdd.header().engine_version(), "2.0");
    assert_eq!(mdd.header().encoding(), "");
}

/// The sample with its paths in the order MDict files them: sorted, case folded.
fn sorted_sample() -> MddBuilder {
    MddBuilder::new()
        .resource("\\img\\a.png", IMAGE)
        .resource("\\img\\b.png", b"another picture")
        .resource("\\sound\\word.spx", SOUND)
        .resource("\\style.css", STYLE)
        .resource("\\Zebra.css", b"z")
        .keys_per_block(2)
        .records_per_block(3)
}

#[test]
fn finds_each_resource_of_a_sorted_file_and_nothing_else() {
    let (_dir, path) = write(&sorted_sample());

    let mdd = Mdd::open(&path).unwrap();

    // `word.spx` is the last key of its block: its record ends where the next block's
    // first record starts.
    assert_eq!(
        mdd.resource("sound/word.spx").unwrap().as_deref(),
        Some(SOUND)
    );
    assert_eq!(
        mdd.resource("zebra.css").unwrap().as_deref(),
        Some(&b"z"[..])
    );
    assert_eq!(
        mdd.resource("img/b.png").unwrap().as_deref(),
        Some(&b"another picture"[..])
    );
    assert_eq!(mdd.resource("style.css").unwrap().as_deref(), Some(STYLE));
    assert_eq!(mdd.resource("img/a.png").unwrap().as_deref(), Some(IMAGE));
    for missing in ["a.png", "img/c.png", "sound", "zz.css", "\\"] {
        assert_eq!(mdd.resource(missing).unwrap(), None, "{missing}");
    }
}

#[test]
fn opening_reads_no_key_block_so_a_damaged_one_is_reported_when_a_resource_is_read() {
    for builder in [sample(), sorted_sample()] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.mdd");
        std::fs::write(&path, builder.build_damaged(Damage::KeyBlock)).unwrap();

        let mdd = Mdd::open(&path).unwrap();

        let read = mdd.resource("img/a.png");
        assert!(matches!(read, Err(Error::Corrupt(_))), "{read:?}");
    }
}
