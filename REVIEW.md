# Code Review Guidelines

These guidelines mirror those used across the bootc-dev organization (bootc,
composefs-rs). They capture the expectations that have emerged from real review
feedback and apply to both authoring and reviewing changes here.

## Testing

Tests are expected for all non-trivial changes — unit and, where it makes sense,
end-to-end. If something is genuinely hard to test right now, at least state
that it was tested manually and how.

### Choosing the right test type

Unit tests are appropriate for parsing logic, data transformations, and
self-contained functions. Use the end-to-end suite (`tests/run-e2e.sh`, driven
by `just e2e*`) for anything that involves real disks, mounts, or booting a VM.

Default to table-driven tests rather than a separate `#[test]` per case.
LLMs in particular tend to generate the latter, which gets verbose fast —
context windows matter to both humans and LLMs reading the code later.

### Separating parsing from I/O

Structure code for testability: have a parser accept a `&str` (or `&[u8]`), and
a separate function that reads from disk and calls the parser. This keeps unit
tests free of filesystem dependencies. `os_release.rs` and `kernel_options.rs`
are the model to follow.

### Test assertions

Make assertions strict and specific. Don't merely check that code "didn't
crash" — verify that outputs match expected values.

## Code quality

### Parsing structured data

Never parse structured formats (JSON, INI, etc.) with text tools like `grep` or
`sed`. Use `serde_json` / the `tini` INI parser already in the dependency tree.

### Shell scripts

Avoid shell scripts longer than ~50 lines where a higher-level structure (a
`just` recipe, or Rust glue) would be clearer. `tests/run-e2e.sh` is the one
large exception; keep new logic out of it where practical.

### Constants and magic values

Extract magic numbers and repeated strings into named constants with a comment
explaining any non-obvious choice (buffer sizes, size thresholds, retry counts).

### Don't swallow errors

Avoid `if let Ok(v) = ...` in Rust or `... 2>/dev/null || true` in shell by
default. Most errors should propagate; if one is deliberately ignored, log it
(at least at debug level) and say why. Handle edge cases explicitly — missing
data, malformed input, offline systems — with error messages that give clear
context for diagnosis.

### Code organization

Separate I/O, parsing, and business logic into different functions. Duplicating
a little code twice can be fine; three copies asks for deduplication.

## Rust-specific guidance

Prefer `rustix` over `libc`. `unsafe` is denied at the crate level
(`[lints.rust] unsafe_code = "deny"`); any reintroduction must be very carefully
justified and documented at the call site.

New dependencies should be justified — prefer well-maintained, widely-used crates
and keep `cargo deny` (`deny.toml`) happy. When adding a command or output
format, design for machine-readable output (JSON) early.

## Commits and pull requests

### Commit organization

Break changes into logical, atomic commits a reviewer can follow. Keep
preparatory refactoring separate from behavioral changes.

### Commit messages

Use a `component: Summary` subject in the imperative mood (e.g.
`xattr: share copy helper between mergetc and file copy`). The body should start
with at least a sentence on **why** the change is being made — even for something
apparently trivial. Don't restate what the diff already shows or add redundant
`Changes:`/`Files changed:` sections. Briefly note non-obvious consequences or
discarded alternatives where useful. `Closes:` tags go at the end.

### Follow-up changes

Squash fixups (CI fixes, review-comment applications, auto-generated
"Update <file>" commits) into the commit they belong to. A commit either stands
alone with its own rationale or it should be squashed.

### Before merge

Self-review your diff first. Do not add `Signed-off-by` automatically — that
requires explicit human action after review. If the change was AI-assisted,
include an `Assisted-by:` trailer (see [AGENTS.md](AGENTS.md)).

## Architecture and design

When implementing a workaround, document where the proper fix belongs and link
the relevant upstream issue. Prefer pushing fixes upstream when the root cause is
in a dependency (ostree, bootc, composefs). When rewriting functionality, verify
the new code path handles every case the old one did.
