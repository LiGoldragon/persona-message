# Persona Message

`persona-message` is Persona's NOTA message proxy. The `message` binary accepts
exactly one NOTA input record, validates it through Rust types, sends one
length-prefixed `signal-persona-message` frame to `persona-router`, reads one
typed reply frame, and prints one NOTA reply.

The supported input records are:

```sh
PERSONA_MESSAGE_STORE=.message \
PERSONA_MESSAGE_ROUTER_SOCKET=/run/persona/router.sock \
  message '(Send designer "Need a layout pass.")'

PERSONA_MESSAGE_STORE=.message \
PERSONA_MESSAGE_ROUTER_SOCKET=/run/persona/router.sock \
  message '(Inbox designer)'
```

`PERSONA_MESSAGE_STORE` is transitional and read-only for this proxy. It points
at a directory containing `actors.nota`, used only to resolve the caller from
process ancestry before building Signal auth. The proxy does not write
`actors.nota`, message ledgers, pending logs, terminal endpoints, or daemon
state.

Durable message acceptance, pending delivery, retry, owner approval, and
terminal delivery state belong to `persona-router` and its downstream
`persona-harness` / `persona-terminal` path. This crate owns text projection at
the edge: NOTA in, Signal out, Signal in, NOTA out.
