# Persona Message

`persona-message` is Persona's human-facing message CLI and router proxy. The
`message` binary accepts one NOTA input record, decodes it through Rust types,
and, when `PERSONA_MESSAGE_ROUTER_SOCKET` is set, sends a length-prefixed
`signal-persona-message` frame to `persona-router`.

The message binary contract belongs to `signal-persona-message`. This
repository remains useful as the text boundary for harnesses and humans: NOTA
in, typed validation, NOTA projection out.

The target send path is:

```sh
PERSONA_MESSAGE_ROUTER_SOCKET=/run/persona/router.sock \
  message '(Send designer "Need a layout pass.")'
```

That path returns a NOTA projection of the router's typed reply, such as
`(SubmissionAccepted 7)`, and does not write `messages.nota.log`. Durable
message acceptance and delivery state belong to `persona-router`.

The old local ledger remains as a development fallback for early
harness-to-harness tests. A harness registers the process identity that should
own its outbound legacy messages:

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

The visible Pi router-relay test still exercises trained harness messaging
through the legacy router-line compatibility path:

```sh
nix run .#test-pty-pi-router-relay
```

It starts trained `initiator` and `responder` Pi harnesses, routes the first
operator instruction through `message '(Send initiator ...)'`, then expects the
agents to use `message` themselves: initiator messages responder, responder
replies to initiator, and initiator reports completion to operator. The same
run also verifies focus and prompt guards before releasing queued deliveries
through pushed observations. This compatibility path uses `PERSONA_ROUTER_SOCKET`
until `persona-router` accepts `signal-persona-message` frames directly.

BEADS remains useful for today's workspace coordination, but it is not part of
the Persona API. Persona coordination flows through relation-specific typed
Signal frames, durable component state lives in component-owned Sema layers
over the `sema` library, and delivery policy lives in `persona-router`, with
NOTA kept at the human and harness projection boundaries.
