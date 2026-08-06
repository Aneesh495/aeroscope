# Contributing

Aeroscope is a focused engineering project. A few conventions keep it honest
and reviewable.

## Ground rules

- Correctness and honesty over polish. Never show a fabricated or stale
  observation as live. If a result is not measured, say so.
- Keep each crate's one concern. Cross-cutting changes should be rare and
  explained.
- Encode invariants in types where you can (units, clocks, identities), so
  misuse is a compile error rather than a comment.

## Before you push

```sh
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
cd web && npx tsc --noEmit && npx vitest run
```

Add a test for every behavior change and a claim-to-evidence entry in
`docs/CLAIMS.md` for any new behavioral or performance claim.

## Commits

Small, focused commits that describe the behavior change and the checks run.
No force pushes to `main`. CI runs build, format, clippy, tests, and the
verifier negative control on every push.

## Measurements

Any performance number must trace to a run identity and raw data. Diagnostic
(bounded) and full-acceptance profiles are reported separately; a diagnostic
result never stands in for a full-host result.
