use std::env;
use std::path::PathBuf;
use std::process;

use agentdeck_lib::autonomy::{
    run_guarded, run_overnight, write_report, PolicyVerdict,
};
use agentdeck_lib::chatgpt_review;

fn main() {
    if let Err(code) = run() {
        eprintln!("hermes: {code}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(usage());
    }

    match args.remove(0).as_str() {
        "guard" => run_guard_command(&args),
        "overnight" => run_overnight_command(&args),
        "review" => run_review_command(),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: hermes guard [--execute|--dry-run] <command>\n       hermes overnight [--queue <path>] [--execute-verify]\n       hermes review".to_owned()
}

fn run_review_command() -> Result<(), String> {
    let health = chatgpt_review::evaluate_default_review_health()?;
    println!("{}", serde_json::to_string_pretty(&health).map_err(|error| error.to_string())?);
    if health.ready_for_reviewers {
        Ok(())
    } else {
        Err("ChatGPT review readiness checks failed".to_owned())
    }
}

fn run_guard_command(args: &[String]) -> Result<(), String> {
    let mut execute = false;
    let mut command_parts = Vec::new();

    for arg in args {
        match arg.as_str() {
            "--execute" => execute = true,
            "--dry-run" => execute = false,
            _ => command_parts.push(arg.clone()),
        }
    }

    if command_parts.is_empty() {
        return Err("guard requires a command".to_owned());
    }

    let command = command_parts.join(" ");
    let outcome = run_guarded(&command, execute);
    println!("{}", serde_json::to_string_pretty(&outcome).map_err(|error| error.to_string())?);

    match outcome.verdict {
        PolicyVerdict::Allow => Ok(()),
        PolicyVerdict::AskFirst { reason } => Err(format!("ASK_FIRST: {reason}")),
        PolicyVerdict::Deny { reason } => Err(format!("DENY: {reason}")),
    }
}

fn run_overnight_command(args: &[String]) -> Result<(), String> {
    let mut queue = PathBuf::from("tasks/overnight.queue.json");
    let mut execute_verify = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--queue" => {
                index += 1;
                queue = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--queue requires a path".to_owned())?,
                );
            }
            "--execute-verify" => execute_verify = true,
            other => return Err(format!("unknown overnight argument: {other}")),
        }
        index += 1;
    }

    let repo_root = env::current_dir().map_err(|error| error.to_string())?;
    let report = run_overnight(&repo_root, &queue, execute_verify);
    let report_path = write_report(&repo_root, &report)?;
    println!("report: {}", report_path.display());
    println!("{}", serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?);
    Ok(())
}