# persona-message skill

Work here when the change concerns the `message` CLI, `persona-message-daemon`,
NOTA message projection, message ingress, or real harness message tests.

Rules for work here:

- Keep the repo at the human/harness text boundary. Message binary records
  belong in `signal-persona-message`.
- `message` sends length-prefixed rkyv Signal frames to
  `persona-message-daemon` through `PERSONA_MESSAGE_SOCKET` and prints one NOTA
  reply projection.
- `persona-message-daemon` binds the supervised `message.sock`, forwards typed
  frames to `persona-router`, and owns no durable message state.
- The component must not write local message ledgers, pending logs, or
  actor-registration files. Router-owned Sema tables are the durable message
  owner.
- Do not trust sender fields written by a model. The component does not include
  a sender field, read a local actor index, resolve process ancestry, or
  construct in-band proof material. Origin stamping must wait for the typed
  stamped-submission contract instead of being encoded as strings.
- Supported input variants are `Send` and `Inbox`. Registry, listing, retry,
  tail, and delivery operations belong to router, mind, harness, or terminal
  surfaces as their contracts land.
- Do not add a local ledger fallback, terminal endpoint vocabulary, or router
  line-protocol fallback here.
- Rebuild stateful harness workflows through `persona-router`,
  `persona-harness`, `persona-terminal`, and typed Signal contracts.

Use component-to-component rkyv frames through relation-specific Signal
contracts when the CLI or daemon crosses into router/store territory. Use NOTA
only at CLI, harness, and audit projection boundaries.
