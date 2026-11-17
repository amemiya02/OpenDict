//! The public interface of `opendict-mdict` for MDD resource files, against fixtures.

use opendict_mdict::Mdd;
use opendict_mdict_fixture::MddBuilder;

/// Bytes with NULs and high bytes in them, so a reader that stops at NUL or decodes
/// text would get them wrong.
const PICTURE: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\xff\x00sample";

#[test]
fn resolves_a_resource_by_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.mdd");
    MddBuilder::new()
        .resource("\\img\\cat.png", PICTURE)
        .resource("\\style.css", b"p { color: teal }")
        .keys_per_block(1)
        .write_to(&path)
        .unwrap();

    let mdd = Mdd::open(&path).unwrap();

    assert_eq!(
        mdd.resource("\\img\\cat.png").unwrap().as_deref(),
        Some(PICTURE)
    );
    assert_eq!(
        mdd.resource("\\style.css").unwrap().as_deref(),
        Some(&b"p { color: teal }"[..])
    );
    assert_eq!(mdd.resource("\\missing.png").unwrap(), None);
}

#[test]
fn accepts_web_style_paths_in_any_letter_case() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.mdd");
    MddBuilder::new()
        .resource("\\img\\Cat.PNG", PICTURE)
        .write_to(&path)
        .unwrap();

    let mdd = Mdd::open(&path).unwrap();

    for spelling in [
        "img/cat.png",
        "/img/cat.png",
        "\\IMG\\cat.png",
        "img\\Cat.PNG",
    ] {
        assert_eq!(
            mdd.resource(spelling).unwrap().as_deref(),
            Some(PICTURE),
            "{spelling}"
        );
    }
}
