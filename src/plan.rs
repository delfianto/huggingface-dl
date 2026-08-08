use crate::{
    cli::{Cli, Repo},
    error::{Error, Result},
};
use globset::{Glob, GlobSetBuilder};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

const ARTIFACTS: &[&str] = &["safetensors", "gguf", "bin", "pt", "pth", "ckpt", "onnx"];
const SHARD_INDEX_SUFFIXES: &[&str] = &[".safetensors.index.json", ".bin.index.json"];
#[derive(Debug, Clone)]
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

pub fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
pub fn select_files(files: Vec<RemoteFile>, cli: &Cli) -> Result<Vec<RemoteFile>> {
    let mut inc = GlobSetBuilder::new();
    for x in &cli.include {
        inc.add(Glob::new(x)?);
    }
    let inc = inc.build()?;
    let mut exc = GlobSetBuilder::new();
    for x in &cli.exclude {
        exc.add(Glob::new(x)?);
    }
    let exc = exc.build()?;
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
            let selected = cli.all
                || !cli.include.is_empty() && inc.is_match(&f.path)
                || cli.include.is_empty() && artifact;
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
}
