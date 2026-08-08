bins := "hfd"
primary := "hfd"
bin_dir := env("HOME") / ".local/bin"
sys_dir := "/usr/local/bin"
links := ""

# Normal tests do not contact Hugging Face. Use --integration for live metadata
# discovery tests, which still use --dry-run and never download model bytes.
test_unit := "cargo test --bin hfd"
test_integration := "HFD_LIVE_TESTS=1 cargo test --test live_hub"

default:
    @just --list

build *flags:
    #!/usr/bin/env bash
    set -euo pipefail
    debug=0
    for f in {{ flags }}; do
        case "$f" in
            --debug) debug=1 ;;
            *) echo "build: unknown flag '$f' (want --debug or none)" >&2; exit 1 ;;
        esac
    done
    if [ "$debug" -eq 1 ]; then
        cargo build
        exit 0
    fi
    RUSTFLAGS="-C target-cpu=native" cargo build --release
    if ! command -v upx >/dev/null 2>&1; then
        echo "build: upx not found in PATH" >&2
        exit 1
    fi
    for b in {{ bins }}; do
        path="target/release/$b"
        if [ ! -f "$path" ]; then
            echo "build: missing $path (is bins= correct?)" >&2
            exit 1
        fi
        upx -t "$path" >/dev/null 2>&1 || upx --best --lzma "$path"
        echo "compressed $path"
    done

run *args:
    cargo run --release --bin {{ primary }} -- {{ args }}

test *flags:
    #!/usr/bin/env bash
    set -euo pipefail
    verbose=0
    integration=0
    run_all=0
    for f in {{ flags }}; do
        case "$f" in
            --verbose) verbose=1 ;;
            --integration) integration=1 ;;
            --run-all) run_all=1 ;;
            *) echo "test: unknown flag '$f' (want --verbose, --integration, --run-all)" >&2; exit 1 ;;
        esac
    done
    if [ "$integration" -eq 1 ] && [ "$run_all" -eq 1 ]; then
        echo "test: use either --integration or --run-all, not both" >&2
        exit 1
    fi
    add_nocapture() {
        local cmd="$1"
        if [ "$verbose" -eq 0 ]; then printf '%s\n' "$cmd"; return; fi
        case "$cmd" in *" -- "*) printf '%s --nocapture\n' "$cmd" ;; *) printf '%s -- --nocapture\n' "$cmd" ;; esac
    }
    run_shell() { local cmd="$1"; echo "+ $cmd"; eval "$cmd"; }
    unit_cmd='{{ test_unit }}'
    integ_cmd='{{ test_integration }}'
    if [ "$integration" -eq 0 ]; then run_shell "$(add_nocapture "$unit_cmd")"; fi
    if [ "$integration" -eq 1 ] || [ "$run_all" -eq 1 ]; then
        if [ -z "$integ_cmd" ]; then echo "test: --integration/--run-all need test_integration knob set in justfile" >&2; exit 1; fi
        run_shell "$(add_nocapture "$integ_cmd")"
    fi

format *flags:
    #!/usr/bin/env bash
    set -euo pipefail
    mode=""
    for f in {{ flags }}; do
        case "$f" in
            --apply) [ -n "$mode" ] && { echo "format: pass only one of --apply / --check-only" >&2; exit 1; }; mode=apply ;;
            --check-only) [ -n "$mode" ] && { echo "format: pass only one of --apply / --check-only" >&2; exit 1; }; mode=check ;;
            *) echo "format: unknown flag '$f' (want --apply or --check-only)" >&2; exit 1 ;;
        esac
    done
    mode="${mode:-apply}"
    if [ "$mode" = apply ]; then cargo fmt --all; else cargo fmt --all -- --check; fi

lint:
    cargo clippy --all-targets --all-features -- -D warnings

full-gate:
    just format --check-only
    just lint
    just test

install *flags: build
    #!/usr/bin/env bash
    set -euo pipefail
    dir="{{ bin_dir }}"
    sudo=""
    for f in {{ flags }}; do
        case "$f" in --system) dir="{{ sys_dir }}"; sudo="sudo" ;; *) echo "install: unknown flag '$f' (only --system is supported)" >&2; exit 1 ;; esac
    done
    for b in {{ bins }}; do $sudo install -Dm755 "target/release/$b" "$dir/$b"; echo "installed $dir/$b"; done
    for pair in {{ links }}; do
        [ -z "$pair" ] && continue
        link="${pair%%:*}"; target="${pair#*:}"
        if [ -z "$link" ] || [ -z "$target" ] || [ "$link" = "$pair" ]; then echo "install: bad links entry '$pair' (want link:target)" >&2; exit 1; fi
        $sudo ln -sf "$target" "$dir/$link"; echo "linked $dir/$link -> $target"
    done

uninstall *flags:
    #!/usr/bin/env bash
    set -euo pipefail
    dir="{{ bin_dir }}"
    sudo=""
    for f in {{ flags }}; do
        case "$f" in --system) dir="{{ sys_dir }}"; sudo="sudo" ;; *) echo "uninstall: unknown flag '$f' (only --system is supported)" >&2; exit 1 ;; esac
    done
    for b in {{ bins }}; do $sudo rm -f "$dir/$b"; echo "removed $dir/$b"; done
    for pair in {{ links }}; do
        [ -z "$pair" ] && continue
        link="${pair%%:*}"; [ -z "$link" ] || [ "$link" = "$pair" ] && continue
        $sudo rm -f "$dir/$link"; echo "removed $dir/$link"
    done

clean:
    cargo clean
