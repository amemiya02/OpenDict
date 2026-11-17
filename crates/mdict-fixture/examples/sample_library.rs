//! Writes a small sample Library Folder for trying the app and for its end-to-end tests.
//!
//! `cargo run -p opendict-mdict-fixture --example sample_library -- <folder>`
//!
//! Every Entry is a sample written for OpenDict, not taken from a real dictionary.

use std::path::PathBuf;

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use opendict_mdict_fixture::{MddBuilder, MdxBuilder};

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

    // A Dictionary styled the way real ones are: a stylesheet beside the MDX and an
    // image in its MDD.
    MdxBuilder::new()
        .title("Sample Illustrated Dictionary")
        .entry(
            "cat",
            r#"<link rel="stylesheet" href="sample-illustrated.css"><p class="hw">cat <span class="pos">noun</span></p><img class="pic" src="img/cat.png" alt="a sample picture"><p>A small furry animal kept as a pet. (sample)</p>"#,
        )
        .write_to(folder.join("sample-illustrated.mdx"))?;
    std::fs::write(
        folder.join("sample-illustrated.css"),
        ".hw { color: rgb(0, 128, 128); font-weight: bold; } .pos { font-style: italic; }",
    )?;
    MddBuilder::new()
        .resource("\\img\\cat.png", &sample_png(8, 8))
        .write_to(folder.join("sample-illustrated.mdd"))?;

    println!("{}", folder.display());
    Ok(())
}

/// A plain teal PNG, `width` x `height` pixels: a sample picture written for OpenDict.
fn sample_png(width: u32, height: u32) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend(kind);
        out.extend(data);
        let crc = crc32(&out[start..]);
        out.extend(crc.to_be_bytes());
    }
    let mut header = Vec::new();
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]); // 8-bit RGB
    // Each row: filter type 0 (none), then teal RGB pixels.
    let row: Vec<u8> = std::iter::once(0)
        .chain([0, 128, 128].repeat(width as usize))
        .collect();
    let pixels = row.repeat(height as usize);
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&pixels)
        .expect("writing to a Vec cannot fail");
    let data = encoder.finish().expect("writing to a Vec cannot fail");

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &data);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
