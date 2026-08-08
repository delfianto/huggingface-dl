mod cli;
mod config;
mod error;
mod hub;
mod plan;
mod transfer;

use clap::Parser;
use cli::Cli;
use error::Result;
use hub::Hub;
use plan::{build_plan, select_files};

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
    let selected = select_files(files, &cli)?;
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
