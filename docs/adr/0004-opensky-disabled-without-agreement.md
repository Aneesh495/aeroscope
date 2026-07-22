# ADR 0004: OpenSky operational use is disabled without a written agreement

## Status
Accepted.

## Context
OpenSky's current terms require a written agreement for operational REST API
use covering display, retention, and redistribution. An API token or account
is not that agreement. The adapter must exist and be tested, but it must not be
possible to accidentally run live traffic against OpenSky without the
deployment explicitly asserting it has the agreement.

## Decision
The OpenSky client is gated by an `OperationalGuard`. Its default state is
disabled with a descriptive reason; `ensure_token` returns an error (not a
network call) until `OperationalGuard::with_signed_agreement()` is used, which
is a deliberate, auditable call a deployment makes only when it truly has the
agreement. The capability registry marks OpenSky redistribution as
`DisabledUnlicensed` and retention as zero by default.

## Consequences
- Offline replay and the local readsb path work with no OpenSky access at all.
- The full live gate G01 stays blocked in any environment lacking the
  agreement, and the status is reported as blocked rather than quietly passed.
- Credentials never live in browser code; the token flow is server-side and the
  transport is abstracted so tests use no real secrets.
