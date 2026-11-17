# opendict-mdict

A reader for MDict dictionary files (`.mdx` and `.mdd`), written from scratch in Rust from public descriptions of the format. Part of [OpenDict](https://github.com/amemiya02/OpenDict), but usable on its own.

```rust
let mdx = opendict_mdict::Mdx::open("dictionary.mdx")?;
println!("{}", mdx.header().title());
for record in mdx.lookup("serendipity")? {
    println!("{record:?}");
}
# Ok::<(), opendict_mdict::Error>(())
```

```rust
let mdd = opendict_mdict::Mdd::open("dictionary.mdd")?;
let image: Option<Vec<u8>> = mdd.resource("img/figure.png")?;
# Ok::<(), opendict_mdict::Error>(())
```

Reads format versions 1.2 and 2.0; plain, zlib and LZO blocks; UTF-8, UTF-16, GBK and Big5; and files whose key index is encrypted (`Encrypted="2"`). Files that need a registration code (`Encrypted="1"`) report `Error::Encrypted`; format 3.0 reports `Error::UnsupportedVersion`. `@@@LINK=` redirects are reported as `Record::Link`, not followed.

## Command-line tool

```
opendict-mdict header <file>
opendict-mdict headwords <file>
opendict-mdict lookup <file.mdx> <headword>
opendict-mdict extract <file.mdd> <path> [<output>]
```

Install with `cargo install opendict-mdict`.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT licence](LICENSE-MIT) at your option. The OpenDict application itself is GPL-3.0-or-later; this crate is not.
