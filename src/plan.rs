use crate::{
    cli::{Cli, Repo},
    error::{Error, Result},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

const ARTIFACTS: &[&str] = &["safetensors", "gguf", "bin", "pt", "pth", "ckpt", "onnx"];
const SHARD_INDEX_SUFFIXES: &[&str] = &[".safetensors.index.json", ".bin.index.json"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedFile {
    pub remote: String,
    pub size: u64,
    pub destination: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PatternMatcher {
    patterns: Vec<Pattern>,
}

#[derive(Debug, Clone)]
pub struct Pattern {
    glob: Option<globset::GlobMatcher>,
    regex: Option<regex::Regex>,
}

impl Pattern {
    pub fn parse(s: &str) -> Result<Self> {
        let glob = globset::GlobBuilder::new(s)
            .case_insensitive(true)
            .literal_separator(false)
            .build()
            .ok()
            .map(|g| g.compile_matcher());

        let regex = regex::RegexBuilder::new(s)
            .case_insensitive(true)
            .build()
            .ok();

        if glob.is_none() && regex.is_none() {
            return Err(Error::InvalidPattern(s.to_string()));
        }

        Ok(Self { glob, regex })
    }

    pub fn is_match(&self, path: &str) -> bool {
        if let Some(glob) = &self.glob
            && glob.is_match(path)
        {
            return true;
        }
        if let Some(regex) = &self.regex
            && regex.is_match(path)
        {
            return true;
        }
        false
    }
}

impl PatternMatcher {
    pub fn new(patterns: &[String]) -> Result<Self> {
        let compiled = patterns
            .iter()
            .map(|p| Pattern::parse(p))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { patterns: compiled })
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn is_match(&self, path: &str) -> bool {
        self.patterns.iter().any(|p| p.is_match(path))
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    if bytes >= TIB {
        format!("{:.2} TB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.2} GB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

pub fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

pub fn select_files(files: Vec<RemoteFile>, cli: &Cli) -> Result<Vec<RemoteFile>> {
    let inc = PatternMatcher::new(&cli.include)?;
    let exc = PatternMatcher::new(&cli.exclude)?;
    Ok(files
        .into_iter()
        .filter(|f| {
            let artifact = f
                .path
                .rsplit('.')
                .next()
                .is_some_and(|x| ARTIFACTS.contains(&x))
                || SHARD_INDEX_SUFFIXES
                    .iter()
                    .any(|suffix| f.path.ends_with(suffix));
            let selected =
                cli.all || !inc.is_empty() && inc.is_match(&f.path) || inc.is_empty() && artifact;
            selected && !exc.is_match(&f.path)
        })
        .collect())
}
pub fn build_plan(repo: &Repo, files: Vec<RemoteFile>, cli: &Cli) -> Result<Vec<PlannedFile>> {
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut out = Vec::new();
    for f in files {
        if !safe_path(&f.path) {
            return Err(Error::UnsafePath(f.path));
        }
        let rel = if cli.repo_dir {
            PathBuf::from(&repo.owner).join(&repo.name).join(&f.path)
        } else {
            PathBuf::from(Path::new(&f.path).file_name().unwrap())
        };
        if !cli.repo_dir {
            let key = rel.to_string_lossy().into_owned();
            if let Some(first) = seen.insert(key.clone(), f.path.clone()) {
                return Err(Error::Collision {
                    first,
                    second: f.path,
                    name: key,
                });
            }
        }
        out.push(PlannedFile {
            remote: f.path,
            size: f.size,
            destination: cli.output.join(rel),
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    fn cli(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }
    #[test]
    fn selection_and_exclusion() {
        let c = cli(&["hfd", "o/r", "-i", "x/**", "-e", "*.bin"]);
        let a = select_files(
            vec![
                RemoteFile {
                    path: "x/a.bin".into(),
                    size: 1,
                },
                RemoteFile {
                    path: "x/a.gguf".into(),
                    size: 1,
                },
            ],
            &c,
        )
        .unwrap();
        assert_eq!(a.len(), 1);
    }
    #[test]
    fn collision() {
        let c = cli(&["hfd", "o/r"]);
        assert!(
            build_plan(
                &c.repo.validate().unwrap(),
                vec![
                    RemoteFile {
                        path: "a/x.bin".into(),
                        size: 1
                    },
                    RemoteFile {
                        path: "b/x.bin".into(),
                        size: 1
                    }
                ],
                &c
            )
            .is_err()
        );
    }
    #[test]
    fn repo_layout() {
        let c = cli(&["hfd", "o/r", "--repo-dir", "-o", "out"]);
        let p = build_plan(
            &c.repo.validate().unwrap(),
            vec![RemoteFile {
                path: "a/x.bin".into(),
                size: 1,
            }],
            &c,
        )
        .unwrap();
        assert_eq!(p[0].destination, PathBuf::from("out/o/r/a/x.bin"));
    }
    #[test]
    fn rejects_traversal() {
        assert!(!safe_path("a/../x"));
    }
    #[test]
    fn includes_shard_indexes_by_default() {
        let c = cli(&["hfd", "o/r"]);
        let selected = select_files(
            vec![RemoteFile {
                path: "model.safetensors.index.json".into(),
                size: 1,
            }],
            &c,
        )
        .unwrap();
        assert_eq!(selected.len(), 1);
    }
    #[test]
    fn include_quant_patterns() {
        let files = vec![
            RemoteFile {
                path: "model-Q4_0.gguf".into(),
                size: 100,
            },
            RemoteFile {
                path: "model-Q4_K_M.gguf".into(),
                size: 200,
            },
            RemoteFile {
                path: "model-Q4_K_S.gguf".into(),
                size: 150,
            },
            RemoteFile {
                path: "model-Q8_0.gguf".into(),
                size: 400,
            },
            RemoteFile {
                path: "README.md".into(),
                size: 10,
            },
        ];

        // --include Q4 should match all Q4 variants
        let c_q4 = cli(&["hfd", "o/r", "-i", "Q4"]);
        let sel_q4 = select_files(files.clone(), &c_q4).unwrap();
        let paths_q4: Vec<_> = sel_q4.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths_q4,
            vec!["model-Q4_0.gguf", "model-Q4_K_M.gguf", "model-Q4_K_S.gguf"]
        );

        // --include Q4_K_M should only match that exact quant
        let c_q4km = cli(&["hfd", "o/r", "-i", "Q4_K_M"]);
        let sel_q4km = select_files(files.clone(), &c_q4km).unwrap();
        let paths_q4km: Vec<_> = sel_q4km.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths_q4km, vec!["model-Q4_K_M.gguf"]);

        // Case insensitivity: --include q4_k_m matches Q4_K_M
        let c_lower = cli(&["hfd", "o/r", "-i", "q4_k_m"]);
        let sel_lower = select_files(files.clone(), &c_lower).unwrap();
        let paths_lower: Vec<_> = sel_lower.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths_lower, vec!["model-Q4_K_M.gguf"]);

        // Regex pattern: Q4_K_[MS] matches Q4_K_M and Q4_K_S
        let c_regex = cli(&["hfd", "o/r", "-i", "Q4_K_[MS]"]);
        let sel_regex = select_files(files, &c_regex).unwrap();
        let paths_regex: Vec<_> = sel_regex.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths_regex, vec!["model-Q4_K_M.gguf", "model-Q4_K_S.gguf"]);
    }
    #[test]
    fn format_bytes_examples() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(10 * 1024 * 1024), "10.00 MB");
        assert_eq!(format_bytes(4 * 1024 * 1024 * 1024), "4.00 GB");
    }
}
