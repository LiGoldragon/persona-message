# Architecture

Persona Message is a CLI and text-boundary repository. It does not own
Persona's reducer, harness lifecycle, authorization policy, durable state
engine, or binary wire contract. `persona-signal` owns the shared rkyv contract.

```mermaid
flowchart LR
    "operator harness" -->|"Send NOTA"| "message CLI"
    "actors.nota" -->|"process ancestry match"| "message CLI"
    "message CLI" -->|"typed request"| "persona-signal"
    "persona-signal" -->|"commit request"| "persona-store"
    "persona-router" -->|"pre-harness NOTA projection"| "designer harness"
```

The prototype store is an append-only NOTA-line ledger plus a small agent config
file. It exists so harnesses can communicate immediately while the Persona
store and router are still being built. The current record shapes are useful
test fixtures, not the permanent inter-component contract:

- `Message` is the durable unit of communication.
- `Agent` maps a harness name to a process ID for sender resolution.
- `Send` is the caller-facing input; the binary stamps the trusted sender.
- `Inbox` reads the current recipient view.
- `Tail` blocks and prints newly appended messages for the resolved recipient.

The later Persona path replaces the file ledger with `persona-store` and
routes delivery through `persona-router`. BEADS should not become a
compatibility surface for this crate; BEADS is transitional coordination
substrate.
