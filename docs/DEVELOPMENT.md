# Development

## Requirements

- **Rust**: Stable toolchain (install via [rustup.rs](https://rustup.rs)).

## Build and Run

```bash
# Debug build (faster compile)
cargo run

# Release build (optimized)
cargo run --release
```

## Testing

```bash
# Run all unit tests
cargo test

# Run specific test
cargo test test_name
```

## Scripts

Helper scripts are located in `scripts/`:

- `changelog.sh`: Helper for generating changelogs.

## Release Process

1. Automated CI ensures builds pass on PRs.
2. Releases are handled via GitHub Actions (`.github/workflows/release.yml`).
3. Ensure documentation under `docs/` is updated before tagging a release.
