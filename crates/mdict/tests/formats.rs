//! Every combination of format version, encoding, compression and key index encryption
//! the parser reads, plus damaged and unsupported files, written by the fixture writer.

use opendict_mdict::{Error, Mdd, Mdx, Record};
use opendict_mdict_fixture::{Compression, Damage, Encoding, MddBuilder, MdxBuilder, Version};

/// Every (version, compression, encrypted key index) layout. Version 1.2 stores its key
/// index raw, so it has no encrypted variant.
fn layouts() -> Vec<(Version, Compression, bool)> {
    let mut layouts = Vec::new();
    for version in Version::ALL {
        for compression in Compression::ALL {
            layouts.push((version, compression, false));
            if version == Version::V2_0 {
                layouts.push((version, compression, true));
            }
        }
    }
    layouts
}

fn mdx(
    version: Version,
    encoding: Encoding,
    compression: Compression,
    encrypt: bool,
) -> MdxBuilder {
    // Every headword and record can be written in all four encodings.
    let builder = MdxBuilder::new()
        .version(version)
        .encoding(encoding)
        .compression(compression)
        .entry("apple", "<p>a fruit</p>")
        .entry("bank", "<p>the side of a river</p>")
        .entry("bank", "<p>a place for money</p>")
        .link("colour", "字典")
        .entry("字典", "<p>中文字典</p>")
        .keys_per_block(2)
        .records_per_block(2);
    if encrypt {
        builder.encrypt_key_index()
    } else {
        builder
    }
}

fn mdd(version: Version, compression: Compression, encrypt: bool) -> MddBuilder {
    let builder = MddBuilder::new()
        .version(version)
        .compression(compression)
        .resource("\\img\\a.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR")
        .resource("\\style.css", b".hw { font-weight: bold; }")
        .resource("\\字典.css", b"p { margin: 0; }")
        .keys_per_block(2)
        .records_per_block(2);
    if encrypt {
        builder.encrypt_key_index()
    } else {
        builder
    }
}

fn open_mdx(bytes: &[u8]) -> opendict_mdict::Result<Mdx> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.mdx");
    std::fs::write(&path, bytes).unwrap();
    Mdx::open(&path)
}

fn open_mdd(bytes: &[u8]) -> opendict_mdict::Result<Mdd> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.mdd");
    std::fs::write(&path, bytes).unwrap();
    Mdd::open(&path)
}

#[test]
fn reads_mdx_in_every_version_encoding_and_compression() {
    for (version, compression, encrypt) in layouts() {
        for encoding in Encoding::ALL {
            let case = format!("{version:?} {encoding:?} {compression:?} encrypted={encrypt}");
            let mdx = open_mdx(&mdx(version, encoding, compression, encrypt).build())
                .unwrap_or_else(|e| panic!("{case}: {e}"));

            assert_eq!(
                mdx.headwords().collect::<Vec<_>>(),
                ["apple", "bank", "bank", "colour", "字典"],
                "{case}"
            );
            assert_eq!(
                mdx.lookup("字典").unwrap(),
                [Record::Text("<p>中文字典</p>".into())],
                "{case}"
            );
            assert_eq!(
                mdx.lookup("bank").unwrap(),
                [
                    Record::Text("<p>the side of a river</p>".into()),
                    Record::Text("<p>a place for money</p>".into())
                ],
                "{case}"
            );
            assert_eq!(
                mdx.lookup("colour").unwrap(),
                [Record::Link("字典".into())],
                "{case}"
            );
        }
    }
}

#[test]
fn reads_mdd_in_every_version_and_compression() {
    for (version, compression, encrypt) in layouts() {
        let case = format!("{version:?} {compression:?} encrypted={encrypt}");
        let mdd = open_mdd(&mdd(version, compression, encrypt).build())
            .unwrap_or_else(|e| panic!("{case}: {e}"));

        assert_eq!(
            mdd.resource_paths().collect::<Vec<_>>(),
            ["\\img\\a.png", "\\style.css", "\\字典.css"],
            "{case}"
        );
        assert_eq!(
            mdd.resource("img/a.png").unwrap().as_deref(),
            Some(&b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"[..]),
            "{case}"
        );
        assert_eq!(
            mdd.resource("字典.css").unwrap().as_deref(),
            Some(&b"p { margin: 0; }"[..]),
            "{case}"
        );
    }
}

/// What a reader must say about a damaged file: never a panic, never wrong content.
fn assert_damage_reported(case: &str, damage: Damage, open: opendict_mdict::Result<()>) {
    match (damage, open) {
        (Damage::HeaderChecksum, Err(Error::NotMdict(_))) => {}
        (_, Err(Error::Corrupt(_))) => {}
        (_, other) => panic!("{case} {damage:?}: expected a damaged-file error, got {other:?}"),
    }
}

const OPEN_DAMAGES: [Damage; 4] = [
    Damage::HeaderChecksum,
    Damage::KeyIndex,
    Damage::KeyBlock,
    Damage::Truncate(10),
];

#[test]
fn reports_damaged_mdx_files_in_every_layout() {
    for (version, compression, encrypt) in layouts() {
        for encoding in Encoding::ALL {
            let case = format!("{version:?} {encoding:?} {compression:?} encrypted={encrypt}");
            let builder = mdx(version, encoding, compression, encrypt);
            for damage in OPEN_DAMAGES {
                let opened = open_mdx(&builder.build_damaged(damage)).map(drop);
                assert_damage_reported(&case, damage, opened);
            }

            // Records are read when looked up, so a damaged record block shows then.
            let mdx = open_mdx(&builder.build_damaged(Damage::RecordBlock))
                .unwrap_or_else(|e| panic!("{case}: {e}"));
            assert_damage_reported(&case, Damage::RecordBlock, mdx.lookup("apple").map(drop));
        }
    }
}

#[test]
fn reports_damaged_mdd_files_in_every_layout() {
    for (version, compression, encrypt) in layouts() {
        let case = format!("{version:?} {compression:?} encrypted={encrypt}");
        let builder = mdd(version, compression, encrypt);
        for damage in OPEN_DAMAGES.into_iter().filter(|d| *d != Damage::KeyBlock) {
            let opened = open_mdd(&builder.build_damaged(damage)).map(drop);
            assert_damage_reported(&case, damage, opened);
        }

        // Opening reads no key block and no record block: their damage shows when a
        // resource is read.
        for damage in [Damage::KeyBlock, Damage::RecordBlock] {
            let mdd =
                open_mdd(&builder.build_damaged(damage)).unwrap_or_else(|e| panic!("{case}: {e}"));
            assert_damage_reported(&case, damage, mdd.resource("img/a.png").map(drop));
        }
    }
}

#[test]
fn reports_a_registration_code_encryption_as_its_own_error() {
    let bytes = MdxBuilder::new()
        .entry("apple", "<p>a fruit</p>")
        .attribute("Encrypted", "1")
        .build();

    assert!(matches!(open_mdx(&bytes), Err(Error::Encrypted)));

    let bytes = MddBuilder::new()
        .resource("\\a.png", b"png")
        .attribute("Encrypted", "3")
        .build();

    assert!(matches!(open_mdd(&bytes), Err(Error::Encrypted)));
}

#[test]
fn rejects_format_version_3_and_others_it_does_not_know() {
    for version in ["3.0", "", "x"] {
        let bytes = MdxBuilder::new()
            .entry("apple", "<p>a fruit</p>")
            .attribute("GeneratedByEngineVersion", version)
            .build();

        assert!(
            matches!(open_mdx(&bytes), Err(Error::UnsupportedVersion(v)) if v == version),
            "version {version:?}"
        );
    }
}

#[test]
fn rejects_an_encoding_it_does_not_know() {
    let bytes = MdxBuilder::new()
        .entry("apple", "<p>a fruit</p>")
        .attribute("Encoding", "KOI8-R")
        .build();

    assert!(matches!(open_mdx(&bytes), Err(Error::Unsupported(_))));
}

#[test]
fn reads_encoding_names_as_real_files_write_them() {
    for (name, encoding) in [
        ("", Encoding::Utf8),
        ("utf8", Encoding::Utf8),
        ("UTF-16LE", Encoding::Utf16),
        ("GB2312", Encoding::Gbk),
        ("GB18030", Encoding::Gbk),
        ("Big5", Encoding::Big5),
    ] {
        let bytes = MdxBuilder::new()
            .encoding(encoding)
            .entry("字典", "<p>中文</p>")
            .attribute("Encoding", name)
            .build();

        let mdx = open_mdx(&bytes).unwrap_or_else(|e| panic!("{name:?}: {e}"));
        assert_eq!(
            mdx.lookup("字典").unwrap(),
            [Record::Text("<p>中文</p>".into())],
            "{name:?}"
        );
    }
}
