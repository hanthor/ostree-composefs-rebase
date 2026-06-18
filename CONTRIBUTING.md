# Contributing

Thanks for your interest in improving `bootc-migrate-composefs`.

> **Note:** this tool performs an in-place, hard-to-reverse migration of a real
> system. Treat changes to the migration phases (`src/migration/`) with care and
> exercise them through the end-to-end suite before merging.

## Development setup

You need a recent stable Rust toolchain (the crate targets edition 2024,
`rust-version = 1.88.0`) and [`just`](https://github.com/casey/just).

```console
$ cargo build
$ just check        # clippy + rustfmt + unit tests + shellcheck — run before every PR
```

The end-to-end tests boot a QEMU VM and need `qemu`, `podman`, `ovmf`,
`cryptsetup`, and root (for loop mounts and pflash). See the `e2e*` recipes:

```console
$ just e2e          # Bluefin stable → Dakota (btrfs)
$ just e2e-lts      # Bluefin LTS → Dakota (xfs + loopback)
$ just e2e-luks     # Bluefin LTS → Dakota (xfs + LUKS)
```

## Before you open a PR

- `just check` passes (this is what CI's `validate` job runs).
- `cargo deny check` passes if you touched dependencies.
- Commits follow the `component: Summary` convention described in
  [REVIEW.md](REVIEW.md); fixups are squashed.
- New non-trivial logic has unit tests (prefer table-driven), and migration
  behavior is exercised by the e2e suite.

## Code review

Please read [REVIEW.md](REVIEW.md) — it describes the testing, code-quality, and
commit-message expectations applied here. AI-assisted contributions must follow
[AGENTS.md](AGENTS.md) (no automatic `Signed-off-by`; add an `Assisted-by:`
trailer).

## License

By contributing, you agree that your contributions are dual-licensed under the
[MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE) licenses.
