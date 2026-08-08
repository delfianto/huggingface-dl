# hfd

`hfd` is a focused Rust downloader for Hugging Face model artifacts. It writes
files directly to a directory; it does not create a Hugging Face cache, blobs,
symlinks, manifests, or persistent metadata.

## Install

```sh
cargo install --path .
# or: just install
```

## Usage

```sh
hfd black-forest-labs/FLUX.1-dev-FP8
# writes ./flux1-dev-fp8.safetensors
```

By default hfd selects `.safetensors`, `.gguf`, `.bin`, `.pt`, `.pth`, `.ckpt`,
and `.onnx` files, plus shard index companions such as
`.safetensors.index.json`, and writes their basenames into the output directory.

```sh
hfd org/model -o models -r main -j 8
hfd org/model --all --repo-dir -o models
hfd org/model -i 'weights/**' -e '*.json'
hfd org/private-model -t "$HF_TOKEN"
```

`--all` selects every file. `--include` and `--exclude` match remote relative
paths (excludes win). Flat output refuses basename collisions; use `--repo-dir`
to write to `<output>/<owner>/<repo>/` while preserving remote directories.

Authentication uses `--token` or the `HF_TOKEN` environment variable. Use
`--endpoint` for a compatible Hub endpoint. Downloads resume from local `.part`
files when the server validates byte ranges, and completed same-size files are
skipped.

## Configuration

By default hfd reads `$XDG_CONFIG_HOME/hfd.toml`, falling back to
`~/.config/hfd.toml` on Linux and macOS. A missing default file is fine. Pass
`--config /path/to/hfd.toml` to use another file.

```toml
# Defaults for downloads that do not pass -j.
max_download_jobs = 4

# Concurrent HTTP range requests per model file. Default: 1.
max_parallel_parts_per_job = 1

# Used when neither --token nor HF_TOKEN is provided.
huggingface_token = "hf_..."
```

Run `hfd --help` for the complete command reference. Development checks:

```sh
just full-gate
```

Optional live discovery tests use public Hub metadata only—never model downloads:

```sh
just test --integration
```
