# opendict-mdict

A reader for MDict dictionary files (`.mdx`), written from scratch in Rust from public descriptions of the format. Part of [OpenDict](https://github.com/amemiya02/OpenDict), but usable on its own.

```rust
let mdx = opendict_mdict::Mdx::open("dictionary.mdx")?;
println!("{}", mdx.header().title());
for record in mdx.lookup("serendipity")? {
    println!("{record:?}");
}
# Ok::<(), opendict_mdict::Error>(())
```

Currently reads format 2.0 files in UTF-8 with zlib compression. Format 1.2, other encodings, LZO, encryption and MDD resources are planned.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT licence](LICENSE-MIT) at your option. The OpenDict application itself is GPL-3.0-or-later; this crate is not.
