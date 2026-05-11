# persona-message — architecture

*NOTA boundary and stateless router proxy for Persona messages.*

`persona-message` owns the `message` CLI. It validates one NOTA request,
projects supported message operations into `signal-persona-message` frames,
sends them to `persona-router`, and projects the typed router reply back to
NOTA.

> **Scope.** Any "sema" reference in this doc means today's `sema`
> library (rename pending → `sema-db`). The eventual `Sema` is broader; today's
> persona-message is a realization step. See `~/primary/ESSENCE.md` §"Today and
> eventually".

---

## 0 · TL;DR

This repo is a text boundary and proxy, not a durable message ledger.

```mermaid
flowchart LR
    "human or harness" -->|"one NOTA Send or Inbox"| "message CLI"
    "message CLI" -->|"length-prefixed signal-persona-message frame"| "persona-router"
    "persona-router" -->|"length-prefixed reply frame"| "message CLI"
    "message CLI" -->|"one NOTA reply"| "human or harness"
```

## 1 · Component Surface

`persona-message` exposes:

- a `message` binary;
- NOTA `Send` and `Inbox` input records;
- one length-prefixed `signal-persona-message` request frame per invocation;
- one NOTA reply projection per invocation;
- read-only caller resolution from the transitional `actors.nota` file.

## 2 · State and Ownership

The proxy owns no durable message state. It requires
`PERSONA_MESSAGE_ROUTER_SOCKET` and exits if the router socket is absent.

Caller identity is resolved from process ancestry against `actors.nota` and
carried as Signal auth. The `MessageSubmission` and `InboxQuery` payloads remain
sender-free.

The transitional `actors.nota` file is read-only from this repo. Actor
registration, actor listing, pending delivery, retry, delivery results, and
message ledger state are router or engine-manager concerns, not proxy state.

## 3 · Boundaries

This repo owns:

- NOTA parsing for the `message` command;
- projection from NOTA `Send` / `Inbox` to `signal-persona-message`;
- projection from `signal-persona-message` replies back to NOTA;
- process-ancestry caller lookup for Signal auth.

This repo does not own:

- message or router contract definitions;
- final routing policy;
- durable database tables;
- actor registration writes;
- local message ledgers;
- terminal endpoint vocabulary;
- terminal byte transport;
- daemon runtime state.

## 4 · Invariants

- The CLI accepts exactly one NOTA input record.
- The CLI prints exactly one NOTA reply record.
- Supported input variants are `Send` and `Inbox`.
- The router socket is mandatory.
- Outbound traffic is a length-prefixed rkyv Signal frame.
- Caller identity is Signal auth, not a request payload field.
- The proxy does not write local message or pending logs.
- The proxy does not build or run a daemon.
- The proxy does not depend on an actor runtime.

## Code Map

```text
src/main.rs       message CLI entry
src/command.rs    NOTA input/output projection
src/resolver.rs   read-only process ancestry to actor lookup
src/router.rs     Signal router client
src/schema.rs     proxy-local NOTA actor id records
src/error.rs      crate error enum
tests/            proxy and architectural-truth tests
```

## Constraint Tests

| Constraint | Test |
|---|---|
| The router Signal path cannot create a local message ledger. | `nix flake check .#message-cli-sends-router-signal-without-local-ledger` |
| Inbox reads come from the router, not a local ledger. | `nix flake check .#message-cli-inbox-uses-router-signal-not-local-ledger` |
| The router socket is mandatory. | `nix flake check .#message-cli-requires-router-socket` |
| Retired terminal-brand vocabulary cannot return. | `nix flake check .#message-runtime-cannot-reference-retired-terminal-brand` |
| Local ledger, daemon, endpoint, and actor-runtime surfaces cannot return. | `nix flake check .#message-proxy-cannot-own-local-ledger` |

## See Also

- `../signal-persona-message/ARCHITECTURE.md`
- `../persona-router/ARCHITECTURE.md`
- `../signal-persona/ARCHITECTURE.md`
