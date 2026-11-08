# Contributing

Small project, short rules: keep the service small, keep tests real, keep the
diff focused.

## Getting set up

```bash
git clone https://github.com/Rift7/shortener.git
cd shortener
cargo test          # 19 tests, no external services needed
make dev            # starts on 127.0.0.1:8080 with a generated API key
```

Rust stable is pinned by `rust-toolchain.toml`; `rustup` picks it up
automatically. `rustfmt` and `clippy` are required components.

## Before opening a PR

All four must pass locally — CI enforces them:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --locked
```

Then:

1. Branch from `main`: `git checkout -b fix/slug-collision`
2. Add a test that fails without your change.
3. Use a conventional commit subject: `fix: retry slug allocation on collision`.
4. Fill in the PR template — the "how was it verified" section is the point.

## Design constraints

These are deliberate; a PR that breaks one needs a convincing argument:

- **Single binary, single file of state.** No Postgres, Redis, or message
  broker. If a feature needs one, it belongs in a separate service.
- **No analytics.** The service does not record requests, IPs, or user agents.
- **No target fetching.** The server never makes outbound requests, which is
  what keeps it out of SSRF territory.
- **API changes are additive.** Existing clients must keep working; new
  behavior goes behind a new field or endpoint.

## Testing notes

Integration tests in `tests/api.rs` build the real router with a temp SQLite
file and drive it through `tower::ServiceExt::oneshot`, so they exercise
extractors, auth, and the database without a network listener. Unit tests cover
URL normalization, slug validation, and generation.

When adding an endpoint, add at least: a happy path, an auth failure, and an
invalid-input case.

## Reporting bugs and requesting features

Use the issue forms. For security issues, see [SECURITY.md](SECURITY.md) and
report privately.
