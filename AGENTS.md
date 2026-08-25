# hfd contributor guide

## Scope

`hfd` is a small, flat-file Hugging Face model downloader. Keep it focused:
do not add cache/blob storage, symlinks, manifests, a server, a TUI, or proxy
management unless the requested task explicitly expands that scope.

The untracked `HuggingFaceModelDownloader/` directory is a Go reference only;
do not edit it as part of work on `hfd`.

## Layout

- `src/cli.rs`: command-line arguments and repository validation.
- `src/config.rs`: TOML config discovery and precedence.
- `src/hub.rs`: Hugging Face tree discovery and URL/status handling.
- `src/plan.rs`: file selection, remote-path safety, output layout.
- `src/transfer.rs`: concurrent streaming, resume, and atomic completion.
- `src/tui.rs`: interactive file selection prompt.
- `tests/live_hub.rs`: opt-in live discovery tests; never download model data.

## Behavior contracts

- Default output is flat. Detect basename collisions before any transfer and
  direct users to `--repo-dir`.
- Treat all server-provided remote paths as untrusted. Preserve remote
  directories only under `--repo-dir` after validating paths.
- A completed matching-size destination is skipped. Interrupted data remains
  as a local `.part` file and resumes only after HTTP range validation.
- Default selection includes model artifact extensions and shard index JSON
  companions. Excludes always override includes.
- Configuration precedence is CLI, then `HF_TOKEN`, then TOML defaults.
  Missing implicit config is normal; a requested config must parse correctly.

## Verification

Run these before handing off a change:

```sh
just format --check-only
just lint
just test
just build
```

`just test --integration` contacts public Hugging Face metadata endpoints and is opt-in.
It must use `--dry-run`; do not make it part of the normal CI gate.

## Style

Use `cargo fmt`; CI treats all Clippy warnings as errors. Add focused unit
tests for parser, selection, path, or transfer changes. Never put credentials
or real tokens in test fixtures, documentation, logs, or commits.
