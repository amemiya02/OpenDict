//! The public interface of `opendict-mdict`, exercised against fixture MDX files.

use opendict_mdict::{Error, Mdx, Record};
use opendict_mdict_fixture::MdxBuilder;
use tempfile::TempDir;

fn write(builder: &MdxBuilder) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.mdx");
    builder.write_to(&path).unwrap();
    (dir, path)
}

#[test]
fn reads_the_header() {
    let (_dir, path) = write(
        &MdxBuilder::new()
            .title("Sample English Dictionary")
            .description("Written for <b>tests</b> & nothing else")
            .entry("apple", "<p>a fruit</p>"),
    );

    let mdx = Mdx::open(&path).unwrap();
    let header = mdx.header();

    assert_eq!(header.title(), "Sample English Dictionary");
    assert_eq!(
        header.description(),
        "Written for <b>tests</b> & nothing else"
    );
    assert_eq!(header.engine_version(), "2.0");
    assert_eq!(header.encoding(), "UTF-8");
    assert_eq!(header.attribute("Format"), Some("Html"));
}

#[test]
fn lists_headwords_in_file_order_across_key_blocks() {
    let (_dir, path) = write(
        &MdxBuilder::new()
            .entry("apple", "<p>a fruit</p>")
            .entry("banana", "<p>a long fruit</p>")
            .entry("cherry", "<p>a small fruit</p>")
            .entry("食べる", "<p>to eat</p>")
            .entry("serendipity", "<p>a happy accident</p>")
            .keys_per_block(2),
    );

    let mdx = Mdx::open(&path).unwrap();

    assert_eq!(
        mdx.headwords().collect::<Vec<_>>(),
        ["apple", "banana", "cherry", "食べる", "serendipity"]
    );
}

#[test]
fn looks_up_the_record_for_a_headword_across_record_blocks() {
    let (_dir, path) = write(
        &MdxBuilder::new()
            .entry("apple", "<p>a fruit</p>")
            .entry("banana", "<p>a long fruit</p>")
            .entry("食べる", "<p>to eat</p>")
            .entry("serendipity", "<p class=\"def\">a happy accident</p>")
            .keys_per_block(3)
            .records_per_block(2),
    );

    let mdx = Mdx::open(&path).unwrap();

    assert_eq!(
        mdx.lookup("serendipity").unwrap(),
        [Record::Text("<p class=\"def\">a happy accident</p>".into())]
    );
    assert_eq!(
        mdx.lookup("食べる").unwrap(),
        [Record::Text("<p>to eat</p>".into())]
    );
    assert_eq!(
        mdx.lookup("apple").unwrap(),
        [Record::Text("<p>a fruit</p>".into())]
    );
}

#[test]
fn finds_nothing_for_a_headword_the_file_does_not_have() {
    let (_dir, path) = write(&MdxBuilder::new().entry("apple", "<p>a fruit</p>"));

    let mdx = Mdx::open(&path).unwrap();

    assert_eq!(mdx.lookup("Apple").unwrap(), []);
    assert_eq!(mdx.lookup("pear").unwrap(), []);
}

#[test]
fn keeps_several_records_under_one_headword_together() {
    let (_dir, path) = write(
        &MdxBuilder::new()
            .entry("bank", "<p>the side of a river</p>")
            .entry("bank", "<p>a place for money</p>")
            .entry("bark", "<p>a dog's cry</p>"),
    );

    let mdx = Mdx::open(&path).unwrap();

    assert_eq!(
        mdx.lookup("bank").unwrap(),
        [
            Record::Text("<p>the side of a river</p>".into()),
            Record::Text("<p>a place for money</p>".into())
        ]
    );
}

#[test]
fn reports_a_redirect_without_following_it() {
    let (_dir, path) = write(
        &MdxBuilder::new()
            .entry("colour", "<p>the property of reflecting light</p>")
            .link("color", "colour"),
    );

    let mdx = Mdx::open(&path).unwrap();

    assert_eq!(
        mdx.lookup("color").unwrap(),
        [Record::Link("colour".into())]
    );
}

#[test]
fn rejects_a_file_that_is_not_mdict() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.mdx");
    std::fs::write(&path, b"<html>not a dictionary</html>").unwrap();

    assert!(matches!(Mdx::open(&path), Err(Error::NotMdict(_))));
}

#[test]
fn reports_a_damaged_record_block_instead_of_returning_garbage() {
    let mut bytes = MdxBuilder::new().entry("apple", "<p>a fruit</p>").build();
    let last = bytes.len() - 6;
    bytes[last] ^= 0xFF; // inside the compressed payload of the only record block
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("damaged.mdx");
    std::fs::write(&path, bytes).unwrap();

    let mdx = Mdx::open(&path).unwrap();

    assert!(matches!(mdx.lookup("apple"), Err(Error::Corrupt(_))));
}

#[test]
fn rejects_a_truncated_file() {
    let bytes = MdxBuilder::new().entry("apple", "<p>a fruit</p>").build();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("truncated.mdx");
    std::fs::write(&path, &bytes[..bytes.len() - 10]).unwrap();

    assert!(matches!(Mdx::open(&path), Err(Error::Corrupt(_))));
}
