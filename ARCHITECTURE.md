# persona-message — architecture

*Engine message ingress / text boundary. Owns the
`message` CLI and the `persona-message` daemon (binary:
`persona-message-daemon`), the supervised first-stack
component.*

`persona-message` owns two binaries:

- The `message` CLI — one NOTA in, one NOTA out. Validates a
  user-typed NOTA record through Rust types, projects to a
  `signal-persona-message` frame, sends it to `persona-message`
  on the engine's user-writable socket (`message.sock`, mode
  0660), reads one reply frame, prints the NOTA reply.
- The `persona-message` daemon (binary file:
  `persona-message-daemon`) — a small Kameo daemon supervised
  by `persona-daemon` as the engine's message ingress
  component. Binds `message.sock` at mode 0660 with the
  engine-owner group by applying the `PERSONA_SOCKET_MODE` value
  from the Persona spawn envelope; stamps `MessageSubmission`
  frames with SO_PEERCRED-derived origin and ingress time; forwards
  `StampedMessageSubmission` frames to `persona-router`'s
  internal socket (`router.sock`, 0600).

There is no `MessageProxy` component here. The supervised
first-stack component is named `persona-message`; the long-lived
binary is `persona-message-daemon`.

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
    "message CLI" -->|"length-prefixed signal-persona-message frame"| "persona-message"
    "persona-message" -->|"StampedMessageSubmission"| "persona-router"
    "persona-router" -->|"length-prefixed reply frame"| "persona-message"
    "persona-message" -->|"length-prefixed reply frame"| "message CLI"
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
- no caller-provided identity and no local actor index.

## 1.5 · Daemon actor topology

```mermaid
flowchart TB
    "MessageDaemonRoot" --> "MessageDaemonConnection"
    "MessageDaemonConnection" --> "SignalRouterClient"
    "SignalRouterClient" --> "persona-router"
```

The current slice has one data-bearing Kameo root actor:
`MessageDaemonRoot { router, stamper, forwarded_count }`. Connection handling,
SO_PEERCRED extraction, origin stamping, and the router client are ordinary
data-bearing types for now. The next actor split is supervision, listener,
origin stamping, and router-client actors. The daemon is stateless across CLI
requests — no redb, no durable message ledger.

The CLI surface (`message` binary) connects to `message.sock` like any other
client. The daemon converts client-side `MessageSubmission` into
router-side `StampedMessageSubmission` by attaching typed provenance from the
kernel peer credentials and a daemon-minted ingress timestamp. It does not
encode provenance as strings.

## 2 · State and Ownership

The message component owns no durable message state. The CLI requires
`PERSONA_MESSAGE_SOCKET` or `PERSONA_SOCKET_PATH` and exits if the message
daemon socket is absent. The daemon requires `PERSONA_MESSAGE_ROUTER_SOCKET`
or a `router` peer socket in the spawn envelope and exits if the router socket
is absent.

Caller identity is not accepted from the model or CLI payload.
`MessageSubmission` and `InboxQuery` stay sender-free, and the component sends
no in-band proof material. The daemon stamps message submissions from
SO_PEERCRED and forwards typed provenance in `StampedMessageSubmission`.

Actor registration, actor listing, pending delivery, retry, delivery results,
and message ledger state are router or engine-manager concerns, not message
state.

## 3 · Boundaries

This repo owns:

- NOTA parsing for the `message` command;
- projection from NOTA `Send` / `Inbox` to `signal-persona-message`;
- projection from `signal-persona-message` replies back to NOTA;
- length-prefixed Signal frame transport from CLI to `message.sock`;
- frame-level exchange echoing for the current one-operation request/reply
  path;
- daemon stamping from `MessageSubmission` to `StampedMessageSubmission`;
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
- The daemon applies the managed spawn-envelope socket mode to
  `message.sock` before accepting client traffic.
- CLI and daemon outbound traffic are length-prefixed rkyv Signal frames.
- Request/reply matching is frame-level: every request frame carries an
  `ExchangeIdentifier`, and every reply frame echoes the same identifier.
- The current message ingress path is deliberately one operation per request.
  Multi-operation request execution belongs in the shared Signal runtime slice,
  not in this component's ad hoc codec.
- A mismatched outer Signal verb and request payload is rejected as typed
  `RequestRejectionReason`, not by string parsing.
- Sender identity is absent from the CLI payload and absent from frame auth.
- Provenance is typed in `StampedMessageSubmission`; the daemon mints it from
  SO_PEERCRED and never accepts it from the CLI payload.
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
| The daemon applies the managed spawn-envelope socket mode. | `nix flake check .#message-daemon-applies-spawn-envelope-socket-mode` |
| The daemon stamps and forwards CLI Signal frames to the router socket. | `nix flake check .#persona-message-daemon-forwards-cli-signal-frame-to-router-socket` |
| Mismatched Signal verb/payload pairs are rejected by typed Signal reason. | `nix flake check .#message-frame-codec-rejects-mismatched-signal-verb` |
| The component does not construct in-band proof material. | `nix flake check .#message-component-cannot-own-local-ledger` |
| Retired terminal-brand vocabulary cannot return. | `nix flake check .#message-runtime-cannot-reference-retired-terminal-brand` |
| Local ledger and endpoint surfaces cannot return. | `nix flake check .#message-component-cannot-own-local-ledger` |

## See Also

- `../signal-persona-message/ARCHITECTURE.md`
- `../persona-router/ARCHITECTURE.md`
- `../signal-persona/ARCHITECTURE.md`
