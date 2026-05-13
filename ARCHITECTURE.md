# persona-message — architecture

*Engine message ingress / text boundary. Owns the
`message` CLI and the `persona-message-daemon` supervised
first-stack daemon.*

`persona-message` owns two binaries:

- The `message` CLI — one NOTA in, one NOTA out. Validates a
  user-typed NOTA record through Rust types, projects to a
  `signal-persona-message` frame, sends it to
  `persona-message-daemon` on the engine's user-writable
  socket (`message.sock`, mode 0660), reads one reply frame,
  prints the NOTA reply.
- The `persona-message-daemon` — a small Kameo daemon
  supervised by `persona-daemon` as the engine's message
  ingress component. Binds `message.sock` at mode 0660 with
  the engine-owner group; forwards typed Signal frames to
  `persona-router`'s internal socket (`router.sock`, 0600).
  SO_PEERCRED origin stamping waits on the stamped-submission
  contract in `signal-persona-message`.

There is no `MessageProxy` component here. Per
`~/primary/reports/designer/142-supervision-in-signal-persona-no-message-proxy-daemon.md`,
the supervised first-stack component is named
`persona-message`; the long-lived binary is
`persona-message-daemon`.

> **Scope.** Any "sema" reference in this doc means today's `sema`
> library (rename pending → `sema-db`). The eventual `Sema` is broader; today's
> persona-message is a realization step. See `~/primary/ESSENCE.md` §"Today and
> eventually".

---

## 0 · TL;DR

This repo owns the engine's message-ingress boundary: a
small supervised daemon plus a CLI client. Neither carries
a durable message ledger; both are stateless boundary
surfaces. Routing policy, delivery state, and channel
authority remain in `persona-router`.

```mermaid
flowchart LR
    "human or harness" -->|"one NOTA Send or Inbox"| "message CLI"
    "message CLI" -->|"length-prefixed signal-persona-message frame"| "persona-message-daemon"
    "persona-message-daemon" -->|"forward typed frame"| "persona-router"
    "persona-router" -->|"length-prefixed reply frame"| "persona-message-daemon"
    "persona-message-daemon" -->|"length-prefixed reply frame"| "message CLI"
    "message CLI" -->|"one NOTA reply"| "human or harness"
```

## 1 · Component Surface

`persona-message` exposes:

- a `message` binary;
- a `persona-message-daemon` binary;
- NOTA `Send` and `Inbox` input records;
- one length-prefixed `signal-persona-message` request frame per CLI invocation;
- one daemon-bound `message.sock` for owner ingress;
- one router client path to internal `router.sock`;
- one NOTA reply projection per invocation;
- no caller-identity resolution and no local actor index.

## 1.5 · Daemon actor topology

Per
`~/primary/reports/designer/142-supervision-in-signal-persona-no-message-proxy-daemon.md` §3.3
and
`~/primary/reports/designer/143-prototype-readiness-gap-audit.md` §4.8:

```mermaid
flowchart TB
    "MessageDaemonRoot" --> "MessageDaemonConnection"
    "MessageDaemonConnection" --> "SignalRouterClient"
    "SignalRouterClient" --> "persona-router"
```

The current slice has one data-bearing Kameo root actor:
`MessageDaemonRoot { router, forwarded_count }`. Connection handling and the
router client are ordinary data-bearing types for now. The next actor split is
supervision, listener, origin stamping, and router-client actors after the
stamped-submission contract lands. The daemon is stateless across CLI requests
— no redb, no durable message ledger.

The CLI surface (`message` binary) connects to `message.sock` like any
other client. The current implementation forwards the typed
`MessageSubmission` / `InboxQuery` frame to `persona-router` over the internal
`router.sock`. Origin stamping and `StampedMessageSubmission` are the next
contract step in `signal-persona-message`; this repo must not fake that by
encoding provenance as strings.

## 2 · State and Ownership

The message component owns no durable message state. The CLI requires
`PERSONA_MESSAGE_SOCKET` or `PERSONA_SOCKET_PATH` and exits if the message
daemon socket is absent. The daemon requires `PERSONA_MESSAGE_ROUTER_SOCKET`
or a `router` peer socket in the spawn envelope and exits if the router socket
is absent.

Caller identity is not resolved in this repo. `MessageSubmission` and
`InboxQuery` stay sender-free, and the component sends no in-band proof
material. The stamped-origin bridge is explicitly pending in the
`signal-persona-message` contract.

Actor registration, actor listing, pending delivery, retry, delivery results,
and message ledger state are router or engine-manager concerns, not message
state.

## 3 · Boundaries

This repo owns:

- NOTA parsing for the `message` command;
- projection from NOTA `Send` / `Inbox` to `signal-persona-message`;
- projection from `signal-persona-message` replies back to NOTA;
- length-prefixed Signal frame transport from CLI to `message.sock`;
- daemon forwarding from `message.sock` to the configured router socket.

This repo does not own:

- message or router contract definitions;
- final routing policy;
- durable database tables;
- actor registration writes;
- local message ledgers;
- terminal endpoint vocabulary;
- terminal byte transport;
- durable daemon state.

## 4 · Invariants

- The CLI accepts exactly one NOTA input record.
- The CLI prints exactly one NOTA reply record.
- Supported input variants are `Send` and `Inbox`.
- The message daemon socket is mandatory for the CLI.
- The router socket is mandatory for the daemon.
- CLI and daemon outbound traffic are length-prefixed rkyv Signal frames.
- Sender identity is absent from the CLI payload and absent from frame auth.
- Provenance must be typed by the stamped-submission contract before this repo
  claims to stamp it.
- The component does not write local message or pending logs.
- The daemon root is a data-bearing Kameo actor.

## Code Map

```text
src/main.rs                    message CLI entry
src/bin/persona_message_daemon.rs daemon entry
src/command.rs                 NOTA input/output projection
src/daemon.rs                  daemon listener and data-bearing Kameo root
src/router.rs                  Signal frame clients and codec
src/surface.rs                 message-local NOTA surface records
src/error.rs                   crate error enum
tests/                         ingress and architectural-truth tests
```

## Constraint Tests

| Constraint | Test |
|---|---|
| The router Signal path cannot create a local message ledger. | `nix flake check .#message-cli-sends-router-signal-without-local-ledger` |
| Inbox reads come from the router, not a local ledger. | `nix flake check .#message-cli-inbox-uses-router-signal-not-local-ledger` |
| The message daemon socket is mandatory for the CLI. | `nix flake check .#message-cli-requires-message-socket` |
| The daemon forwards CLI Signal frames to the router socket. | `nix flake check .#persona-message-daemon-forwards-cli-signal-frame-to-router-socket` |
| The component does not construct in-band proof material. | `nix flake check .#message-component-cannot-own-local-ledger` |
| Retired terminal-brand vocabulary cannot return. | `nix flake check .#message-runtime-cannot-reference-retired-terminal-brand` |
| Local ledger and endpoint surfaces cannot return. | `nix flake check .#message-component-cannot-own-local-ledger` |

## See Also

- `../signal-persona-message/ARCHITECTURE.md`
- `../persona-router/ARCHITECTURE.md`
- `../signal-persona/ARCHITECTURE.md`
