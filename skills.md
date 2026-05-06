# Skill — persona-message

*How to work on Persona's first typed message contract.*

---

## What this repo owns

`persona-message` owns the small NOTA message-plane prototype for Persona. The
important contract is:

- `Actor` maps a harness actor name to a process ID in `actors.nota`.
  Actors may own an endpoint such as a PTY socket or WezTerm pane.
- `Send` is the caller-facing input.
- `Message` is the stored durable fact in `messages.nota.log`.
- `Inbox` and `Tail` are the current read surfaces.

The repo does not own Persona's final reducer, authorization gate, BEADS
compatibility, or harness lifecycle engine.

---

## Working rules

Keep the prototype honest. Do not add destination records unless a test drives
behavior through them. `Authorization` and `Delivery` belong later, when there
is an authorization gate or delivery state machine to exercise them.

Sender identity is not trusted from model text. The binary resolves the caller
through Linux process ancestry and `actors.nota`; tests should preserve that
property.

Actual harness tests are stateful and authenticated. They are ignored by
default and should be run through the named scripts exposed by the flake:

```sh
nix run .#test-basic
nix run .#test-actual-codex-to-claude
nix run .#test-actual-claude-to-codex
nix run .#setup-harnesses-visible
nix run .#setup-harnesses-headless
```

Use `scripts/` for repeatable debug workflows. If you find yourself running a
custom command more than once while debugging WezTerm, Codex, Claude, or the
message store, turn it into a named script and expose it from `flake.nix`.

Actual harness means interactive harness. Do not replace these tests with
`claude --print` or `codex exec`; those are only useful for quick model-alias
checks. The target is a persistent terminal pane that can be attached to,
nudged, and captured.

---

## See also

- `AGENTS.md` — repo instructions and BEADS boundary.
- `ARCHITECTURE.md` — current prototype architecture.
- this workspace's `skills/autonomous-agent.md` — stateful test commands should
  become Nix-runnable scripts.
