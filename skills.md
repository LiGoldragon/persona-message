# persona-message skill

Work here when the change concerns the `message` CLI, NOTA message projection,
router proxying, or real harness message tests.

Rules for work here:

- Keep the repo at the human/harness text boundary. Message binary records
  belong in `signal-persona-message`.
- `PERSONA_MESSAGE_ROUTER_SOCKET` is required. `message` sends length-prefixed
  rkyv Signal frames and prints one NOTA reply projection.
- The proxy must not write local message ledgers, pending logs, daemon state, or
  actor-registration files. Router-owned Sema tables are the durable message
  owner.
- Do not trust sender fields written by a model. The proxy does not include a
  sender field, read a local actor index, resolve process ancestry, or
  construct in-band proof material; router/daemon ingress stamps provenance
  from the accepted socket context.
- Supported input variants are `Send` and `Inbox`. Registry, listing, retry,
  tail, and delivery operations belong to router, mind, harness, or terminal
  surfaces as their contracts land.
- Do not add an actor runtime, daemon binary, local ledger fallback, terminal
  endpoint vocabulary, or router line-protocol fallback here.
- Rebuild stateful harness workflows through `persona-router`,
  `persona-harness`, `persona-terminal`, and typed Signal contracts.

Use component-to-component rkyv frames through relation-specific Signal
contracts when the CLI or daemon crosses into router/store territory. Use NOTA
only at CLI, harness, and audit projection boundaries.
