# Persona Message

`persona-message` is the `message` CLI — Persona's NOTA-to-router boundary
translator at the CLI surface. The `message` binary accepts exactly one
NOTA input record, validates it through Rust types, sends one
length-prefixed `signal-persona-message` frame to `persona-router`'s
public ingress socket (`router-public.sock`, mode 0660), reads one
typed reply frame, and prints one NOTA reply. There is no daemon.

The supported input records are:

```sh
PERSONA_MESSAGE_ROUTER_SOCKET=/run/persona/router.sock \
  message '(Send designer "Need a layout pass.")'

PERSONA_MESSAGE_ROUTER_SOCKET=/run/persona/router.sock \
  message '(Inbox designer)'
```

The proxy does not resolve sender identity, construct in-band proof material,
or read a local actor index. The router/daemon side stamps provenance from the
accepted socket context. The proxy does not write message ledgers, pending
logs, terminal endpoints, actor-registration files, or daemon state.

Durable message acceptance, pending delivery, retry, owner approval, and
terminal delivery state belong to `persona-router` and its downstream
`persona-harness` / `persona-terminal` path. This crate owns text projection at
the edge: NOTA in, Signal out, Signal in, NOTA out.
