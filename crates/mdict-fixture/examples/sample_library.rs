//! Writes a small sample Library Folder for trying the app and for its end-to-end tests.
//!
//! `cargo run -p opendict-mdict-fixture --example sample_library -- <folder>`
//!
//! Every Entry is a sample written for OpenDict, not taken from a real dictionary.
//!
//! `sample-bought.mdx` needs a registration code (`Encrypted="1"`): it opens with the
//! e-mail `reader@example.com` and the code `0123456789ABCDEF0123456789ABCDEF`.

use std::path::PathBuf;

use opendict_mdict_fixture::{Identity, MdxBuilder};

fn main() -> std::io::Result<()> {
    let folder: PathBuf = std::env::args_os()
        .nth(1)
        .map(Into::into)
        .unwrap_or_else(|| "sample-library".into());
    std::fs::create_dir_all(&folder)?;

    MdxBuilder::new()
        .title("Sample English Dictionary")
        .entry(
            "apple",
            r#"<p><b>apple</b> <i>noun</i></p><ol><li>The round fruit of a tree of the rose family. (sample)</li></ol>"#,
        )
        .link("apples", "apple")
        .entry(
            "serendipity",
            r#"<style>.def { font-family: serif; } .ex { color: #555; }</style><p class="def"><b>serendipity</b> <i>noun</i></p><ol><li>Finding something good without looking for it. <span class="ex">It was pure serendipity. (sample)</span></li></ol>"#,
        )
        .write_to(folder.join("sample-english.mdx"))?;

    MdxBuilder::new()
        .title("サンプル国語辞典")
        .entry(
            "食べる",
            "<p><b>たべる【食べる】</b></p><p>食物を口に入れ、かんで飲み込む。（見本）</p>",
        )
        .write_to(folder.join("sample-japanese.mdx"))?;

    MdxBuilder::new()
        .title("Sample Bought Dictionary")
        .entry(
            "bargain",
            "<p><b>bargain</b> <i>noun</i></p><p>Something bought for less than it is worth. (sample)</p>",
        )
        .registered_to(
            "reader@example.com",
            "0123456789ABCDEF0123456789ABCDEF",
            Identity::Utf16,
        )
        .write_to(folder.join("sample-bought.mdx"))?;

    println!("{}", folder.display());
    Ok(())
}
