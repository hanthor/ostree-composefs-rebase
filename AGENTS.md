# Instructions for AI agents

This project follows the conventions of the wider bootc-dev / composefs-rs
ecosystem. If you are an LLM or an LLM-assisted tool contributing here, follow
the guidance below.

## CRITICAL instructions for generating commits

### Signed-off-by

Human review is required for all code that is generated or assisted by a large
language model. If you are an LLM, you MUST NOT add a `Signed-off-by` trailer to
automatically generated git commits. Only an explicit human action or request
should add a `Signed-off-by`. If you open a pull request and the DCO check fails,
tell the human to review the code and explain how to add a signoff.

### Attribution

When generating substantial amounts of code, you SHOULD include an
`Assisted-by: TOOLNAME (MODELNAME)` trailer. For example,
`Assisted-by: Claude Code (Opus 4.8)`.

## Code guidelines

[REVIEW.md](REVIEW.md) describes expectations around testing, code quality,
commit messages, and commit organization. After each commit — and especially
when you believe a task is complete — you are strongly encouraged to review your
change against those guidelines (a subagent review is a good way to do this),
alongside looking for any other issues. The same applies when reviewing others'
code.

Key project-specific points (see REVIEW.md for the full list):

- Prefer `rustix` over `libc`; `unsafe` is denied via `[lints.rust]` and must be
  carefully justified if ever reintroduced.
- Keep parsing separate from I/O so logic stays unit-testable; prefer
  table-driven tests.
- Run `just check` (clippy, rustfmt, unit tests, shellcheck) before opening a PR.

## Follow other guidelines

Read [README.md](README.md) and [CONTRIBUTING.md](CONTRIBUTING.md) and follow
the contribution guidance there. Current project status and the active
workstream live in [HANDOFF.md](HANDOFF.md).
