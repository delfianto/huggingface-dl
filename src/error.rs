use std::io;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid repository `{0}`; expected OWNER/REPO")]
    InvalidRepo(String),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("unsafe remote path `{0}`")]
    UnsafePath(String),
    #[error("flat output collision: `{first}` and `{second}` both map to `{name}`; use --repo-dir")]
    Collision {
        first: String,
        second: String,
        name: String,
    },
    #[error(
        "Hub returned 401/403 for `{0}`; pass --token or set HF_TOKEN (the repository may be gated)"
    )]
    Unauthorized(String),
    #[error("Hub repository or revision was not found: `{0}`")]
    NotFound(String),
    #[error("Hub rate limited this request; retry later")]
    RateLimited,
    #[error("Hub request failed ({status}): {message}")]
    Http {
        status: reqwest::StatusCode,
        message: String,
    },
    #[error("server did not honor a valid resume range for `{0}`")]
    InvalidRange(String),
    #[error("download size mismatch for `{path}`: expected {expected}, got {actual}")]
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    HttpClient(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("could not read configuration `{path}`: {source}")]
    ConfigIo { path: String, source: io::Error },
    #[error("could not parse configuration `{path}`: {source}")]
    ConfigToml {
        path: String,
        source: toml::de::Error,
    },
    #[error(transparent)]
    Glob(#[from] globset::Error),
}
