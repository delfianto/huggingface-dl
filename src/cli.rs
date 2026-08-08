use crate::error::{Error, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "hfd",
    version,
    about = "Download Hugging Face model files directly"
)]
pub struct Cli {
    /// Repository in OWNER/REPO form
    pub repo: RepoArg,
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,
    /// Preserve remote paths in OUTPUT/OWNER/REPO
    #[arg(long)]
    pub repo_dir: bool,
    #[arg(short = 'r', long, default_value = "main")]
    pub revision: String,
    #[arg(short = 'i', long)]
    pub include: Vec<String>,
    #[arg(short = 'e', long)]
    pub exclude: Vec<String>,
    #[arg(long)]
    pub all: bool,
    #[arg(short = 't', long, env = "HF_TOKEN")]
    pub token: Option<String>,
    #[arg(long, default_value = "https://huggingface.co")]
    pub endpoint: String,
    #[arg(short = 'j', long, value_parser = parse_jobs)]
    pub jobs: Option<usize>,
    /// Read settings from this TOML file instead of the default config path
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub quiet: bool,
}

#[derive(Debug, Clone)]
pub struct RepoArg(pub String);
impl std::str::FromStr for RepoArg {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(s.into()))
    }
}
#[derive(Debug, Clone)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}
impl RepoArg {
    pub fn validate(&self) -> Result<Repo> {
        let Some((owner, name)) = self.0.split_once('/') else {
            return Err(Error::InvalidRepo(self.0.clone()));
        };
        if owner.is_empty()
            || name.is_empty()
            || name.contains('/')
            || owner.contains("..")
            || name.contains("..")
        {
            return Err(Error::InvalidRepo(self.0.clone()));
        }
        Ok(Repo {
            owner: owner.into(),
            name: name.into(),
        })
    }
}
impl Repo {
    pub fn full(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

fn parse_jobs(value: &str) -> std::result::Result<usize, String> {
    match value.parse() {
        Ok(0) | Err(_) => Err("must be a positive integer".into()),
        Ok(value) => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults() {
        let c = Cli::try_parse_from(["hfd", "a/b"]).unwrap();
        assert_eq!(c.revision, "main");
        assert_eq!(c.jobs, None);
        assert_eq!(c.output, PathBuf::from("."));
    }
    #[test]
    fn rejects_bad_repo() {
        assert!(RepoArg("a/b/c".into()).validate().is_err());
    }
}
