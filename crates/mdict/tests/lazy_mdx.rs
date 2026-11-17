//! Reading an MDX file lazily, for callers that keep their own index of headwords:
//! opening reads no key block, keys are read once to build the index, and records are
//! read afterwards by where they lie.

use opendict_mdict::{Error, LazyMdx, Record};
use opendict_mdict_fixture::{Compression, Damage, Encoding, MdxBuilder, Version};

fn sample() -> MdxBuilder {
    MdxBuilder::new()
        .title("Lazy Sample")
        .entry("apple", "<p>a fruit</p>")
        .entry("bank", "<p>the side of a river</p>")
        .entry("bank", "<p>a place for money</p>")
        .link("colour", "字典")
        .entry("字典", "<p>中文字典</p>")
        .keys_per_block(2)
        .records_per_block(2)
}

fn open(bytes: &[u8]) -> opendict_mdict::Result<LazyMdx> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.mdx");
    std::fs::write(&path, bytes).unwrap();
    LazyMdx::open(&path)
}

/// Every key with its record, read the way an index would: keys first, then each record
/// from its start up to the next larger start (or the end of the record stream).
fn read_all(mdx: &LazyMdx) -> Vec<(String, Record)> {
    let keys: Vec<(String, u64)> = mdx.keys().collect::<Result<_, _>>().unwrap();
    let mut starts: Vec<u64> = keys.iter().map(|(_, start)| *start).collect();
    starts.sort_unstable();
    starts.dedup();
    keys.into_iter()
        .map(|(headword, start)| {
            let next = starts.partition_point(|&s| s <= start);
            let end = starts.get(next).copied().unwrap_or(mdx.record_stream_len());
            (headword, mdx.record(start..end).unwrap())
        })
        .collect()
}

#[test]
fn reads_every_key_in_file_order_and_each_record_by_where_it_lies() {
    let mdx = open(&sample().build()).unwrap();

    assert_eq!(mdx.header().title(), "Lazy Sample");
    assert_eq!(mdx.key_count(), 5);
    assert_eq!(
        read_all(&mdx),
        [
            (
                "apple".to_owned(),
                Record::Text("<p>a fruit</p>".to_owned())
            ),
            (
                "bank".to_owned(),
                Record::Text("<p>the side of a river</p>".to_owned())
            ),
            (
                "bank".to_owned(),
                Record::Text("<p>a place for money</p>".to_owned())
            ),
            ("colour".to_owned(), Record::Link("字典".to_owned())),
            (
                "字典".to_owned(),
                Record::Text("<p>中文字典</p>".to_owned())
            ),
        ]
    );
}

#[test]
fn reads_every_version_encoding_and_compression() {
    for version in Version::ALL {
        for compression in Compression::ALL {
            for encoding in Encoding::ALL {
                let case = format!("{version:?} {compression:?} {encoding:?}");
                let bytes = sample()
                    .version(version)
                    .compression(compression)
                    .encoding(encoding)
                    .build();
                let mdx = open(&bytes).unwrap_or_else(|e| panic!("{case}: {e}"));
                let all = read_all(&mdx);
                assert_eq!(all.len(), 5, "{case}");
                assert_eq!(
                    all[4],
                    (
                        "字典".to_owned(),
                        Record::Text("<p>中文字典</p>".to_owned())
                    ),
                    "{case}"
                );
            }
        }
    }
}

#[test]
fn reads_an_encrypted_key_index() {
    let mdx = open(&sample().encrypt_key_index().build()).unwrap();

    assert_eq!(read_all(&mdx).len(), 5);
}

#[test]
fn opening_reads_no_key_block_so_a_damaged_one_is_reported_when_keys_are_read() {
    let mdx = open(&sample().build_damaged(Damage::KeyBlock)).unwrap();

    let keys: Result<Vec<_>, _> = mdx.keys().collect();

    assert!(matches!(keys, Err(Error::Corrupt(_))), "{keys:?}");
}

#[test]
fn a_damaged_key_index_or_header_is_still_reported_at_open() {
    for damage in [
        Damage::HeaderChecksum,
        Damage::KeyIndex,
        Damage::Truncate(10),
    ] {
        assert!(
            open(&sample().build_damaged(damage)).is_err(),
            "{damage:?} opened"
        );
    }
}

#[test]
fn a_damaged_record_block_is_reported_when_its_record_is_read() {
    let mdx = open(&sample().build_damaged(Damage::RecordBlock)).unwrap();
    let keys: Vec<(String, u64)> = mdx.keys().collect::<Result<_, _>>().unwrap();

    let first = mdx.record(keys[0].1..keys[1].1);

    assert!(matches!(first, Err(Error::Corrupt(_))), "{first:?}");
}

#[test]
fn a_record_span_outside_the_record_stream_is_an_error_not_a_panic() {
    let mdx = open(&sample().build()).unwrap();
    let end = mdx.record_stream_len();

    assert!(mdx.record(end..end + 10).is_err());
    #[allow(clippy::reversed_empty_ranges)]
    let backwards = 5..2;
    assert!(mdx.record(backwards).is_err());
}

#[test]
fn a_registration_code_encryption_is_its_own_error() {
    let bytes = sample().attribute("Encrypted", "1").build();

    assert!(matches!(open(&bytes), Err(Error::Encrypted)));
}
