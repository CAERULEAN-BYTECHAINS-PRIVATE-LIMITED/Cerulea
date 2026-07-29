# Deploying CBC-PRAMAAN on Railway

Four services from **one** repo and **one** `Dockerfile`. Three validators plus the web
application. Every service deploys from branch `pramaan-poc`; pushing to that branch
redeploys them automatically.

The three validators are identical images. The only thing that differs between them is
`NODE_ROLE`, which `docker-entrypoint.sh` reads to decide which validator to start and
whether to dial a bootnode.

---

## Before you start: generate the network key

Alice's peer identity must be **stable**, because Bob and Charlie derive it to build the
bootnode address they dial. If Alice generates a random key on each boot they can never
find her, and the network degrades to a single validator with no quorum — which still
produces blocks, so it fails quietly rather than loudly. That is the trap.

Generate one:

```bash
cerulea-node key generate-node-key
```

One value, 64 hex characters, pasted into **all three** validator services. It is a P2P
identity key, not a validator signing key, but it still does not belong in git —
`keys/` is gitignored deliberately, which is why this arrives as an environment
variable instead.

---

## Service 1 — `cbc-alice` (bootnode, validator, public RPC)

| Setting | Value |
|---|---|
| Source | this repo, branch `pramaan-poc` |
| Root directory | *(repo root)* |
| Builder | Dockerfile |
| Volume | mount at `/data`, 10 GB+ |
| Public networking | enable — this is the RPC endpoint the web app uses |

Variables:

```
NODE_ROLE       = alice
ALICE_NODE_KEY  = <the 64-hex key>
PORT            = 9944
P2P_PORT        = 30333
PROMETHEUS_PORT = 9615
RUST_LOG        = info,cerulea_consensus=info
```

Once deployed, note the public domain Railway assigns. The web app connects to it as
`wss://<that-domain>`.

---

## Services 2 and 3 — `cbc-bob`, `cbc-charlie` (validators, private)

Same repo, same Dockerfile, same volume setup (mount `/data`). **Do not** enable public
networking on these — nothing should reach them from outside.

`cbc-bob`:

```
NODE_ROLE       = bob
ALICE_NODE_KEY  = <the SAME 64-hex key as Alice>
ALICE_HOST      = cbc-alice.railway.internal
PORT            = 9945
P2P_PORT        = 30334
PROMETHEUS_PORT = 9616
RUST_LOG        = info,cerulea_consensus=info
```

`cbc-charlie`:

```
NODE_ROLE       = charlie
ALICE_NODE_KEY  = <the SAME 64-hex key as Alice>
ALICE_HOST      = cbc-alice.railway.internal
PORT            = 9946
P2P_PORT        = 30335
PROMETHEUS_PORT = 9617
RUST_LOG        = info,cerulea_consensus=info
```

`ALICE_HOST` **must** carry the `.railway.internal` suffix. Railway's private network
resolves only those names, and it is IPv6-only — which is why the entrypoint binds
`/ip6/::/tcp/$P2P_PORT` alongside the IPv4 address. A node listening on IPv4 alone is
invisible to its peers on Railway even though it looks perfectly healthy in its own logs.

---

## Service 4 — `cbc-pramaan-web` (Next.js)

| Setting | Value |
|---|---|
| Source | this repo, branch `pramaan-poc` |
| Root directory | `apps/web` |
| Build command | `npm ci && npm run build` |
| Start command | `npm run start` |
| Public networking | enable |

Variables:

```
NODE_VERSION       = 22
NODE_ENV           = production
CHAIN_WS_ENDPOINT  = ws://cbc-alice.railway.internal:9944
```

Use the **internal** address, not the public `wss://` one. It avoids TLS overhead and,
more importantly, avoids any public-proxy idle timeout on the long-lived
`subscribeFinalizedHeads` subscription that the finality wait holds open for the whole
request.

*(If the web app is hosted on Vercel instead, it cannot reach Railway's private network
and must use the public `wss://<alice-domain>` address.)*

---

## After the first deploy

The chain starts **empty**. Two scripts populate it, in this order:

```bash
CHAIN_WS_ENDPOINT=wss://<alice-domain> npx tsx scripts/bootstrap/fund-personas.ts
```

```bash
CERULEA_WS_ENDPOINT=wss://<alice-domain> npx tsx scripts/seed-ministries/seed.ts
```

The first funds the demo persona accounts, without which every extrinsic they sign is
rejected for want of fees before any pallet logic runs. The second writes the 21 nodal
ministry rule sets and the HSN map, then reads every one back and diffs it against the
source JSON, failing loudly on any mismatch.

Optionally, drive realistic traffic through the six trigger points so the dashboard and
explorer have genuine finalized decisions to show:

```bash
npx tsx scripts/gem-simulator/volume.ts --count=120 --api=https://<web-domain>
```

---

## Checking it actually worked

The failure mode to look for is **one healthy validator instead of three**, because
that still produces blocks and still serves RPC. In `cbc-alice`'s logs:

- `💤 Idle (2 peers)` — 2, not 0. Zero peers means Bob and Charlie never found her:
  check `ALICE_NODE_KEY` is byte-identical across all three, and that `ALICE_HOST` ends
  in `.railway.internal`.
- `finalized #N` should track `best: #N`, not lag or sit at `#0`.
- `Selected 2 votes with accumulated weight ... (threshold: ...)` — this line is the
  2-of-3 quorum actually being reached, and is the thing worth screenshotting.
- `long-range attack` must appear **zero** times. If it appears, a validator finalized
  its own block before peering and has permanently forked; redeploy all three together
  so they start from genesis at the same time.
