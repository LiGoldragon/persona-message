# Persona Message

`persona-message` is Persona's human-facing message CLI and transitional ledger.
The `message` binary accepts one NOTA input record, decodes it through Rust
types, and stores canonical `Message` records in a local development ledger.

The shared binary contract now belongs to `signal-persona`. This repository
remains useful as the text boundary for harnesses and humans: NOTA in, typed
validation, NOTA projection out.

The first harness-to-harness path is deliberately small. A harness registers
the process identity that should own its outbound messages:

```sh
PERSONA_MESSAGE_STORE=.message message '(Register operator None)'
PERSONA_MESSAGE_STORE=.message message '(Agents)'
```

`Register` writes the same typed `Actor` records that tests used to hand-write
in `actors.nota`, but keeps the sender PID minted by infrastructure instead of
model text. Harnesses then send with `Send`; the binary resolves the sender
from process ancestry and writes the full stored `Message`:

```sh
PERSONA_MESSAGE_STORE=.message message '(Send designer "Need a layout pass.")'
PERSONA_MESSAGE_STORE=.message message '(Inbox designer)'
PERSONA_MESSAGE_STORE=.message message '(Tail)'
```

The visible Pi focus harness test exercises the current Niri focus source
against two real persistent Pi windows:

```sh
nix run .#test-pty-pi-niri-focus
```

It starts `initiator` and `responder` Pi harnesses with `qwen3.6-27b`, attaches
visible WezTerm viewers, discovers their Niri window ids, subscribes through
`persona-system`, and drives focus between the windows.

The visible Pi guarded-delivery test exercises the transitional delivery gate:

```sh
nix run .#test-pty-pi-guarded-delivery
```

It binds actor endpoints to Niri window ids, creates a neutral focus window,
proves delivery is deferred while the responder window is focused, then moves
focus to neutral and flushes the pending message.

The visible Pi router-delivery test exercises the router actor path:

```sh
nix run .#test-pty-pi-router-delivery
```

It starts the `persona-router` daemon, registers the Pi harnesses as actors,
routes one message while the responder window is focused, then routes another
while the responder prompt contains a human draft. Both messages remain pending
until pushed focus or prompt observations make delivery safe.

BEADS remains useful for today's workspace coordination, but it is not part of
the Persona API. Persona coordination flows through typed frames in
`signal-persona`, durable commits in `persona-store`, and delivery policy in
`persona-router`, with NOTA kept at the human and harness projection
boundaries.
