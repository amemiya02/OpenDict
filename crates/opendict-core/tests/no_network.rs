//! The core never touches the network: nothing in it, in the parser, or in anything they
//! depend on can open a connection. Rust has no portable way to watch a process's sockets,
//! so this checks every way a connection could be made: the standard library's network
//! types in the source, and networking crates among the dependencies.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Crates that open network connections, or exist to.
const NETWORKING_CRATES: &[&str] = &[
    "async-std",
    "attohttpc",
    "curl",
    "h2",
    "h3",
    "hyper",
    "isahc",
    "mio",
    "native-tls",
    "openssl",
    "quinn",
    "reqwest",
    "rustls",
    "smol",
    "socket2",
    "surf",
    "tokio",
    "trust-dns-resolver",
    "hickory-resolver",
    "tungstenite",
    "ureq",
    "websocket",
];

/// What the standard library opens connections and resolves names with.
const NETWORKING_APIS: &[&str] = &[
    "std::net",
    "core::net",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "ToSocketAddrs",
    "UnixStream",
    "UnixDatagram",
];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_owned()
}

/// The names of every crate `package` is built with (its dependencies, not its tests').
fn dependencies(package: &str) -> Vec<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .current_dir(workspace())
        .args([
            "tree",
            "--package",
            package,
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--locked",
            "--offline",
        ])
        .output()
        .expect("cargo runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut names: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

fn rust_files(folder: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn the_core_and_the_parser_depend_on_no_networking_crate() {
    let core = dependencies("opendict-core");
    assert!(
        core.iter().any(|name| name == "opendict-mdict"),
        "the dependency list is read: {core:?}"
    );

    let networking: Vec<&String> = core
        .iter()
        .filter(|name| NETWORKING_CRATES.contains(&name.as_str()))
        .collect();

    assert!(networking.is_empty(), "networking crates: {networking:?}");
}

#[test]
fn the_core_and_the_parser_use_none_of_the_standard_network_types() {
    let mut files = Vec::new();
    rust_files(&workspace().join("crates/opendict-core/src"), &mut files);
    rust_files(&workspace().join("crates/mdict/src"), &mut files);
    assert!(files.len() > 10, "the sources are found: {files:?}");

    let uses: Vec<String> = files
        .iter()
        .flat_map(|file| {
            let text = std::fs::read_to_string(file).unwrap();
            NETWORKING_APIS
                .iter()
                .filter(|api| text.contains(*api))
                .map(|api| format!("{}: {api}", file.display()))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(uses.is_empty(), "network use: {uses:?}");
}
