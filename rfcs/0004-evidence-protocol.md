# RFC-0004: Colloq evidence protocol

- **Status:** Experimental
- **Date:** 2026-08-09
- **Scope:** Research validation, benchmarks, and public result artifacts

## Summary

Colloq claims that one typed global conversation can project into compatible distributed endpoint
programs while preserving inspectable semantics across transports and deployments. This RFC defines
how that claim is tested. It pre-registers workloads, baselines, measurements, environments,
acceptance gates, and reporting rules before the physical-node experiments are run.

The protocol treats a failed gate as a valid research result. It forbids turning a local
microbenchmark into a general performance claim and requires enough raw context for another person
to reproduce or challenge every published result.

## Motivation

The current prototype has evidence for validation, projection, plan identity, compact wire
encoding, four transports, typed failures, persistent Iroh identities, exact authorization, and
authenticated Automerge synchronization. Its current performance report is deliberately narrower:
an in-process request/token/done exchange over channels and JSON.

That is insufficient to establish any of the following:

- reliability across physical hosts;
- latency or throughput relative to a conventional RPC stack;
- behavior under adverse network conditions;
- operational recovery and identity lifecycle;
- value on a representative distributed-AI workload;
- reproducibility outside the original development machine.

Without a protocol fixed in advance, it would be too easy to select a favorable machine, workload,
sample window, baseline, or percentile after seeing the output.

## Non-goals

This RFC does not declare Colloq production-ready, select a final wire encoding, promise that Colloq is
faster than conventional RPC, or define production service-level objectives. It does not make
GitHub-hosted CI a performance laboratory. It defines what evidence must exist before narrower
claims may be made.

## Hypotheses

### H1 — Semantic agreement

Independently projected endpoints running the same accepted plan produce the same ordered semantic
trace and terminal outcome in a clean network.

### H2 — Fail-closed authority

An unknown endpoint, wrong role, wrong plan, wrong conversation, wrong wire encoding, or copied
channel binding cannot advance an Colloq conversation past frame zero.

### H3 — Draft convergence without semantic bypass

Conflict-free concurrent Automerge edits synchronize to the same heads and may pass Colloq promotion.
Conflicting meaning converges as document history but is rejected by the Colloq promotion gate.

### H4 — Bounded reference overhead

For an equivalent workload and transport, a warm compact Colloq session can approach the conventional
implementation without omitting Colloq's state, sequence, identity, or authorization checks.

### H5 — Defined failure observations

Network faults produce a declared local Colloq outcome or a bounded test failure; they do not silently
advance, deadlock indefinitely, or fabricate agreement unavailable to either endpoint.

### H6 — Representative AI value

On a distributed dynamic-batching workload, Colloq preserves comparable execution performance while
making protocol state, cancellation, deadlines, placement, and authority more explicit and
machine-checkable than an equivalent conventional implementation.

## Evidence tiers

Results must identify one of four tiers. A higher tier does not replace lower-tier checks.

| Tier | Environment | Purpose | Permitted claim |
| --- | --- | --- | --- |
| `ci` | Shared GitHub-hosted runner | Deterministic correctness and artifact validity | The checked revision passes its conformance suite |
| `local` | One identified development host | Microbenchmarks and profiling | A narrow implementation cost on this host |
| `physical` | Two dedicated, routable Linux hosts | Network behavior and controlled comparisons | A two-host result for the recorded hardware and topology |
| `miren` | Two declared Miren placements | Deployment and operational behavior | The same plan runs through the recorded Miren configuration |

Performance headlines require `physical` evidence. GitHub-hosted timing may diagnose gross
regressions but must not be used as the published latency baseline.

## Workloads

### W1 — Generate control conversation

The existing prompt/token/continue-or-cancel/done graph. It isolates control-plane state transitions,
streaming, cancellation, plan binding, and transport setup.

Required parameter sets:

- 1, 3, 32, and 256 tokens;
- prompt sizes of 32 B, 1 KiB, and 64 KiB;
- new connection per session and reused process with a new session where supported;
- success and cancellation after the first and midpoint tokens.

### W2 — Collaborative draft synchronization

Two drafts from the same base receive disjoint edits, synchronize, compare heads, and promote. A
second case makes conflicting edits at the same semantic path and must fail promotion.

Required parameter sets:

- 1, 10, and 100 disjoint scalar edits per peer;
- a small Generate graph and a generated graph with at least 1,000 states;
- clean, delayed, disconnected, and resumed exchanges once recovery exists.

### W3 — Dynamic inference batching

A gateway submits requests to a batcher that selects one of two deterministic inference workers,
streams results, enforces deadlines, propagates cancellation, and reports worker failure. The first
implementation uses deterministic mock compute. A real small model is a separate measurement so
model time does not hide coordination cost.

## Baselines

Every comparison must use the same payloads, topology, operation count, success criteria, and
connection lifecycle.

1. A hand-written Rust state machine over the same serialization and channel or socket boundary.
2. Raw Iroh using the same connection and payload framing without Colloq projection or checks.
3. Colloq reference wire over Iroh.
4. Colloq compact wire over Iroh.
5. A conventional `tonic` gRPC implementation for W1 and W3.

Comparisons must state which semantic guarantees each baseline omits. Removing identity, ordering,
cancellation, or validation from a baseline makes it an isolation measurement, not an equivalent
replacement.

## Measurements

### Correctness and security

- sessions attempted, established, completed, cancelled, and failed;
- semantic trace mismatches;
- plan, role, sequence, and state mismatches;
- unauthorized acceptances;
- draft head mismatches and promotion outcomes;
- deadlocks or operations exceeding the declared bound.

### Performance

- connection and Colloq-preface latency;
- cold and warm session latency;
- per-transition p50, p95, and p99 latency;
- end-to-end p50, p95, and p99 latency;
- sessions and messages per second;
- application bytes and observed transport bytes;
- user and system CPU time;
- peak resident memory and allocations where available;
- network baseline RTT and throughput;
- recovery time after a defined fault.

### Engineering outcome

- workload-specific source lines, excluding generated files and tests;
- number of protocol states represented in executable code;
- invalid fixtures rejected before deployment;
- changes needed to add cancellation, a deadline, or a worker role;
- structured diagnostics available to a machine author.

These measurements are descriptive. Source lines alone are not a quality metric.

## Statistical protocol

- Warm up before collecting timed samples.
- Use at least 30 independent runs for a published physical-host comparison.
- Use at least 10,000 measured transitions per configuration unless an RFC amendment explains why
  the operation is too expensive.
- Rotate implementation order between runs.
- Report every sample or a lossless raw sample artifact, not only aggregates.
- Report median, p95, p99, minimum, maximum, arithmetic mean, and a 95% bootstrap confidence
  interval for the median.
- Pin processes to recorded CPU sets where possible and record power/frequency policy.
- Run physical comparisons on otherwise idle dedicated hosts.
- Repeat the accepted matrix on three separate days. A result differing by more than 10% requires
  investigation and must not be summarized as one stable number.
- Do not remove outliers unless the protocol declared the removal rule before execution. Publish
  both raw and filtered data when a declared rule applies.

## Provisional acceptance gates

These thresholds are deliberately strict enough to falsify the current implementation.

| Gate | Threshold |
| --- | --- |
| Clean local agreement | 10,000 sessions, zero semantic or outcome mismatches |
| Clean physical agreement | 1,000 sessions, zero semantic or outcome mismatches |
| Authorization | Zero unauthorized acceptances in the complete negative matrix |
| Draft convergence | Zero head mismatches for conflict-free cases; every meaning conflict rejected at promotion |
| Compact local overhead | Warm whole-exchange median no more than 1.25× the equivalent hand-written baseline |
| Physical network overhead | p95 end-to-end latency no more than 15% above the equivalent raw transport for W1 |
| Reproducibility | Three-day medians within 10%, or an explicit inconclusive result |
| Fault behavior | No silent advancement or unbounded hang; declared local outcome within the workload bound |

The checked-in local compact result is 1.51× its hand-written baseline and therefore currently
fails the 1.25× gate. The threshold remains fixed until evidence justifies an RFC amendment.

## Fault and adversarial matrix

The harness must eventually cover:

- unknown but cryptographically valid client identity;
- valid identity granted the wrong role;
- valid identity granted a different plan;
- stale or modified endpoint ticket;
- copied preface on another TLS connection;
- replayed application session once nonces exist;
- 10, 50, and 100 ms added latency;
- 0.1%, 1%, and 5% packet loss;
- jitter and reordering;
- disconnect before frame zero, mid-stream, and during close;
- server restart and identity rotation;
- concurrent draft meaning conflict.

Linux network impairment should use a recorded `tc netem` configuration. The untouched network
baseline must run before and after the impaired matrix.

## Evidence artifact

Every run emits one document conforming to `spec/colloq-evidence-v0.schema.json`. It records:

- repository and exact commit;
- dirty-worktree status;
- start and finish timestamps;
- evidence tier and environment details;
- protocol revision, workload, transport, wire form, and parameters;
- conversation, plan, and public endpoint identities;
- correctness outcome and measurement aggregates;
- paths and SHA-256 digests of raw data and endpoint reports;
- explicit `passed`, `failed`, or `inconclusive` status;
- free-form limitations and notes.

Private keys, authorization secrets, tokens, host credentials, and environment-variable contents
must never enter an evidence artifact.

Suggested repository layout:

```text
benchmarks/
  protocols/
    two-node-v1.md
  results/
    2026-08-09-<short-commit>-<run-id>/
      evidence.json
      summary.md
      client-report.json
      server-report.json
      latency.csv
      plots/
```

Small raw artifacts may be committed. Large traces belong in a GitHub Release asset, referenced by
URL and digest from the committed evidence document.

## GitHub publication policy

1. The protocol or an RFC amendment merges before the measured implementation change.
2. Correctness CI is required on every pull request.
3. Physical performance runs use protected self-hosted infrastructure and never execute untrusted
   pull-request code.
4. Each result PR includes raw data, a generated summary, reproduction commands, and limitations.
5. Failed and inconclusive gates remain in history.
6. README and website claims link directly to the evidence document supporting them.
7. An evidence release is tagged only after its predefined gates are evaluated. A failed gate may
   still receive a research release if the failure is explicit in the name and summary.

## Initial execution order

1. Evidence schema, example, and CI validation.
2. Deterministic two-process smoke harness on Linux and macOS.
3. Local evidence runner and raw-sample format.
4. Two dedicated Linux hosts with direct Iroh and raw-Iroh baseline.
5. gRPC baseline and controlled fault matrix.
6. Dynamic-batching workload.
7. Three-day replication and public evidence release.

## Open questions

- Which protected runner provider will host the physical two-node laboratory?
- Should large raw samples use Git LFS or only GitHub Release assets?
- Which allocation profiler is sufficiently portable across Linux and macOS?
- What connection-pooling contract should define a reusable Colloq session?
- Which real model is small enough not to make the first W3 run operationally irreproducible?
