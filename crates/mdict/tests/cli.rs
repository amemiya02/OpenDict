//! The `opendict-mdict` command-line tool, run as a user would run it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use opendict_mdict_fixture::{MddBuilder, MdxBuilder};
use tempfile::TempDir;

const IMAGE: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn fixtures() -> (TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let mdx = dir.path().join("sample.mdx");
    let mdd = dir.path().join("sample.mdd");
    MdxBuilder::new()
        .title("Sample Dictionary")
        .entry("apple", "<p>a fruit</p>")
        .entry("bank", "<p>the side of a river</p>")
        .entry("bank", "<p>a place for money</p>")
        .link("colour", "color")
        .entry("食べる", "<p>to eat</p>")
        .write_to(&mdx)
        .unwrap();
    MddBuilder::new()
        .resource("\\img\\a.png", IMAGE)
        .resource("\\style.css", b"p { margin: 0; }")
        .write_to(&mdd)
        .unwrap();
    (dir, mdx, mdd)
}

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_opendict-mdict"))
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn os(text: &str) -> &std::ffi::OsStr {
    text.as_ref()
}

fn p(path: &Path) -> &std::ffi::OsStr {
    path.as_os_str()
}

#[test]
fn shows_the_header() {
    let (_dir, mdx, _) = fixtures();

    let output = run(&[os("header"), p(&mdx)]);

    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Title: Sample Dictionary"), "{text}");
    assert!(text.contains("GeneratedByEngineVersion: 2.0"), "{text}");
    assert!(text.contains("Encoding: UTF-8"), "{text}");
}

#[test]
fn lists_headwords_one_per_line_in_file_order() {
    let (_dir, mdx, _) = fixtures();

    let output = run(&[os("headwords"), p(&mdx)]);

    assert!(output.status.success());
    assert_eq!(stdout(&output), "apple\nbank\nbank\ncolour\n食べる\n");
}

#[test]
fn lists_the_resource_paths_of_an_mdd() {
    let (_dir, _, mdd) = fixtures();

    let output = run(&[os("headwords"), p(&mdd)]);

    assert!(output.status.success());
    assert_eq!(stdout(&output), "\\img\\a.png\n\\style.css\n");
}

#[test]
fn looks_up_a_headword_and_prints_every_record() {
    let (_dir, mdx, _) = fixtures();

    let output = run(&[os("lookup"), p(&mdx), os("bank")]);

    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "<p>the side of a river</p>\n</>\n<p>a place for money</p>\n"
    );

    let output = run(&[os("lookup"), p(&mdx), os("食べる")]);
    assert_eq!(stdout(&output), "<p>to eat</p>\n");
}

#[test]
fn prints_a_redirect_as_written_instead_of_following_it() {
    let (_dir, mdx, _) = fixtures();

    let output = run(&[os("lookup"), p(&mdx), os("colour")]);

    assert!(output.status.success());
    assert_eq!(stdout(&output), "@@@LINK=color\n");
}

#[test]
fn fails_when_the_headword_is_missing() {
    let (_dir, mdx, _) = fixtures();

    let output = run(&[os("lookup"), p(&mdx), os("pear")]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn extracts_a_resource_to_a_file() {
    let (dir, _, mdd) = fixtures();
    let out = dir.path().join("a.png");

    let output = run(&[os("extract"), p(&mdd), os("img/a.png"), p(&out)]);

    assert!(output.status.success());
    assert_eq!(std::fs::read(&out).unwrap(), IMAGE);
}

#[test]
fn extracts_a_resource_to_standard_output() {
    let (_dir, _, mdd) = fixtures();

    let output = run(&[os("extract"), p(&mdd), os("\\img\\a.png")]);

    assert!(output.status.success());
    assert_eq!(output.stdout, IMAGE);
}

#[test]
fn fails_when_the_resource_is_missing() {
    let (_dir, _, mdd) = fixtures();

    let output = run(&[os("extract"), p(&mdd), os("img/b.png")]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[test]
fn reports_an_unreadable_file_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.mdx");
    std::fs::write(&path, b"not a dictionary").unwrap();

    let output = run(&[os("headwords"), p(&path)]);

    assert_eq!(output.status.code(), Some(1));
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("not an MDict file"), "{message}");
}

#[test]
fn explains_its_usage_when_called_wrongly() {
    for args in [
        &[][..],
        &[os("lookup")][..],
        &[os("frobnicate"), os("x")][..],
    ] {
        let output = run(args);

        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(message.contains("Usage"), "{message}");
    }
}
