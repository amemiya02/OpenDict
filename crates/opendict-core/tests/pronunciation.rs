//! The page head shows how the headword is said, where an Entry shows that plainly.
//! All markup here is written for these tests, in the shapes real Dictionaries use.

use opendict_core::pronunciation;

fn said(html: &str) -> Option<String> {
    pronunciation(html)
}

#[test]
fn reads_an_element_marked_as_a_pronunciation() {
    assert_eq!(
        said(r#"<h3>se·rif <span class="phon">/ˈserɪf/</span></h3><p>a small line (sample)</p>"#),
        Some("/ˈserɪf/".to_owned())
    );
}

#[test]
fn knows_the_marks_dictionaries_use_in_any_case_among_other_classes() {
    let cases = [
        (
            r#"<span class="headword HATSUON">/səˈriːn/</span>"#,
            "/səˈriːn/",
        ),
        (
            r#"<span class='pr'><span class="ph ty_pinyin">xūyú <d:prn/></span></span>"#,
            "xūyú",
        ),
        (r#"<span data-type="発音">こもれび</span>"#, "こもれび"),
        (r#"<i class=ipa>rʌn</i>"#, "rʌn"),
    ];
    for (html, expected) in cases {
        assert_eq!(said(html).as_deref(), Some(expected), "{html}");
    }
}

#[test]
fn gives_the_text_alone_with_references_decoded_and_space_collapsed() {
    assert_eq!(
        said("<span class=\"pron\">\n  /r&#652;n/ <b>&amp;</b>  /r&#x28C;n&#x2D0;/ </span>"),
        Some("/rʌn/ & /rʌnː/".to_owned())
    );
}

#[test]
fn passes_over_a_marked_element_that_holds_more_than_a_pronunciation() {
    let long = "a whole paragraph of explanation about how this word is said in several regions";
    assert_eq!(
        said(&format!(
            r#"<div class="pron">{long}</div><span class="ipa">/ˈɡlɒsəri/</span>"#
        )),
        Some("/ˈɡlɒsəri/".to_owned())
    );
    assert_eq!(said(r#"<span class="pron"> </span>"#), None);
}

#[test]
fn finds_a_transcription_between_slashes_by_its_ipa_letters() {
    assert_eq!(
        said("<b>serendipity</b> /ˌserənˈdɪpəti/ <i>n.</i> a happy accident (sample)"),
        Some("/ˌserənˈdɪpəti/".to_owned())
    );
    assert_eq!(
        said("<p>either/or, 1/2/3 and http://example.org/a</p><p>/ˈaɪðə/</p>"),
        Some("/ˈaɪðə/".to_owned()),
        "slashes without IPA letters are not a transcription"
    );
}

#[test]
fn reads_the_kana_before_a_japanese_headword_in_brackets() {
    assert_eq!(
        said(
            r#"<link rel="stylesheet" href="sample.css"/><span class="項目">たべる【食べる】</span><br>[他下一]"#
        ),
        Some("たべる".to_owned())
    );
    assert_eq!(said("<b>カフェ【café】</b>"), Some("カフェ".to_owned()));
    assert_eq!(
        said("<b>食べる【たべる】</b>"),
        None,
        "kanji before the brackets is not a reading"
    );
}

#[test]
fn says_nothing_when_the_entry_does_not_show_it() {
    assert_eq!(
        said("<p>A plain entry with no pronunciation (sample).</p>"),
        None
    );
    assert_eq!(said(""), None);
    assert_eq!(
        said("<script>var a = '/ˈfeɪk/';</script><style>.phon{}</style><p>plain (sample)</p>"),
        None,
        "scripts and styles are not text"
    );
}

#[test]
fn survives_broken_markup() {
    for html in [
        "<",
        "<span class=\"phon",
        "&#xFFFFFFFF; &",
        "</b></i>text",
        "<!-- open",
        "<script>",
    ] {
        let _ = said(html);
    }
    assert_eq!(said("<span class=\"phon\">/ˈx"), Some("/ˈx".to_owned()));
}
