mod cli;
mod config;
mod error;
mod hub;
mod plan;
mod transfer;
mod tui;

use clap::Parser;
use cli::Cli;
use error::Result;
use hub::Hub;
use plan::{build_plan, select_files};
use std::io::IsTerminal;
use tui::prompt_selection;

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("hfd: {error}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let config = config::load(cli.config.as_ref())?;
    config.validate()?;
    let mut cli = cli;
    if cli.jobs.is_none() {
        cli.jobs = config.max_download_jobs.or(Some(4));
    }
    if cli.token.is_none() {
        cli.token = config.huggingface_token;
    }
    let repo = cli.repo.validate()?;
    let hub = Hub::new(&cli)?;
    let files = hub.tree(&repo, &cli.revision).await?;
    let mut selected = select_files(files, &cli)?;

    let should_prompt = (cli.interactive
        || (cli.include.is_empty() && !cli.yes && selected.len() > 1))
        && !cli.dry_run
        && !cli.json
        && !cli.quiet
        && !cli.yes
        && std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal();

    if should_prompt {
        match prompt_selection(&repo, selected)? {
            Some(chosen) if chosen.is_empty() => {
                if !cli.quiet {
                    println!("No files selected.");
                }
                return Ok(());
            }
            Some(chosen) => {
                selected = chosen;
            }
            None => {
                return Ok(());
            }
        }
    }

    let plan = build_plan(&repo, selected, &cli)?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else if !cli.quiet {
        for item in &plan {
            println!("{} -> {}", item.remote, item.destination.display());
        }
    }
    if cli.dry_run {
        return Ok(());
    }
    transfer::download_all(
        hub,
        &repo,
        &cli.revision,
        plan,
        cli.jobs.unwrap_or(4),
        config.max_parallel_parts_per_job.unwrap_or(1),
        cli.quiet,
    )
    .await
}
