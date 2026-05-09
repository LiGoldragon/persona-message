# persona-message skill

Work here when the change concerns the `message` CLI, NOTA message projection,
sender resolution, the transitional local ledger, or real harness message tests.

Rules for work here:

- Keep the repo at the human/harness text boundary. Shared binary records
  belong in `signal-persona`.
- Keep sender identity resolved by process ancestry and actor registration; do
  not trust sender fields written by a model.
- Use `Register` and `Agents` for normal local actor setup and inspection.
  Hand-edit `actors.nota` only when debugging the resolver itself.
- Keep stateful harness workflows named under `scripts/` and exposed by
  `flake.nix`.
- Keep real harness tests interactive and persistent. Do not replace them with
  non-interactive `claude --print` or `codex exec` checks.
- Use `scripts/test-pty-pi-niri-focus` when validating `persona-system` focus
  observations against actual visible Pi harness windows.
- Use `scripts/test-pty-pi-guarded-delivery` when validating that terminal
  delivery defers while a target harness window is focused and delivers only
  after focus moves to a neutral window.
- Treat the local ledger as transitional development state. Do not deepen it
  into the final database surface.

Use component-to-component rkyv frames through `signal-persona` when the CLI or
daemon crosses into router/store territory. Use NOTA only at CLI, harness, and
audit projection boundaries.
