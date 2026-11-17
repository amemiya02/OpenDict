//! `opendict-mdict`: inspect an MDict file from the command line.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use opendict_mdict::{Header, Mdd, Mdx, Record};

const USAGE: &str = "\
Usage:
  opendict-mdict header <file>                    show the header attributes
  opendict-mdict headwords <file>                 list headwords (MDX) or resource paths (MDD)
  opendict-mdict lookup <file.mdx> <headword>     print the records for a headword
  opendict-mdict extract <file.mdd> <path> [<output>]
                                                  write a resource to <output>, or to standard output

Several records for one headword are separated by a line holding `</>`.
A redirect is printed as written (`@@@LINK=target`), not followed.";

enum Command {
    Header(PathBuf),
    Headwords(PathBuf),
    Lookup(PathBuf, String),
    Extract(PathBuf, String, Option<PathBuf>),
}

/// Why the tool stopped: the user asked for something absent, or something failed.
enum Failure {
    NotFound(String),
    Error(String),
}

impl From<opendict_mdict::Error> for Failure {
    fn from(error: opendict_mdict::Error) -> Self {
        Failure::Error(error.to_string())
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Failure::Error(error.to_string())
    }
}

fn main() -> ExitCode {
    let Some(command) = parse(std::env::args_os().skip(1).collect()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::NotFound(message) | Failure::Error(message)) => {
            eprintln!("opendict-mdict: {message}");
            ExitCode::from(1)
        }
    }
}

fn parse(args: Vec<OsString>) -> Option<Command> {
    let text = |arg: &OsString| arg.to_str().map(str::to_owned);
    match args.as_slice() {
        [cmd, file] if cmd == "header" => Some(Command::Header(file.into())),
        [cmd, file] if cmd == "headwords" => Some(Command::Headwords(file.into())),
        [cmd, file, headword] if cmd == "lookup" => {
            Some(Command::Lookup(file.into(), text(headword)?))
        }
        [cmd, file, path] if cmd == "extract" => {
            Some(Command::Extract(file.into(), text(path)?, None))
        }
        [cmd, file, path, output] if cmd == "extract" => Some(Command::Extract(
            file.into(),
            text(path)?,
            Some(output.into()),
        )),
        _ => None,
    }
}

fn is_mdd(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mdd"))
}

fn run(command: Command) -> Result<(), Failure> {
    let mut out = io::stdout().lock();
    match command {
        Command::Header(file) => {
            if is_mdd(&file) {
                print_header(&mut out, Mdd::open(&file)?.header())?;
            } else {
                print_header(&mut out, Mdx::open(&file)?.header())?;
            }
        }
        Command::Headwords(file) => {
            if is_mdd(&file) {
                for path in Mdd::open(&file)?.resource_paths() {
                    writeln!(out, "{path}")?;
                }
            } else {
                for headword in Mdx::open(&file)?.headwords() {
                    writeln!(out, "{headword}")?;
                }
            }
        }
        Command::Lookup(file, headword) => {
            let records = Mdx::open(&file)?.lookup(&headword)?;
            if records.is_empty() {
                return Err(Failure::NotFound(format!("no entry for {headword:?}")));
            }
            for (i, record) in records.iter().enumerate() {
                if i > 0 {
                    writeln!(out, "</>")?;
                }
                match record {
                    Record::Text(text) => writeln!(out, "{text}")?,
                    Record::Link(target) => writeln!(out, "@@@LINK={target}")?,
                }
            }
        }
        Command::Extract(file, path, output) => {
            let bytes = Mdd::open(&file)?
                .resource(&path)?
                .ok_or_else(|| Failure::NotFound(format!("no resource at {path:?}")))?;
            match output {
                Some(output) => std::fs::write(output, bytes)?,
                None => out.write_all(&bytes)?,
            }
        }
    }
    out.flush()?;
    Ok(())
}

fn print_header(out: &mut impl Write, header: &Header) -> io::Result<()> {
    for (name, value) in header.attributes() {
        writeln!(out, "{name}: {value}")?;
    }
    Ok(())
}
