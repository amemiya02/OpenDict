//! Reading MDD resource files through the public interface of `opendict-mdict`.

use opendict_mdict::Mdd;
use opendict_mdict_fixture::MddBuilder;
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
