# CBC Chain — Docker Setup Guide

This document covers everything you need to run, develop, and maintain the CBC Chain
3-validator network using Docker locally, and how it maps to the Render deployment.

---

## Prerequisites

- Docker >= 20.x
- docker-compose >= 1.29
- ~25 GB free disk space (build cache + chain data)
- Ports 9944, 9945, 9946, 30333, 30334, 30335, 9615, 9616, 9617 free on your machine

---

## Project Structure (Docker-relevant files)

```
.
├── Dockerfile              # 3-stage build: deps cache → builder → runtime image
├── docker-entrypoint.sh    # Entrypoint: handles key setup and node startup per role
├── docker-compose.yml      # Local 3-node network (alice + bob + charlie)
├── keys/
│   └── alice/
│       └── secret_ed25519  # Alice's fixed network key — baked into image for deterministic peer-id
└── .dockerignore           # Excludes target/, logs, docs from build context
```

---

## Node Layout

| Node    | Role               | RPC Port | P2P Port | Prometheus |
|---------|--------------------|----------|----------|------------|
| Alice   | Bootnode + RPC     | 9944     | 30333    | 9615       |
| Bob     | Validator          | 9945     | 30334    | 9616       |
| Charlie | Validator          | 9946     | 30335    | 9617       |

Bob and Charlie connect to Alice as their bootnode. Alice's peer-id is deterministic
because her network key (`keys/alice/secret_ed25519`) is baked into the image at build
time. Bob and Charlie auto-generate their own keys on first boot.

---

## Quick Start

### 1. Start all three nodes

```bash
docker-compose up --build
```

First run compiles the entire Rust codebase — expect ~20 minutes.
Subsequent runs with source changes take ~2–5 minutes (see Caching section).

### 2. Run in the background (detached)

```bash
docker-compose up --build -d
```

### 3. Verify nodes are running

```bash
docker ps
```

Expected output:
```
NAMES         STATUS        PORTS
cbc-charlie   Up X minutes  0.0.0.0:9946->9946/tcp, 0.0.0.0:30335->30335/tcp ...
cbc-bob       Up X minutes  0.0.0.0:9945->9945/tcp, 0.0.0.0:30334->30334/tcp ...
cbc-alice     Up X minutes  0.0.0.0:9944->9944/tcp, 0.0.0.0:30333->30333/tcp ...
```

### 4. Confirm nodes are connected and in sync

Check Alice's peer count (should be 2):
```bash
curl -s -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"system_health","params":[]}' \
  http://localhost:9944
```

Expected: `{"result":{"peers":2,"isSyncing":false,...}}`

Check all three nodes are on the same block:
```bash
for port in 9944 9945 9946; do
  echo -n "Port $port — block: "
  curl -s -H "Content-Type: application/json" \
    -d '{"id":1,"jsonrpc":"2.0","method":"chain_getBlock","params":[]}' \
    http://localhost:$port | python3 -c \
    "import sys,json; d=json.load(sys.stdin); print(int(d['result']['block']['header']['number'],16))"
done
```

All three should print the same block number.

---

## Viewing Logs

Tail all nodes at once:
```bash
docker-compose logs -f
```

Tail a specific node:
```bash
docker-compose logs -f alice
docker-compose logs -f bob
docker-compose logs -f charlie
```

View last N lines:
```bash
docker-compose logs --tail=100 alice
```

---

## Stopping the Network

Stop containers but keep chain data (volumes preserved):
```bash
docker-compose down
```

Stop and wipe all chain data (full reset):
```bash
docker-compose down -v
```

---

## Rebuilding After Code Changes

### Source file changes (fast — ~2–5 min)

If you edit any `.rs` file under `cbc-node/`, `cbc-runtime/`, or `cbc-pallets/`:

```bash
docker-compose down
docker-compose up --build
```

Docker reuses the dependency cache (Stage 1) and only recompiles your changed crates.

### Dependency changes (slow — ~20 min)

If you edit `Cargo.toml` or `Cargo.lock` (add/remove/update a crate):

```bash
docker-compose down
docker-compose up --build
```

Stage 1 cache is invalidated and all dependencies recompile from scratch.

### Script/entrypoint changes (instant)

If you only edit `docker-entrypoint.sh` or files in `scripts/`:

```bash
docker-compose down
docker-compose up --build
```

Only Stage 3 (the tiny runtime image) rebuilds — takes seconds.

### How the cache works

The Dockerfile is split into 3 stages deliberately:

```
Stage 1 (deps)     — copies only Cargo.toml/Cargo.lock + stubs, runs cargo fetch
                     Cached until manifests change. ~2 GB layer, built once.

Stage 2 (builder)  — copies real source over stubs, runs cargo build --release
                     Only recompiles crates whose source actually changed.

Stage 3 (runtime)  — copies the binary into a minimal debian:bookworm-slim image
                     Rebuilds in seconds, always.
```

---

## Environment Variables

All nodes accept these environment variables (set in `docker-compose.yml` or passed via `-e`):

| Variable           | Default     | Description                                      |
|--------------------|-------------|--------------------------------------------------|
| `NODE_ROLE`        | `alice`     | Which validator to start: `alice`, `bob`, `charlie` |
| `PORT`             | `9944`      | RPC port (Render uses this; maps to `RPC_PORT`)  |
| `P2P_PORT`         | `30333`     | P2P listening port                               |
| `PROMETHEUS_PORT`  | `9615`      | Prometheus metrics port                          |
| `ALICE_HOST`       | `cbc-alice` | Hostname for Alice (used by Bob/Charlie to build bootnode address) |
| `RUST_LOG`         | —           | Log filter, e.g. `info,cbc_consensus=debug`      |

---

## Connecting a Frontend / Wallet

Alice's RPC is exposed on port 9944 and accepts both HTTP and WebSocket:

```
HTTP RPC  : http://localhost:9944
WS RPC    : ws://localhost:9944
```

Use this in Polkadot.js Apps:
1. Open https://polkadot.js.org/apps
2. Click the network selector (top-left)
3. Choose "Development" → "Custom"
4. Enter `ws://localhost:9944`
5. Click "Switch"

---

## Chain Data Persistence

Chain data is stored in named Docker volumes:

| Volume              | Mounted at | Node    |
|---------------------|------------|---------|
| `cbc-chain_alice-data`   | `/data`    | Alice   |
| `cbc-chain_bob-data`     | `/data`    | Bob     |
| `cbc-chain_charlie-data` | `/data`    | Charlie |

Volumes survive `docker-compose down` but are wiped by `docker-compose down -v`.

To inspect a volume:
```bash
docker volume inspect cbc-chain_alice-data
```

---

## Alice's Network Key

Alice's P2P identity is fixed via `keys/alice/secret_ed25519`. This file is committed
to the repo and baked into the Docker image so her peer-id never changes between
rebuilds or deployments.

Bob and Charlie derive Alice's peer-id from this baked key at startup — no manual
configuration needed.

If you ever need to regenerate Alice's key (this will change her peer-id and break
existing bootnodes pointing to her):
```bash
./target/release/cbc-node key generate-node-key --file keys/alice/secret_ed25519
```

Then rebuild the image.

---

## Resetting the Network

Full clean slate — wipes chain data, removes containers, images, and volumes:

```bash
docker-compose down -v
docker system prune -af --volumes
docker-compose up --build
```

Partial reset — keep the image cache but wipe chain data only:
```bash
docker-compose down -v
docker-compose up
```

---

## Render Deployment

The same `Dockerfile` and `docker-entrypoint.sh` are used on Render. The
`render.yaml` defines three services:

- `cbc-alice` — type `web` (public HTTPS/WSS endpoint on port 9944)
- `cbc-bob` — type `pserv` (private, validator only)
- `cbc-charlie` — type `pserv` (private, validator only)

Bob and Charlie use `ALICE_HOST: cbc-alice` which resolves to Alice's internal
Render hostname. The bootnode address is built automatically in the entrypoint.

To deploy: push to your connected Git branch. Render picks up the changes and
rebuilds automatically.

---

## Troubleshooting

**Nodes not connecting (peers: 0)**

Alice's key may not have been baked correctly. Check:
```bash
docker-compose logs alice | grep "peer-id\|network key\|Installing"
```

**Port already in use**

Another process is using 9944/9945/9946. Find and kill it:
```bash
sudo lsof -i :9944
```

**Build fails with "not enough disk space"**

Check available space:
```bash
df -h
```
Free up Docker cache:
```bash
docker system prune -af
```

**Want to rebuild only one service**

```bash
docker-compose up --build alice
```

**Attach to a running container**

```bash
docker exec -it cbc-alice bash
```
