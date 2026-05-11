# persona-message skill

Work here when the change concerns the `message` CLI, NOTA message projection,
sender resolution, the transitional local ledger, or real harness message tests.

Rules for work here:

- Keep the repo at the human/harness text boundary. Message binary records
  belong in `signal-persona-message`.
- Treat `PERSONA_MESSAGE_ROUTER_SOCKET` as the target router path: `message`
  sends length-prefixed rkyv Signal frames and prints one NOTA reply projection.
- The target router path must not write `messages.nota.log`; router-owned Sema
  tables are the durable message owner.
- Keep sender identity resolved by process ancestry and actor registration; do
  not trust sender fields written by a model. In the Signal router path, attach
  the resolved caller as Signal auth; do not add a sender field to
  `MessageSubmission`.
- Use `Register` and `Agents` for normal local actor setup and inspection.
  Hand-edit `actors.nota` only when debugging the resolver itself.
- Keep stateful harness workflows named under `scripts/` and exposed by
  `flake.nix`.
- Keep daemon request intake inside the Kameo `DaemonRoot`; client streams may
  decode frames, but store mutations belong behind that mailbox and then behind
  the supervised `Ledger` child.
- Do not add empty marker Kameo messages. Runtime inspection and mailbox-path
  witnesses carry data describing what is being inspected.
- Keep real harness tests interactive and persistent. Retired terminal harness
  scripts remain only as migration reporters; rebuild their workflows through
  `persona-router`, `persona-harness`, `persona-terminal`, and typed Signal
  contracts.
- The old router-delivery, router-relay, and local terminal-delivery scripts
  are retired. Rebuild those workflows only with typed Signal contracts.
- Treat the local ledger as transitional development state. Do not deepen it
  into the final database or router protocol surface.

Use component-to-component rkyv frames through relation-specific Signal
contracts when the CLI or daemon crosses into router/store territory. Use NOTA
only at CLI, harness, and audit projection boundaries.
