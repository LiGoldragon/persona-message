# Persona Message

`persona-message` is the first typed message contract for Persona. The `message`
binary accepts one NOTA input record, decodes it through Rust types, and stores
canonical `Message` records in a local prototype ledger.

The first harness-to-harness path is deliberately small. The test setup writes
an `actors.nota` file that maps harness names to parent process IDs:

```nota
(Actor operator 12345 None)
(Actor designer 12346 None)
```

Harnesses send with `Send`; the binary resolves the sender from process
ancestry and writes the full stored `Message`:

```sh
PERSONA_MESSAGE_STORE=.message message '(Send designer "Need a layout pass.")'
PERSONA_MESSAGE_STORE=.message message '(Inbox designer)'
PERSONA_MESSAGE_STORE=.message message '(Tail)'
```

BEADS remains useful for today's workspace coordination, but it is not part of
the destination API. Persona coordination is expected to become typed NOTA
records flowing through one reducer.
