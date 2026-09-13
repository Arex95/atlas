# Multi-stage build: compile in a full toolchain, run on a minimal image.
# The runner builds and publishes; the target only pulls. Compiling on a
# machine that serves traffic competes with what is running on it.

# ─── build ────────────────────────────────────────────────────────────────
FROM rust:1.88-slim-bookworm AS build

WORKDIR /src

# Cache dependencies: copy the workspace + every member's manifest,
# stub every crate with an empty source tree, prebuild, then swap
# in the real sources. On unchanged deps the whole cargo download +
# compile step is served from Docker's layer cache.
#
# **When you add a new crate under bin/ or features/**, extend FOUR
# lists below, not three:
#   1. a COPY line for its Cargo.toml
#   2. the mkdir + echo stub list
#   3. the `rm -rf` of its src directory
#   4. the `rm -rf` of its compiled artefacts — both `atlas_<name>*`
#      and `libatlas_<name>*` under target/release/deps
# Missing (4) is the quiet one: the sources get replaced correctly,
# cargo reuses the stub's rlib anyway, and the build fails with
# "could not find `api` in `atlas_<name>`" that points nowhere near
# this file.
COPY Cargo.toml Cargo.lock ./
COPY bin/atlas-server/Cargo.toml       bin/atlas-server/Cargo.toml
COPY bin/atlas/Cargo.toml              bin/atlas/Cargo.toml
COPY features/tracker/Cargo.toml       features/tracker/Cargo.toml
COPY features/mcp/Cargo.toml           features/mcp/Cargo.toml
COPY features/messaging/Cargo.toml     features/messaging/Cargo.toml
COPY features/sessions/Cargo.toml      features/sessions/Cargo.toml
COPY features/terminal/Cargo.toml      features/terminal/Cargo.toml
COPY features/afg/Cargo.toml           features/afg/Cargo.toml
COPY features/auth/Cargo.toml          features/auth/Cargo.toml
COPY features/sync/Cargo.toml          features/sync/Cargo.toml
COPY features/memory/Cargo.toml        features/memory/Cargo.toml
COPY features/notes/Cargo.toml         features/notes/Cargo.toml
COPY features/graph/Cargo.toml         features/graph/Cargo.toml
RUN mkdir -p bin/atlas-server/src bin/atlas/src features/tracker/src features/mcp/src features/messaging/src features/sessions/src features/terminal/src features/afg/src features/auth/src features/sync/src features/memory/src features/notes/src features/graph/src \
    && echo "fn main() {}"          > bin/atlas-server/src/main.rs \
    && echo "fn main() {}"          > bin/atlas/src/main.rs \
    && echo "// stub"               > features/tracker/src/lib.rs \
    && echo "// stub"               > features/mcp/src/lib.rs \
    && echo "// stub"               > features/messaging/src/lib.rs \
    && echo "// stub"               > features/sessions/src/lib.rs \
    && echo "// stub"               > features/terminal/src/lib.rs \
    && echo "// stub"               > features/afg/src/lib.rs \
    && echo "// stub"               > features/auth/src/lib.rs \
    && echo "// stub"               > features/sync/src/lib.rs \
    && echo "// stub"               > features/memory/src/lib.rs \
    && echo "// stub"               > features/notes/src/lib.rs \
    && echo "// stub"               > features/graph/src/lib.rs \
    && cargo build --release --locked --bin atlas-server --bin atlas \
    && rm -rf bin/atlas-server/src bin/atlas/src features/tracker/src features/mcp/src features/messaging/src features/sessions/src features/terminal/src features/afg/src features/auth/src features/sync/src features/memory/src features/notes/src features/graph/src \
              target/release/atlas-server \
              target/release/atlas \
              target/release/deps/atlas_cli* \
              target/release/deps/atlas_server* \
              target/release/deps/atlas_tracker* \
              target/release/deps/atlas_mcp* \
              target/release/deps/atlas_messaging* \
              target/release/deps/atlas_sessions* \
              target/release/deps/atlas_terminal* \
              target/release/deps/atlas_afg* \
              target/release/deps/atlas_auth* \
              target/release/deps/atlas_sync* \
              target/release/deps/atlas_memory* \
              target/release/deps/atlas_notes* \
              target/release/deps/atlas_graph* \
              target/release/deps/libatlas_tracker* \
              target/release/deps/libatlas_mcp* \
              target/release/deps/libatlas_messaging* \
              target/release/deps/libatlas_sessions* \
              target/release/deps/libatlas_terminal* \
              target/release/deps/libatlas_afg* \
              target/release/deps/libatlas_auth* \
              target/release/deps/libatlas_sync* \
              target/release/deps/libatlas_memory* \
              target/release/deps/libatlas_notes* \
              target/release/deps/libatlas_graph*

COPY . .
# Both binaries: the server, and the `atlas` CLI that gets put on a
# spawned terminal's PATH so an agent working in a session can call
# Atlas back as that session.
RUN cargo build --release --locked --bin atlas-server --bin atlas

# ─── runtime ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Non-root user. A container running as root defeats every namespace boundary
# the runtime provides.
RUN groupadd --system --gid 10001 atlas \
    && useradd --system --uid 10001 --gid atlas --home-dir /var/lib/atlas atlas \
    && mkdir -p /var/lib/atlas \
    && chown atlas:atlas /var/lib/atlas

# CA certs so outbound HTTPS to trackers works out of the box.
#
# git is here for `graph.changes`, which reads a mounted repository's
# working tree by running git rather than linking a library — the
# answer then matches what the developer sees in their own terminal.
# Without it that one tool fails; everything else works regardless.
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/target/release/atlas-server /usr/local/bin/atlas-server
COPY --from=build /src/target/release/atlas /usr/local/bin/atlas

USER atlas
WORKDIR /var/lib/atlas

# Config comes from the environment, so one artefact runs anywhere. The binary
# reads any `ATLAS_*` variables it needs.
ENV RUST_LOG=info

EXPOSE 4000

ENTRYPOINT ["/usr/local/bin/atlas-server"]
