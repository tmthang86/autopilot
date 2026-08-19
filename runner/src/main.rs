use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const SUBCOMMANDS: &[&str] = &[
    "run-once",
    "status",
    "start",
    "stop",
    "install",
    "preflight",
];

fn usage() -> ExitCode {
    eprintln!(
        "usage: autopilot <{}> --project <path>",
        SUBCOMMANDS.join("|")
    );
    ExitCode::from(1)
}

fn main() -> ExitCode {
    let mut project: Option<PathBuf> = None;
    let mut subcommand: Option<String> = None;

    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--project" => match args.next() {
                Some(p) => project = Some(PathBuf::from(p)),
                None => return usage(),
            },
            s if SUBCOMMANDS.contains(&s) => subcommand = Some(s.to_string()),
            other => {
                eprintln!("unknown argument: {other}");
                return usage();
            }
        }
    }

    match (subcommand.as_deref(), project) {
        (Some("run-once"), Some(_)) => ExitCode::SUCCESS,
        _ => usage(),
    }
}
