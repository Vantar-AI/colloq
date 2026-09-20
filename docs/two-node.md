# Two-node Colloq/Iroh runbook

This is the first end-to-end path in which the two Colloq roles are independent operating-system
processes with persistent identities and local authorization policy. It is intended for a laptop,
two routable data-center hosts, or a Miren UDP node port. It is not yet a production key-management
system.

## Trust artifacts

Colloq deliberately separates three files:

| Artifact | Contains | Distribution |
| --- | --- | --- |
| `*.colloqnode.json` | Ed25519 private key and derived Iroh Endpoint ID | Local secret; mode `0600` on Unix |
| `*.colloqendpoint.json` | Public Endpoint ID and direct socket addresses | Give to connecting peers |
| `*.authorization.json` | Exact Endpoint ID → role → Colloq Plan grants | Local policy |

There are no wildcard grants in v0. A changed conversation produces a changed plan identity and
therefore needs a new explicit authorization.

## One-machine two-process acceptance run

The repository runs this complete acceptance path on Linux and macOS in CI. Locally:

```bash
cargo build --locked
scripts/two-node-smoke.sh
```

The harness checks a successful Generate session, independently edited draft convergence and
promotion, private identity file permissions, and fail-closed rejection of an unknown endpoint.

Create two node identities and reciprocal policy for both the Generate workload and the
`colloq.draft-sync` control conversation:

```bash
cargo run -- bootstrap-two-node --out build/two-node
```

Start the server in one terminal. It writes a public ticket before accepting one connection:

```bash
cargo run -- serve-iroh \
  --listen 127.0.0.1:7880 \
  --ticket-out build/two-node/server.colloqendpoint.json \
  --report-out build/two-node/server-report.json \
  --tokens 4
```

Connect from another terminal and verify the independently recorded outcomes:

```bash
cargo run -- connect-iroh \
  --server build/two-node/server.colloqendpoint.json \
  --report-out build/two-node/client-report.json

cargo run -- verify-session \
  --client build/two-node/client-report.json \
  --server build/two-node/server-report.json
```

The verifier requires the same conversation identity, plan identity, semantic-trace identity,
tokens, success result, and terminal outcome.

## Two physical servers

Generate each identity on the machine that will own it:

```bash
cargo run -- node-init --out /secure/colloq/server.colloqnode.json
cargo run -- node-init --out /secure/colloq/client.colloqnode.json
```

Exchange only the printed public Endpoint IDs. On the server, grant the client ID the `client`
role; on the client, grant the server ID the `server` role:

```bash
cargo run -- policy-allow --policy /secure/colloq/server.authorization.json \
  --peer CLIENT_ENDPOINT_ID --role client

cargo run -- policy-allow --policy /secure/colloq/client.authorization.json \
  --peer SERVER_ENDPOINT_ID --role server
```

The server binds all interfaces but advertises the address reachable by the client:

```bash
cargo run --release --locked -- serve-iroh \
  --identity /secure/colloq/server.colloqnode.json \
  --policy /secure/colloq/server.authorization.json \
  --listen 0.0.0.0:7880 \
  --advertise 10.20.0.12:7880 \
  --ticket-out /tmp/server.colloqendpoint.json
```

Copy only `/tmp/server.colloqendpoint.json` to the client, then run `connect-iroh` with the client
identity and policy. Allow UDP on the selected port. Iroh authenticates both Endpoint IDs; Colloq then
binds the exact roles, compiled plan, wire encoding, and TLS exporter before frame zero.

## Authenticated collaborative draft sync

Draft synchronization is itself a projected Colloq conversation. The peers exchange incremental
Automerge sync messages as typed `sync` transitions; completion is an explicit `done` choice.

```bash
# Create the same initial draft on both hosts, then edit independently.
cargo run -- draft-create examples/generate.colloqconv.json \
  --out build/two-node/server.colloqdraft

# Server
cargo run -- draft-serve-iroh \
  --draft build/two-node/server.colloqdraft \
  --listen 127.0.0.1:7881

# Client
cargo run -- draft-connect-iroh \
  --draft build/two-node/client.colloqdraft \
  --server build/two-node/draft-server.colloqendpoint.json
```

Both draft files are saved after synchronization. Automerge convergence does not bypass Colloq's
promotion gate: unresolved meaning conflicts are still rejected, then the materialized graph is
strictly deserialized, validated, and compiled.

## Miren path

Miren v0.5+ supports UDP node ports for non-HTTP services. Generate an Iroh-specific manifest:

```bash
cargo run -- emit-miren examples/generate.colloqconv.json \
  --transport iroh --port 7880
```

The manifest exposes UDP `7880` and declares three required values:

- `COLLOQ_NODE_IDENTITY_JSON` — sensitive node identity JSON;
- `COLLOQ_AUTHORIZATION_JSON` — sensitive exact authorization policy JSON;
- `COLLOQ_ADVERTISE_ADDRESS` — the Miren host or overlay socket address peers can reach.

Set those through Miren's environment management and run `miren deploy`. A connecting node receives
the corresponding public ticket out of band, or supplies it through `COLLOQ_SERVER_TICKET_JSON`.
Miren owns image placement, restart, and UDP forwarding; Colloq/Iroh owns peer authentication and the
language session. See the official [Miren app.toml reference](https://miren.md/app-toml) and
[non-HTTP traffic routing](https://miren.md/traffic-routing).

The repository does not contain node secrets. Deployment requires a configured Miren CLI/cluster
and operator-provided identities; a source checkout alone cannot safely deploy an authenticated
node.

## Remaining security work

v0 is fail-closed for an unknown identity, wrong role, wrong plan, wrong encoding, and replaying a
preface on another TLS connection. Rotation and revocation distribution, session nonces, durable
anti-replay state, protected hardware keys, long-running multiplexed sessions, and workload-identity
attestation remain future work.
