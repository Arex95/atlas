#!/usr/bin/env bash
#
# Every workspace crate must appear in all four of the Dockerfile's
# lists, and this says which one is missing.
#
# The dependency-cache stage names each crate four times — the manifest
# COPY, the stub `src/` directory, the stub `lib.rs`, and the `rm -rf`
# of its compiled artefacts. Miss the fourth and the build does not
# fail: cargo reuses the stub's rlib and the error surfaces much later
# as "could not find `api` in `atlas_<name>`", pointing nowhere near
# the Dockerfile.
#
# A full `docker build` catches that too, in about a minute. This
# catches it in milliseconds and names the line to add, which is the
# difference between a gate people run and one they skip.
#
# It does not replace the image build: a crate list can be complete
# while the image is still broken for some other reason — a missing
# runtime package, say. Both are in `just check`.

set -euo pipefail

cd "$(dirname "$0")/.."

failed=0

note() {
    printf '  %s\n' "$1"
}

for manifest in features/*/Cargo.toml bin/*/Cargo.toml; do
    dir="$(dirname "$manifest")"
    # atlas-graph -> atlas_graph, the name cargo gives its artefacts.
    #
    # awk with an exit rather than `sed | head -1`. Under `pipefail`,
    # `head` closing the pipe after the first line leaves `sed` writing
    # into it, and the SIGPIPE that follows becomes the pipeline's exit
    # status — 141, which `set -e` turns into a dead script. Whether it
    # happens at all depends on which process reaches the end first, so
    # it passes locally on small manifests and fails on a CI runner.
    # This has no pipe to break.
    package="$(awk -F'"' '/^name = /{print $2; exit}' "$manifest")"
    underscored="${package//-/_}"

    if ! grep -qF "COPY $manifest" Dockerfile; then
        note "Dockerfile does not COPY $manifest"
        failed=1
    fi
    if ! grep -qF "$dir/src" Dockerfile; then
        note "Dockerfile creates no stub src/ for $dir"
        failed=1
    fi
    if ! grep -qF "$dir/src/lib.rs" Dockerfile && ! grep -qF "$dir/src/main.rs" Dockerfile; then
        note "Dockerfile writes no stub entry point for $dir"
        failed=1
    fi
    # The two that fail silently, and they are two because the globs do
    # not overlap: a library's rlib is `libatlas_<name>-<hash>.rlib`,
    # which `atlas_<name>*` does not match. Checking only one of them
    # is how this script passed a Dockerfile that could not build —
    # `atlas_notes*` was present, `libatlas_notes*` was not, and the
    # final stage reused the stub rlib.
    if ! grep -qF "deps/${underscored}*" Dockerfile; then
        note "Dockerfile never removes deps/${underscored}* — the stub artefacts will be reused"
        failed=1
    fi
    # Only libraries produce an rlib; a binary crate has no lib form.
    if grep -qF "$dir/src/lib.rs" Dockerfile \
        && ! grep -qF "deps/lib${underscored}*" Dockerfile; then
        note "Dockerfile never removes deps/lib${underscored}* — the stub rlib will be reused and the build will fail with \"unresolved import \\\`${underscored}::api\\\`\""
        failed=1
    fi
done

if [ "$failed" -ne 0 ]; then
    echo
    echo "The Dockerfile's dependency-cache stage is out of date." >&2
    echo "See the comment at the top of the Dockerfile: four lists, all of them." >&2
    exit 1
fi

echo "Dockerfile lists every workspace crate."
