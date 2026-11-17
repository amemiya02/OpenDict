//! Writes a small sample Library Folder for trying the app and for its end-to-end tests.
//!
//! `cargo run -p opendict-mdict-fixture --example sample_library -- <folder>`
//!
//! Every Entry is a sample written for OpenDict, not taken from a real dictionary.
//!
//! `sample-bought.mdx` needs a registration code (`Encrypted="1"`): it opens with the
//! e-mail `reader@example.com` and the code `0123456789ABCDEF0123456789ABCDEF`.

use std::path::PathBuf;

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use opendict_mdict_fixture::{Identity, MddBuilder, MdxBuilder};

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
            r#"<style>.def { font-family: serif; } .ex { color: #555; }</style><p class="def" id="top"><b>serendipity</b> <i>noun</i></p><ol><li>Finding something good without looking for it. <span class="ex">It was pure serendipity. (sample)</span></li></ol><p class="say"><a href="sound://serendipity.wav">Listen</a></p><p class="see">See also <a href="entry://serendipitous">serendipitous</a>, <a href="entry://#top">top</a>.</p>"#,
        )
        .entry(
            "serendipitous",
            r#"<p><b>serendipitous</b> <i>adjective</i></p><ol><li>Found by happy accident. (sample)</li></ol><p>See <a href="entry://serendipity">serendipity</a>.</p>"#,
        )
        .link("happy accident", "serendipities")
        .link("serendipities", "serendipity")
        .entry(
            "thumb",
            &long_entry(
                "<p><b>thumb</b> /θʌm/ <i>noun</i></p>",
                "The short thick first digit of the hand. (sample)",
            ),
        )
        .write_to(folder.join("sample-english.mdx"))?;
    MddBuilder::new()
        .resource("\\serendipity.wav", &sample_tone())
        .write_to(folder.join("sample-english.mdd"))?;

    MdxBuilder::new()
        .title("サンプル国語辞典")
        .entry(
            "食べる",
            "<p><b>たべる【食べる】</b></p><p>食物を口に入れ、かんで飲み込む。（見本）</p>",
        )
        .entry(
            "thumb",
            &long_entry(
                "<p><b>サム【thumb】</b></p>",
                "手の親指。つめ見出しのこと。（見本）",
            ),
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

    // A Dictionary styled the way real ones are: a stylesheet beside the MDX and an
    // image in its MDD.
    MdxBuilder::new()
        .title("Sample Illustrated Dictionary")
        .entry(
            "cat",
            r#"<link rel="stylesheet" href="sample-illustrated.css"><p class="hw">cat <span class="pos">noun</span></p><img class="pic" src="img/cat.png" alt="a sample picture"><p>A small furry animal kept as a pet. (sample)</p>"#,
        )
        .entry(
            "thumb",
            &long_entry(
                r#"<link rel="stylesheet" href="sample-illustrated.css"><p class="hw">thumb <span class="pos">noun</span></p>"#,
                "A notch cut into the fore-edge of a dictionary. (sample)",
            ),
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

/// An Entry taller than a window: `head`, then `sense` as forty numbered senses.
/// "thumb" has one in every sample Dictionary, so the thumb index has a page to move about.
fn long_entry(head: &str, sense: &str) -> String {
    let senses: String = (1..=40).map(|n| format!("<li>{sense} {n}</li>")).collect();
    format!("{head}<ol>{senses}</ol>")
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

/// A quarter-second 440 Hz tone as a WAV file: a sample sound written for OpenDict.
fn sample_tone() -> Vec<u8> {
    const RATE: u32 = 8000;
    let samples: Vec<i16> = (0..RATE / 4)
        .map(|n| {
            let t = n as f64 / f64::from(RATE);
            ((t * 440.0 * std::f64::consts::TAU).sin() * 8000.0) as i16
        })
        .collect();
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(RATE.to_le_bytes());
    out.extend((RATE * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    for sample in samples {
        out.extend(sample.to_le_bytes());
    }
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
