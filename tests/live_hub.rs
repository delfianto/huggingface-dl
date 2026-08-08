//! Opt-in checks against real Hugging Face repositories; they use --dry-run only.
use std::{env, process::Command};

fn plan(repo: &str) -> Vec<serde_json::Value> {
    if env::var_os("HFD_LIVE_TESTS").is_none() {
        eprintln!("skipping live Hub test; set HFD_LIVE_TESTS=1 to enable it");
        return Vec::new();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_hfd"))
        .args([repo, "--dry-run", "--json", "--quiet"])
        .output()
        .expect("run hfd");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON plan")
}

fn paths(files: Vec<serde_json::Value>) -> Vec<String> {
    files
        .into_iter()
        .filter_map(|file| file["remote"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn flux_has_one_default_artifact() {
    let actual = paths(plan("black-forest-labs/FLUX.1-dev-FP8"));
    if actual.is_empty() {
        return;
    }
    assert_eq!(actual, ["flux1-dev-fp8.safetensors"]);
}

#[test]
fn qwen_gguf_repo_has_multiple_selectable_artifacts() {
    let actual = paths(plan(
        "HauhauCS/Qwen3.6-35B-A3B-Uncensored-HauhauCS-Aggressive",
    ));
    if actual.is_empty() {
        return;
    }
    assert!(actual.len() > 1);
    assert!(actual.iter().all(|path| path.ends_with(".gguf")));
}

#[test]
fn gemma_shards_include_the_index() {
    let actual = paths(plan("google/gemma-4-31B-it"));
    if actual.is_empty() {
        return;
    }
    assert!(
        actual
            .iter()
            .any(|path| path == "model-00001-of-00002.safetensors")
    );
    assert!(
        actual
            .iter()
            .any(|path| path == "model-00002-of-00002.safetensors")
    );
    assert!(
        actual
            .iter()
            .any(|path| path.ends_with(".safetensors.index.json"))
    );
}
