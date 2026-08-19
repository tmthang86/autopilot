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

fn main() -> ExitCode {
    let mut project: Option<PathBuf> = None;
    let mut subcommand: Option<String> = None;
    let mut interval: u64 = 2100;

    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--project" => match args.next() {
                Some(p) => project = Some(PathBuf::from(p)),
                None => return usage(),
            },
            "--interval" => match args.next().and_then(|n| n.parse().ok()) {
                Some(n) => interval = n,
                None => return usage(),
            },
            s if SUBCOMMANDS.contains(&s) => subcommand = Some(s.to_string()),
            other => {
                eprintln!("unknown argument: {other}");
                return usage();
            }
        }
    }

    let (Some(sub), Some(project)) = (subcommand, project) else {
        return usage();
    };
    let code = match sub.as_str() {
        "run-once" => autopilot::run_once::run(&project),
        "status" => ExitCode::from(autopilot::ctl::status(&project) as u8),
        "start" => ExitCode::from(autopilot::ctl::start(&project) as u8),
        "stop" => ExitCode::from(autopilot::ctl::stop(&project) as u8),
        "install" => ExitCode::from(autopilot::install::run(&project, interval) as u8),
        "preflight" => {
            let report = autopilot::preflight::run(&project);
            autopilot::preflight::print(&report);
            ExitCode::SUCCESS
        }
        _ => usage(),
    };
    code
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: autopilot <{}> --project <path> [--interval <seconds>]",
        SUBCOMMANDS.join("|")
    );
    ExitCode::from(1)
}
