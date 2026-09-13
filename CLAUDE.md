# CLAUDE.md

What an agent working in this repository needs to know that the code
does not already say. Everything here is specific to this project — a
general rule about writing Rust or using git does not belong in it.

## Documentation ships with the change

Not as a follow-up commit. In the **same commit**:

| Touched | Update |
|---|---|
| a route in a `router.rs` | `docs/reference/http-api.md` |
| a tool schema under `domain/tools/` | `docs/reference/mcp-tools.md` |
| an `ENV_*` constant | `docs/reference/environment.md` **and** `.env.example` |
| a user-facing capability | a page under `docs/guide/` |
| something that would surprise a reader | a page under `docs/concepts/` |

Check before committing: if a `router.rs` or a file under
`domain/tools/` changed and `docs/` did not, something is missing.

Deferred documentation cannot be audited, which is the whole reason
this is a rule rather than a preference.

## Where to look first

- [`features/README.md`](./features/README.md) — the shape every
  feature crate follows, the direction its layers run, and how each of
  those rules is checked. Read it before adding a crate or moving a
  file.
- [`docs/concepts/why-atlas.md`](./docs/concepts/why-atlas.md) — what
  this is for, and what it deliberately refuses to do. The refusals
  are load-bearing; a change that quietly reverses one needs to say so.
- [`docs/concepts/trust-model.md`](./docs/concepts/trust-model.md) —
  where the boundary actually is. Do not describe anything in this
  repository as containment without reading it first.

## Facts about the toolchain

- **Rust `1.88`+**, edition `2024`, workspace resolver `3`. The MSRV
  is what it is to allow let-chains, which several dependencies assume.
- **Zero warnings.** `#[deny(unsafe_code)]` and clippy denied globally
  through `[workspace.lints]`. A local exception uses `#[allow(...)]`
  on the smallest scope that works, with a comment saying why.
- **One binary composes the crates.** `bin/atlas-server` wires them
  together and holds no business logic.
- **`just check` reproduces every CI gate**, image build included.

## Traps specific to this repository

**Do not add `sqlx` or any database crate to the workspace root.** It
belongs in the feature crate that owns the data. A shared client at the
root re-introduces two modules writing the same table, one import
removed.

**Migrations live inside the feature that owns the schema.** A
root-level `migrations/` directory would break that ownership
silently. Version them with a UTC timestamp — every crate migrates the
same SQLite file, so two crates must never produce the same version.

**MSRV is `1.88`, and your local `rustc` is almost certainly newer.**
`cargo build` will accept language features stabilised after it, and
newer clippy ships a *softer* lint set than 1.88 does — `similar_names`
fires there on cases 1.93 ignores. Run both before pushing:

```sh
rustup toolchain install 1.88 --profile minimal
rustup component add --toolchain 1.88 clippy rustfmt
cargo +1.88 clippy --workspace --all-targets --all-features -- -D warnings
```

`cargo +1.88 check` alone is not enough: CI runs clippy, and clippy has
caught what `check` did not.

**Verify against something running, not only against tests.** A green
suite says the assertions hold. It does not say the endpoint answers,
the terminal spawns, or the gate fires. Start the server and use it.
