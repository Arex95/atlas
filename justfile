# One-liner commands for the repo. Every gate that CI runs also runs from
# `just check`, so a red pipeline can be reproduced before pushing rather
# than discovered afterwards. That claim was false until the image build
# was added: three pipelines went red on `main` for container problems no
# local gate looked at.

default:
    @just --list

# ─── build & run ──────────────────────────────────────────────────────────

# Build the workspace in debug mode.
build:
    cargo build --workspace

# Build the release binary (what the container packages).
build-release:
    cargo build --workspace --release --locked

# Run the atlas-server binary in debug mode.
run:
    cargo run --bin atlas-server

# ─── quality gates ────────────────────────────────────────────────────────

# Enforce formatting. Fails on any diff.
fmt:
    cargo fmt --all --check

# Reformat the tree in place. Never run from CI.
fmt-fix:
    cargo fmt --all

# Full clippy pass, warnings denied. Same command CI runs.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Test suite. `--locked` because CI passes it: without it a stale
# Cargo.lock — a dependency added and not committed — passes here and
# fails there, which is exactly what this gate exists to prevent.
test:
    cargo test --workspace --all-features --locked

# Documentation builds with no broken links.
#
# Not covered by clippy or by the test suite, which is how a link to a
# method renamed months ago survived in a doc comment: rustdoc resolved
# nothing, warned, and nobody ran it.
docs:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace

# Every workspace crate is named in all four of the Dockerfile's lists.
# Milliseconds, and it catches the one that otherwise fails deep inside a
# container build with an error pointing somewhere else entirely.
check-dockerfile:
    bash scripts/check-dockerfile-crates.sh

# Every gate at once, in the order they will fail in CI.
#
# `image` is last and is the slow one — about a minute on a source
# change. It is here because CI builds the image too, and a gate that
# omits what CI runs is a gate that lies.
check: fmt check-dockerfile lint docs build test image

# ─── agent connections ────────────────────────────────────────────────────

# Print ready-to-paste MCP config for a running atlas-server.
connect-agent *ARGS:
    bash scripts/mcp-connect.sh {{ARGS}}

# ─── container ────────────────────────────────────────────────────────────

# Build the release image locally.
image:
    docker build -t atlas-server:local .

# Bring the local compose stack up in the foreground.
up:
    docker compose up --build

# Tear the local compose stack down and drop the data volume.
down:
    docker compose down --volumes
