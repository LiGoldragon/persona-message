# Agent Instructions - Persona Message

You MUST read lore's `AGENTS.md` and the primary workspace
orchestration protocol before editing this repository.

## Repo Role

Persona Message is the typed message contract and first shim for
harness-to-harness communication. It owns the NOTA records that prove a message,
authorization, or delivery can cross a harness boundary without becoming a
stringly-typed bead.

## Current Phase

This repo is in contract-prototype phase. Keep the implementation narrow:

- NOTA data types for `Agent` and `Message`.
- A `message` binary that decodes `Send`, `Inbox`, and `Tail`, resolves sender
  identity from process ancestry, writes canonical NOTA lines, and reads
  recipient inboxes from the prototype store.
- Tests that show two named agents can communicate through the typed file
  ledger.

BEADS is transitional workspace coordination. Do not add a BEADS bridge here;
Persona's typed fabric is intended to absorb that role later.

## Version Control

This is a Git-backed colocated Jujutsu repository. Use `jj` for local history
work and keep Git as the remote/storage compatibility layer.

## Rust

Follow lore's Rust discipline: domain values are typed, behavior lives on the
types, errors use one crate enum, and public surfaces speak NOTA unless the
boundary is explicitly binary.
