# Development

## Requirements

- Rust stable toolchain.

## Build and Run

```bash
cargo run
```

## Validate

```bash
cargo check
cargo test
```

## Release and CI

- CI workflow: `.github/workflows/ci.yml`
- Release workflow: `.github/workflows/release.yml`
- Changelog helper: `scripts/changelog.sh`

## Contributing Notes

- Prefer focused changes with `cargo check` before opening PRs.
- Keep docs under `/docs` updated when features or behavior changes.
