# Persona Message

`persona-message` is the engine's message-ingress component. It owns the
`message` CLI and the `persona-message-daemon` supervised first-stack daemon.

The `message` binary accepts exactly one NOTA input record, validates it
through Rust types, sends one length-prefixed `signal-persona-message` frame
to `persona-message-daemon` on the engine's user-writable socket
(`message.sock`, mode 0660), reads one typed reply frame, and prints one
NOTA reply.

The `persona-message-daemon` binary is the engine's user-writable ingress
boundary: it binds `message.sock` (mode 0660, engine-owner group) and forwards
typed Signal frames to `persona-router` over the internal `router.sock`. No
durable state; no local message ledger. SO_PEERCRED origin stamping waits on
the stamped-submission contract in `signal-persona-message`.

The supported input records are:

```sh
PERSONA_MESSAGE_SOCKET=/run/persona/engine-main/message.sock \
  message '(Send designer "Need a layout pass.")'

PERSONA_MESSAGE_SOCKET=/run/persona/engine-main/message.sock \
  message '(Inbox designer)'
```

The message component does not construct in-band proof material, read a local
actor index, or write message ledgers, pending logs, terminal endpoints, or
actor-registration files. Origin stamping is the daemon/router ingress
boundary; as of this slice, the daemon forwards typed message frames and the
stamped-submission contract is still pending in `signal-persona-message`.

Durable message acceptance, pending delivery, retry, owner approval, and
terminal delivery state belong to `persona-router` and its downstream
`persona-harness` / `persona-terminal` path. This crate owns text projection at
the edge: NOTA in, Signal out, Signal in, NOTA out.
