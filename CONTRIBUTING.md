# Contributing to Atlas

Thanks for your interest in contributing to Atlas.

## Getting Started

1. Fork the repository and clone your fork.
2. Follow the setup instructions in [README.md](README.md).
3. Create a branch from `main`: `git checkout -b feat/your-feature`.

## Development

```bash
# First-time setup
cp .env.example .env
cp apps/web/.env.example apps/web/.env
pnpm install

# Dev mode (run both in parallel)
cargo run -p atlas-server   # backend on :4000
pnpm --filter web dev       # frontend on :3000
```

## Before Submitting

Both pipelines must be warning-free:

```bash
# Backend
RUSTFLAGS="-D warnings" cargo build -p atlas-server
cargo clippy -p atlas-server -- -D warnings

# Frontend
pnpm --filter web build
```

If you add a new SQL migration, rebuild the server binary before running so sqlx embeds it:

```bash
cargo build --release -p atlas-server
```

## Commit Style

Conventional commits:

```
feat(server): add X
fix(web): correct Y
chore: update dependencies
docs: clarify Z
```

Keep the subject line under 72 characters. A body explaining *why* is encouraged.

## Pull Requests

- Target `main`.
- One logical change per PR.
- Include a short description of what changed and why.
- Make sure CI passes (zero warnings on both Rust and TypeScript).

## Code Style

- **Rust** — follow `rustfmt` defaults, no `#[allow(unused)]` without a comment explaining why.
- **Vue/TypeScript** — no hardcoded colors or sizes; use the design tokens from `apps/web/src/assets/main.css`.
- **No new comments** that only restate what the code does — only comments that explain a non-obvious *why*.

## Reporting Issues

Open a GitHub issue with:
- Atlas version / commit hash
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs (server output, browser console)

## License

By contributing you agree that your contributions will be licensed under the [MIT License](LICENSE).
