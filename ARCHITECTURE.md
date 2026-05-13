# persona-message — architecture

*NOTA-to-router CLI boundary. One NOTA in, one NOTA out;
no daemon, no actor runtime, no durable state.*

`persona-message` owns the `message` CLI. It validates one NOTA request,
projects supported message operations into `signal-persona-message` frames,
sends them to `persona-router`'s **public ingress socket**
(`router-public.sock`, mode 0660), and projects the typed router reply back
to NOTA. The word "proxy" in earlier framings of this repo meant *"boundary
translator at the CLI surface"*, not *"proxy daemon process"*. There is no
intermediate daemon; see
`~/primary/reports/designer/142-supervision-in-signal-persona-no-message-proxy-daemon.md`.

> **Scope.** Any "sema" reference in this doc means today's `sema`
> library (rename pending → `sema-db`). The eventual `Sema` is broader; today's
> persona-message is a realization step. See `~/primary/ESSENCE.md` §"Today and
> eventually".

---

## 0 · TL;DR

This repo is a **CLI boundary**, not a daemon and not a
durable message ledger. The `message` binary is a one-shot
translator from NOTA to a `signal-persona-message` frame
sent to the router's public ingress socket.

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
- no caller-identity resolution and no local actor index.

## 2 · State and Ownership

The proxy owns no durable message state. It requires
`PERSONA_MESSAGE_ROUTER_SOCKET` and exits if the router socket is absent.

Caller identity is not resolved in this repo. `MessageSubmission` and
`InboxQuery` stay sender-free, and the proxy sends no in-band proof material.
Router/daemon ingress stamps provenance from the accepted socket context.

Actor registration, actor listing, pending delivery, retry, delivery results,
and message ledger state are router or engine-manager concerns, not proxy
state.

## 3 · Boundaries

This repo owns:

- NOTA parsing for the `message` command;
- projection from NOTA `Send` / `Inbox` to `signal-persona-message`;
- projection from `signal-persona-message` replies back to NOTA;
- length-prefixed Signal frame transport to the configured router socket.

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
- Sender identity is absent from the CLI payload and absent from frame auth.
- Provenance is stamped by router/daemon ingress, not by this proxy.
- The proxy does not write local message or pending logs.
- The proxy does not build or run a daemon.
- The proxy does not depend on an actor runtime.

## Code Map

```text
src/main.rs       message CLI entry
src/command.rs    NOTA input/output projection
src/router.rs     Signal router client
src/surface.rs    proxy-local NOTA surface records
src/error.rs      crate error enum
tests/            proxy and architectural-truth tests
```

## Constraint Tests

| Constraint | Test |
|---|---|
| The router Signal path cannot create a local message ledger. | `nix flake check .#message-cli-sends-router-signal-without-local-ledger` |
| Inbox reads come from the router, not a local ledger. | `nix flake check .#message-cli-inbox-uses-router-signal-not-local-ledger` |
| The router socket is mandatory. | `nix flake check .#message-cli-requires-router-socket` |
| The proxy does not construct in-band proof material. | `nix flake check .#message-proxy-cannot-own-local-ledger` |
| Retired terminal-brand vocabulary cannot return. | `nix flake check .#message-runtime-cannot-reference-retired-terminal-brand` |
| Local ledger, daemon, endpoint, and actor-runtime surfaces cannot return. | `nix flake check .#message-proxy-cannot-own-local-ledger` |

## See Also

- `../signal-persona-message/ARCHITECTURE.md`
- `../persona-router/ARCHITECTURE.md`
- `../signal-persona/ARCHITECTURE.md`
