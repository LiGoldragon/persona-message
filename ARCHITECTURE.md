# persona-message — architecture

*Human and harness NOTA boundary for Persona messages.*

`persona-message` owns the `message` CLI and the transitional message ledger
used while the router and store are being assembled. It validates NOTA input
from a human or harness, resolves sender identity from the running process, and
projects typed message records back to NOTA.

---

## 0 · TL;DR

This repo is the text boundary, not the shared binary contract. Component-to-
component traffic uses `persona-signal`; durable assembled state belongs behind
`persona-store`.

```mermaid
flowchart LR
    "human or harness" -->|"NOTA Send"| "message CLI"
    "actors.nota" -->|"process ancestry"| "message CLI"
    "message CLI" -->|"typed validation"| "local message ledger"
    "message CLI" -->|"Frame request"| "persona-signal"
    "persona-router" -->|"pre-harness NOTA projection"| "message CLI"
```

## 1 · Component Surface

`persona-message` exposes:

- `message` CLI for NOTA input/output;
- `message-daemon` as the transitional daemon surface;
- local actor resolution from process ancestry;
- local append/read surfaces for message tests;
- stateful harness scripts exposed through Nix apps.

## 2 · State and Ownership

The current local ledger is development state. It keeps harness-to-harness tests
usable before `persona-router` and `persona-store` fully own delivery and
durable commits.

In the assembled runtime:

- `persona-message` remains the NOTA CLI/projection layer;
- `persona-router` owns routing and pending delivery;
- `persona-store` owns durable transition ordering;
- `persona-signal` owns the Rust wire records.

## 3 · Boundaries

This repo owns:

- NOTA `Send`, `Inbox`, and `Tail` CLI surfaces;
- sender resolution from process ancestry;
- human/harness message projection;
- stateful real-harness test scripts.

This repo does not own:

- shared rkyv frame types;
- final routing policy;
- final durable database;
- OS/window-manager focus observations;
- terminal byte transport.

## 4 · Invariants

- Sender identity is trusted from process ancestry, not model text.
- NOTA input is decoded into typed Rust before it affects state.
- Harness tests target interactive persistent harnesses, not non-interactive
  provider commands.
- Repeated debug commands become named scripts and Nix apps.
- BEADS remains outside the Persona API.

## Code Map

```text
src/main.rs            message CLI entry
src/bin/message-daemon.rs
src/schema.rs          NOTA-facing records
src/resolver.rs        process ancestry sender resolution
src/store.rs           transitional local ledger
src/daemon.rs          transitional daemon surface
scripts/               repeatable stateful harness workflows
tests/                 CLI, daemon, two-process, and harness tests
```

## See Also

- `../persona-signal/ARCHITECTURE.md`
- `../persona-router/ARCHITECTURE.md`
- `../persona-store/ARCHITECTURE.md`
- `../persona-wezterm/ARCHITECTURE.md`
