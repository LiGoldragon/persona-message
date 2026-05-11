# persona-message — architecture

*Human and harness NOTA boundary for Persona messages.*

`persona-message` owns the `message` CLI and the transitional human/harness
projection surface for Persona messages. It validates one NOTA input record,
converts router-bound message operations into `signal-persona-message` frames,
and projects typed router replies back to NOTA.

> **Scope.** Any "sema" reference in this doc means today's `sema`
> library (rename pending → `sema-db`). The eventual `Sema` is
> broader; today's persona-message is a realization step. See
> `~/primary/ESSENCE.md` §"Today and eventually".

---

## 0 · TL;DR

This repo is the text boundary and proxy, not the durable message ledger.
Component-to-component traffic uses `signal-persona-message`; durable assembled
state belongs to the router-owned Sema layer over the `sema` library.

```mermaid
flowchart LR
    "human or harness" -->|"NOTA Send"| "message CLI"
    "message CLI" -->|"MessageSubmission frame"| "signal-persona-message"
    "signal-persona-message" -->|"length-prefixed rkyv"| "persona-router"
    "persona-router" -->|"MessageReply frame"| "message CLI"
    "message CLI" -->|"NOTA reply"| "human or harness"
    "message-daemon" -. "Kameo mailbox" .-> "Ledger"
    "Ledger" -. "transitional only" .-> "messages.nota.log"
```

## 1 · Component Surface

`persona-message` exposes:

- `message` CLI for NOTA input/output;
- Signal-frame router proxying through `PERSONA_MESSAGE_ROUTER_SOCKET`;
- `message-daemon` as the transitional daemon surface;
- a Kameo `DaemonRoot` that owns daemon request intake;
- a supervised Kameo `Ledger` child that owns transitional ledger reads and
  writes behind the root;
- local actor resolution from process ancestry;
- actor registration and listing through `Register` and `Agents`;
- local append/read surfaces for legacy message tests;
- stateful harness scripts exposed through Nix apps.

## 2 · State and Ownership

The target `Send` and `Inbox` path is `message` → `signal-persona-message` →
`persona-router`. When `PERSONA_MESSAGE_ROUTER_SOCKET` is set, `message`
encodes a length-prefixed Signal frame, waits for one Signal reply frame, and
prints one NOTA projection of that reply. That path must not append to
`messages.nota.log`; the router is the durable owner. The CLI resolves the
caller from process ancestry and carries it as Signal auth. The
`MessageSubmission` payload remains sender-free.

The current local ledger is development state. It keeps older harness tests
usable before `persona-router` fully owns delivery and router-scoped durable
commits. While the transitional daemon is running, client requests enter
through `DaemonRoot`. Ledger reads and writes then cross into the supervised
`Ledger`, so the root coordinates daemon requests and the ledger owns the
mutation plane.

In the assembled runtime:

- `persona-message` remains the NOTA CLI/projection layer;
- `persona-router` owns routing, pending delivery, and durable message
  transitions;
- `persona-router` uses a router-owned Sema layer for typed storage tables;
- `signal-persona-message` owns the message channel wire records.

## 3 · Boundaries

This repo owns:

- NOTA `Register`, `Agents`, `Send`, `Inbox`, and `Tail` CLI surfaces;
- proxying `Send` and `Inbox` to `persona-router` as
  `signal-persona-message` frames;
- sender resolution from process ancestry for Signal auth and for the local
  development path;
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
- With `PERSONA_MESSAGE_ROUTER_SOCKET`, `Send` and `Inbox` use
  `signal-persona-message` length-prefixed rkyv frames.
- The caller identity in the Signal path is auth, not a field in
  `MessageSubmission`.
- The Signal router path never writes `messages.nota.log`; durable message
  acceptance belongs to `persona-router`.
- `persona-router` ingress is Signal-only. Do not add a router line-protocol
  fallback.
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
src/router.rs          Signal router client
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
