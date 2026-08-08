use crate::error::{Error, Result};
use serde::Deserialize;
use std::{env, path::PathBuf};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub max_download_jobs: Option<usize>,
    pub max_parallel_parts_per_job: Option<usize>,
    pub huggingface_token: Option<String>,
}

pub fn default_path() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|dir| dir.join("hfd.toml"))
}

pub fn load(explicit: Option<&PathBuf>) -> Result<Config> {
    let path = explicit.cloned().or_else(default_path);
    let Some(path) = path else {
        return Ok(Config::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|source| Error::ConfigToml {
            path: path.display().to_string(),
            source,
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound && explicit.is_none() => {
            Ok(Config::default())
        }
        Err(source) => Err(Error::ConfigIo {
            path: path.display().to_string(),
            source,
        }),
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.max_download_jobs == Some(0) || self.max_parallel_parts_per_job == Some(0) {
            return Err(Error::InvalidConfig("job counts must be positive".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_values() {
        let c: Config = toml::from_str(
            "max_download_jobs = 3\nmax_parallel_parts_per_job = 2\nhuggingface_token = 'x'",
        )
        .unwrap();
        assert_eq!(c.max_download_jobs, Some(3));
        assert_eq!(c.max_parallel_parts_per_job, Some(2));
        assert_eq!(c.huggingface_token.as_deref(), Some("x"));
    }
}
