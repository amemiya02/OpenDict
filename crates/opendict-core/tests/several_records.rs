//! A Dictionary may file several records under one headword: they are one Entry.

use opendict_core::Library;
use opendict_mdict_fixture::MdxBuilder;

#[test]
fn several_records_under_one_headword_stay_together_in_one_entry() {
    let folder = tempfile::tempdir().unwrap();
    MdxBuilder::new()
        .title("Sample Homographs")
        .entry("bank", "<p>1. the side of a river (sample)</p>")
        .entry("bank", "<p>2. a place that keeps money (sample)</p>")
        .entry("bar", "<p>a long rigid piece (sample)</p>")
        .write_to(folder.path().join("homographs.mdx"))
        .unwrap();
    MdxBuilder::new()
        .title("Sample Second")
        .entry("bank", "<p>a raised shelf of ground (sample)</p>")
        .write_to(folder.path().join("second.mdx"))
        .unwrap();

    let library = Library::open(folder.path()).unwrap();
    let entries = library.look_up("bank").unwrap();

    let titles: Vec<&str> = entries.iter().map(|e| e.dictionary_title.as_str()).collect();
    assert_eq!(titles, ["Sample Homographs", "Sample Second"]);
    let html = &entries[0].html;
    let first = html.find("the side of a river").expect("first record");
    let second = html.find("a place that keeps money").expect("second record");
    assert!(first < second, "records keep the file's order: {html}");
    assert!(!html.contains("a long rigid piece"), "{html}");
}
