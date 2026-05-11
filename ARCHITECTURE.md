# persona-message — architecture

*Human and harness NOTA boundary for Persona messages.*

`persona-message` owns the `message` CLI and the transitional message ledger
used while the router and durable state are being assembled. It validates NOTA input
from a human or harness, resolves sender identity from the running process, and
projects typed message records back to NOTA.

> **Scope.** Any "sema" reference in this doc means today's `sema`
> library (rename pending → `sema-db`). The eventual `Sema` is
> broader; today's persona-message is a realization step. See
> `~/primary/ESSENCE.md` §"Today and eventually".

---

## 0 · TL;DR

This repo is the text boundary, not the binary contract. Component-to-component
traffic uses `signal-persona-message`; durable assembled state belongs to the
router-owned Sema layer over the `sema` library.

```mermaid
flowchart LR
    "human or harness" -->|"NOTA Send"| "message CLI"
    "message CLI" -->|"Register"| "actors.nota"
    "actors.nota" -->|"process ancestry"| "message CLI"
    "message CLI" -->|"typed validation"| "DaemonRoot"
    "DaemonRoot" -->|"store intent"| "Ledger"
    "Ledger" -->|"serialized writes"| "local message ledger"
    "message CLI" -->|"Frame request"| "signal-persona-message"
    "persona-router" -->|"pre-harness NOTA projection"| "message CLI"
```

## 1 · Component Surface

`persona-message` exposes:

- `message` CLI for NOTA input/output;
- `message-daemon` as the transitional daemon surface;
- a Kameo `DaemonRoot` that owns daemon request intake;
- a supervised Kameo `Ledger` child that owns transitional ledger reads and
  writes behind the root;
- local actor resolution from process ancestry;
- actor registration and listing through `Register` and `Agents`;
- local append/read surfaces for message tests;
- stateful harness scripts exposed through Nix apps.

## 2 · State and Ownership

The current local ledger is development state. It keeps harness-to-harness tests
usable before `persona-router` fully owns delivery and router-scoped durable
commits. While the daemon is running, client requests enter through
`DaemonRoot`. Ledger reads and writes then cross into the supervised `Ledger`,
so the root coordinates daemon requests and the ledger owns the mutation plane.

In the assembled runtime:

- `persona-message` remains the NOTA CLI/projection layer;
- `persona-router` owns routing, pending delivery, and durable message
  transitions;
- `persona-router` uses a router-owned Sema layer for typed storage tables;
- `signal-persona-message` owns the message channel wire records.

## 3 · Boundaries

This repo owns:

- NOTA `Register`, `Agents`, `Send`, `Inbox`, and `Tail` CLI surfaces;
- sender resolution from process ancestry;
- human/harness message projection;
- stateful real-harness test scripts.

This repo does not own:

- rkyv frame types owned by contract repos;
- final routing policy;
- final durable database;
- OS/window-manager focus observations;
- terminal byte transport.

## 4 · Invariants

- Sender identity is trusted from process ancestry, not model text.
- Agents register their local process identity before sending; ad hoc
  `actors.nota` edits are a fallback for debugging, not the normal path.
- NOTA input is decoded into typed Rust before it affects state.
- Daemon requests touch the transitional ledger only through Kameo mailboxes:
  `DaemonRoot` first, supervised `Ledger` second.
- Kameo messages are data-bearing; empty marker messages are forbidden for
  runtime-path tests and inspection.
- Harness tests target interactive persistent harnesses, not non-interactive
  provider commands.
- Repeated debug commands become named scripts and Nix apps.
- BEADS remains outside the Persona API.

## Code Map

```text
src/main.rs            message CLI entry
src/bin/message-daemon.rs
src/actors/            Kameo actor planes
src/schema.rs          NOTA-facing records
src/resolver.rs        process ancestry sender resolution
src/store.rs           transitional local ledger
src/daemon.rs          transitional daemon surface and daemon root actor
scripts/               repeatable stateful harness workflows
tests/                 CLI, daemon, actor-runtime, two-process, and harness tests
```

## See Also

- `../signal-persona/ARCHITECTURE.md`
- `../signal-persona-message/ARCHITECTURE.md`
- `../persona-router/ARCHITECTURE.md`
- `../sema/ARCHITECTURE.md`
- `../persona-wezterm/ARCHITECTURE.md`
