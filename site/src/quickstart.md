# Quickstart

Colloq is a research prototype. It builds with a normal Rust toolchain, needs no
services, and every step below runs on one machine.

## Install

```bash
git clone https://github.com/Vantar-AI/colloq
cd colloq
cargo build --release
```

Rust 1.91 or newer. The binary is `target/release/colloq`.

## 1. Check a conversation

A conversation names the roles, the messages between them, the choices, the loops and
the failures. The checker rejects an unreachable state, an undeclared failure and a
branch that no role can take.

```bash
colloq check examples/route.colloqconv.json
```

```text
valid conversation example.route (2 roles, 11 states)
```

## 2. Project it into endpoints

Each role gets one local protocol machine, derived from the same conversation. Two
servers cannot disagree about the protocol, because neither one wrote it separately.

```bash
colloq project examples/route.colloqconv.json --out build/endpoints
```

## 3. Compile a plan

The plan is the reusable artifact. It carries the conversation identity, the plan
identity and the compact transition table. Both identities are content addressed, so
an edit anywhere produces a different plan.

```bash
colloq compile examples/generate.colloqconv.json --out build/generate.colloqplan.json
```

## 4. Run it

The same plan runs over shared memory, TCP, authenticated QUIC and Iroh. Every
transport reconstructs the same semantic trace.

```bash
colloq run-plan build/generate.colloqplan.json --transport memory --wire compact
colloq run-plan build/generate.colloqplan.json --transport quic   --wire compact
```

## 5. Two processes, mutually authenticated

This creates two persistent identities and the matching authorization policies, then
runs the conversation between two real processes.

```bash
colloq bootstrap-two-node --out build/two-node
```

The [two-node runbook](../docs/two-node.md) walks through the rest, including how a
peer is granted exactly one role under exactly one plan.

## What to read next

- [The conversation is the computation](../rfcs/0002-conversation-is-the-computation.md)
  is the core idea in one document.
- [Plans and identity](../docs/plan.md) explains what the compiler produces.
- [Benchmark](../docs/benchmark.md) says where Colloq is slower than hand-written code,
  and by how much.
