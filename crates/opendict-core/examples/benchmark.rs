//! Local benchmark against real Dictionaries: index build time, start-up with stored
//! indexes, Lookup latency and headword-index (prefix) latency.
//!
//! ```text
//! OPENDICT_TEST_DICTS=<folder of .mdx files> cargo run --release -p opendict-core --example benchmark
//! ```
//!
//! Prints counts and timings only, never Dictionary content. See `docs/benchmarks.md`.

use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use opendict_core::{IndexState, Library, Start};

/// Where in the index the sampled headwords start: Latin, kana and kanji.
const SAMPLE_KEYS: &[&str] = &[
    "a", "e", "k", "m", "s", "t", "あ", "か", "さ", "た", "な", "は", "ま", "や", "ら", "日", "中",
    "人", "大", "生",
];

fn main() {
    let Some(folder) = std::env::var_os("OPENDICT_TEST_DICTS") else {
        eprintln!("Set OPENDICT_TEST_DICTS to a folder of MDict files.");
        std::process::exit(2);
    };
    let data = tempfile::tempdir().expect("a temporary data directory");
    let file = data.path().join("library.json");

    // First start: scan (opening every Dictionary lazily), then build every index.
    let started = Instant::now();
    let mut library = Library::load(&file).expect("load");
    library
        .add_folder(folder.as_ref())
        .expect("add the Library Folder");
    let scan = started.elapsed();
    let dictionaries = library.dictionaries().count();
    let entries: usize = library.dictionaries().map(|d| d.entry_count).sum();
    println!(
        "first scan: {dictionaries} dictionaries, {entries} headwords, {} ms",
        scan.as_millis()
    );

    let mut slowest = (Duration::ZERO, 0usize);
    let started = Instant::now();
    for job in library.index_jobs() {
        let count = job.entry_count();
        let one = Instant::now();
        let outcome = job.run(&mut |_| ControlFlow::Continue(()));
        library.finish_index(outcome);
        let took = one.elapsed();
        if took > slowest.0 {
            slowest = (took, count);
        }
    }
    let build = started.elapsed();
    let failed = library
        .dictionaries()
        .filter(|d| matches!(d.index, IndexState::Failed(_)))
        .count();
    let stored: u64 = std::fs::read_dir(data.path().join("indexes"))
        .map(|entries| {
            entries
                .filter_map(|e| e.ok()?.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0);
    println!(
        "index build: {} ms in all ({:.0} headwords/ms), slowest {} ms for {} headwords, {failed} failed, {} MiB stored",
        build.as_millis(),
        entries as f64 / build.as_millis().max(1) as f64,
        slowest.0.as_millis(),
        slowest.1,
        stored / (1 << 20)
    );
    drop(library);

    // Later start: the stored indexes are reused.
    let started = Instant::now();
    let library = Library::load(&file).expect("load again");
    let restart = started.elapsed();
    let pending = library.index_jobs().len();
    println!(
        "later start: {} ms, {pending} dictionaries to index",
        restart.as_millis()
    );

    // Lookup: headwords sampled from every Dictionary's index.
    let mut sample = Vec::new();
    for dictionary in library.dictionaries() {
        let Some(index) = library.index_of(&dictionary.id) else {
            continue;
        };
        for key in SAMPLE_KEYS {
            if let Ok(found) = index.headwords(Start::AtKey(key), 2) {
                sample.extend(found.into_iter().map(|h| h.headword));
            }
        }
    }
    let mut times = Vec::new();
    let mut answers = 0;
    for headword in &sample {
        let one = Instant::now();
        let found = library.look_up(headword).unwrap_or_default();
        answers += found.len();
        times.push(one.elapsed());
        if std::env::var_os("BENCH_DEBUG").is_some() && one.elapsed().as_millis() > 100 {
            let bytes: usize = found.iter().map(|e| e.html.len()).sum();
            eprintln!(
                "slow {} ms: {} entries, {} bytes",
                one.elapsed().as_millis(),
                found.len(),
                bytes
            );
        }
    }
    report("look_up", &mut times);
    println!("  {} lookups, {answers} entries", sample.len());

    // Headword index: one prefix query per sampled key across every Dictionary.
    let mut times = Vec::new();
    for key in SAMPLE_KEYS {
        let one = Instant::now();
        for dictionary in library.dictionaries() {
            if let Some(index) = library.index_of(&dictionary.id) {
                let _ = index.headwords(Start::AtKey(key), 50);
            }
        }
        times.push(one.elapsed());
    }
    report("50 headwords from every dictionary", &mut times);
}

fn report(what: &str, times: &mut [Duration]) {
    if times.is_empty() {
        println!("{what}: no samples");
        return;
    }
    times.sort();
    let micros = |d: Duration| d.as_secs_f64() * 1e6;
    let mean = times.iter().map(|&d| micros(d)).sum::<f64>() / times.len() as f64;
    println!(
        "{what}: median {:.0} µs, mean {mean:.0} µs, p95 {:.0} µs, max {:.0} µs",
        micros(times[times.len() / 2]),
        micros(times[times.len() * 95 / 100]),
        micros(times[times.len() - 1]),
    );
}
