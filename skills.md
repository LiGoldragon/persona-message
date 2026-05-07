# persona-message skill

Work here when the change concerns the `message` CLI, NOTA message projection,
sender resolution, the transitional local ledger, or real harness message tests.

Rules for work here:

- Keep the repo at the human/harness text boundary. Shared binary records
  belong in `signal-persona`.
- Keep sender identity resolved by process ancestry and actor registration; do
  not trust sender fields written by a model.
- Keep stateful harness workflows named under `scripts/` and exposed by
  `flake.nix`.
- Keep real harness tests interactive and persistent. Do not replace them with
  non-interactive `claude --print` or `codex exec` checks.
- Treat the local ledger as transitional development state. Do not deepen it
  into the final database surface.

Use component-to-component rkyv frames through `signal-persona` when the CLI or
daemon crosses into router/store territory. Use NOTA only at CLI, harness, and
audit projection boundaries.

