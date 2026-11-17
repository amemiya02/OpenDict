//! Files that need a registration code (`Encrypted="1"`): opened with the code and the
//! e-mail (or device ID) it was issued to, refused without them or with wrong ones.

use opendict_mdict::{Error, Mdd, Mdx, Record, Registration};
use opendict_mdict_fixture::{Compression, Identity, MddBuilder, MdxBuilder, Version};

const EMAIL: &str = "reader@example.com";
const CODE: &str = "0123456789ABCDEF0123456789ABCDEF";
const OTHER_CODE: &str = "FEDCBA9876543210FEDCBA9876543210";

fn sold(version: Version, identity: Identity) -> MdxBuilder {
    MdxBuilder::new()
        .version(version)
        .compression(Compression::Zlib)
        .entry("apple", "<p>a fruit</p>")
        .entry("bank", "<p>a place for money</p>")
        .keys_per_block(1)
        .registered_to(EMAIL, CODE, identity)
}

fn write(bytes: &[u8], name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    (dir, path)
}

fn registration(email: &str, code: &str) -> Registration {
    Registration::new(email, code).expect("a well-formed code")
}

#[test]
fn opens_with_the_right_code_and_email_in_every_version_and_identity_form() {
    for version in Version::ALL {
        for identity in [Identity::Utf16, Identity::Ascii] {
            let case = format!("{version:?} {identity:?}");
            let (_dir, path) = write(&sold(version, identity).build(), "sold.mdx");

            let mdx = Mdx::open_registered(&path, &registration(EMAIL, CODE))
                .unwrap_or_else(|e| panic!("{case}: {e}"));

            assert_eq!(mdx.header().encrypted(), 1, "{case}");
            assert_eq!(
                mdx.lookup("bank").unwrap(),
                [Record::Text("<p>a place for money</p>".into())],
                "{case}"
            );
        }
    }
}

#[test]
fn opens_a_file_whose_key_index_is_encrypted_as_well() {
    let bytes = sold(Version::V2_0, Identity::Utf16)
        .encrypt_key_index()
        .build();
    let (_dir, path) = write(&bytes, "sold.mdx");

    let mdx = Mdx::open_registered(&path, &registration(EMAIL, CODE)).unwrap();

    assert_eq!(mdx.header().encrypted(), 3);
    assert_eq!(mdx.headwords().collect::<Vec<_>>(), ["apple", "bank"]);
}

#[test]
fn needs_a_registration_to_open() {
    let (_dir, path) = write(&sold(Version::V2_0, Identity::Utf16).build(), "sold.mdx");

    assert!(matches!(Mdx::open(&path), Err(Error::Encrypted)));
}

#[test]
fn refuses_a_wrong_code_or_a_wrong_email() {
    for version in Version::ALL {
        let (_dir, path) = write(&sold(version, Identity::Utf16).build(), "sold.mdx");

        for (email, code) in [(EMAIL, OTHER_CODE), ("someone@example.com", CODE)] {
            let result = Mdx::open_registered(&path, &registration(email, code));
            assert!(
                matches!(result, Err(Error::WrongRegistration)),
                "{version:?} {email} {code}: {:?}",
                result.err()
            );
        }
    }
}

#[test]
fn uses_the_code_the_file_carries_when_only_the_email_is_given() {
    let bytes = sold(Version::V2_0, Identity::Utf16)
        .attribute("RegCode", CODE)
        .build();
    let (_dir, path) = write(&bytes, "sold.mdx");

    let mdx = Mdx::open_registered(&path, &Registration::without_code(EMAIL)).unwrap();
    assert_eq!(mdx.headwords().count(), 2);

    let (_dir, path) = write(&sold(Version::V2_0, Identity::Utf16).build(), "sold.mdx");
    assert!(matches!(
        Mdx::open_registered(&path, &Registration::without_code(EMAIL)),
        Err(Error::Encrypted)
    ));
}

#[test]
fn opens_a_resource_file_that_needs_a_registration() {
    for version in Version::ALL {
        let bytes = MddBuilder::new()
            .version(version)
            .resource("\\img\\a.png", b"\x89PNG\r\n\x1a\n")
            .registered_to(EMAIL, CODE, Identity::Utf16)
            .build();
        let (_dir, path) = write(&bytes, "sold.mdd");

        assert!(matches!(Mdd::open(&path), Err(Error::Encrypted)));
        assert!(matches!(
            Mdd::open_registered(&path, &registration(EMAIL, OTHER_CODE)),
            Err(Error::WrongRegistration)
        ));
        let mdd = Mdd::open_registered(&path, &registration(EMAIL, CODE)).unwrap();
        assert_eq!(
            mdd.resource("img/a.png").unwrap().as_deref(),
            Some(&b"\x89PNG\r\n\x1a\n"[..])
        );
    }
}

#[test]
fn a_registration_does_not_stop_an_unencrypted_file_opening() {
    let bytes = MdxBuilder::new().entry("apple", "<p>a fruit</p>").build();
    let (_dir, path) = write(&bytes, "free.mdx");

    let mdx = Mdx::open_registered(&path, &registration(EMAIL, CODE)).unwrap();
    assert_eq!(mdx.headwords().count(), 1);
}

#[test]
fn reads_codes_as_32_hex_digits_ignoring_spaces_and_dashes() {
    assert!(Registration::new(EMAIL, "0123 4567-89ab cdef 0123 4567 89AB CDEF").is_some());
    for malformed in [
        "",
        "0123",
        "0123456789ABCDEF0123456789ABCDEG",
        &format!("{CODE}00"),
    ] {
        assert!(
            Registration::new(EMAIL, malformed).is_none(),
            "{malformed:?}"
        );
    }
}

#[test]
fn never_prints_the_code_or_the_email() {
    let shown = format!("{:?}", registration(EMAIL, CODE));
    assert!(!shown.contains(EMAIL) && !shown.to_lowercase().contains(&CODE.to_lowercase()));
}
