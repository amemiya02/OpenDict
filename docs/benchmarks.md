# Benchmarks

There are no numeric performance targets (see `docs/roadmap.md`), but index build time and
Lookup latency are measured locally against real Dictionaries to catch regressions. Real
Dictionaries are never committed; the benchmark prints counts and timings only.

## Running it

```sh
OPENDICT_TEST_DICTS=<folder of .mdx files> cargo run --release -p opendict-core --example benchmark
```

It loads a fresh Library (in a temporary data directory) with that folder as its Library
Folder, and reports:

- **first scan**: finding and opening every Dictionary (opening reads only headers and
  section tables).
- **index build**: building every index, with the slowest Dictionary and the size stored.
- **later start**: loading the Library again with its stored indexes.
- **look_up**: `Library::look_up` for headwords sampled from every index (40 per
  Dictionary, at Latin, kana and kanji starting points), across the whole Library.
- **50 headwords from every dictionary**: one prefix query per starting point on every
  Dictionary's index, as the headword column asks.

Run it twice and keep the second result: the first run also measures reading the
Dictionary files from a cold disk.

## Results

Release build, Windows 11, Intel Core i9-13900H, 32 GB RAM, Dictionaries on a local SSD.
Compare against the line for the same collection; record new lines when the code changes
what is measured.

| Date | Commit | Collection | First scan | Index build (slowest) | Stored | Later start | look_up median / p95 / max | Prefix query, all Dictionaries |
|---|---|---|---|---|---|---|---|---|
| 2026-11-17 | ticket/6 | 36 MDX, 8.73 M headwords | 18 ms | 5.1–5.7 s (0.8–1.2 s, 1.46 M headwords) | 407 MiB | 18–23 ms | 1.9 / 18–19 / 40–49 ms | 0.26–0.37 ms |
| 2026-11-17 | ticket/6 | 1 MDX, 187 k headwords | 1 ms | 91 ms | 8 MiB | < 1 ms | 0.4 / 1.1 / 1.4 ms | 9 µs |

Notes:

- The slowest Lookups return the most text: single-kana headwords with two dozen Entries and
  over 1 MB of HTML. Each record block is decompressed once per Lookup (the parser keeps the
  last block it decompressed); there is no cache across Lookups.
- Before the index, opening the same 36 Dictionaries read every headword into memory:
  about 2 s per start in a release build, and several hundred megabytes of memory.
