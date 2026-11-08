## What does this change?

<!-- One or two sentences. Link the issue if one exists: Closes #123 -->

## How was it verified?

<!-- Actual commands and output, not "tested locally". -->

```text
$ cargo test
...
```

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] New behavior is covered by a test
- [ ] `README.md` / `.env.example` updated if config or API changed
- [ ] No secrets, keys, or private URLs in the diff
