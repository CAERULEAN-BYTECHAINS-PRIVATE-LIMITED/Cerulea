# cbc-pramaan.cerulea.io — live deployment state

## What is live right now

`https://cbc-pramaan.cerulea.io` serves the CBC-PRAMAAN application and answers real,
finalized, on-chain compliance decisions. Verified end to end on the live domain:

- All ten routes return 200.
- `/api/chain/status` reports `connected: true`, runtime `cerulea-runtime v100`.
- A live bid returns a real GREEN/YELLOW/RED verdict with a transaction hash and block.
- **12 of 12 decision pathways pass against the live domain** (118 checks, exit 0).
- `/api/chain/rules` returns all 21 seeded ministries; `/api/chain/metrics` returns real
  counters (declarations, inconsistency flags, certificates, preference decisions).

## The topology, and its one caveat

```
Vercel (cbc-pramaan.cerulea.io)  ->  ngrok tunnel  ->  WSL solo validator (this machine)
   web app + API routes              wss://...ngrok        cerulea-node --dev, archive
```

- **Web app**: Vercel project `cbc-pramaan`, deployed from `apps/web` on branch
  `pramaan-poc`. The domain alias already points here.
- **Chain**: a single `--dev` validator (`NODE_ROLE=solo` equivalent) running in WSL2 on
  this machine, exposed over ngrok. Single-node so finality can never stall mid-demo.
- **The caveat, stated plainly**: this is an INTERIM host. It depends on this machine
  staying awake, the WSL node running, and the ngrok tunnel being up. A free ngrok URL
  changes if the tunnel restarts, which then needs the Vercel env var updated (below).
  The permanent home is Railway (see RAILWAY_DEPLOY.md); its build queue was stuck on a
  plan/resource limit that only the dashboard can clear.

## The one Vercel env var

`CHAIN_WS_ENDPOINT` (Production) = the current public chain endpoint.
Today: `wss://11e9-122-175-41-167.ngrok-free.app`.

If the ngrok URL changes, update it and redeploy:

```bash
cd apps/web
vercel env rm CHAIN_WS_ENDPOINT production --yes
echo "wss://<new-ngrok-host>" | vercel env add CHAIN_WS_ENDPOINT production
vercel --prod --yes
```

The app fails gracefully if the chain is unreachable — every route returns a clear
"could not connect" error within 5s rather than hanging, so a dropped tunnel degrades
the site to honest error messages, not a hang.

## Restarting the interim chain (if this machine reboots)

```bash
# 1. solo validator in WSL
wsl -d Ubuntu -u root -- bash -c '/root/build/Cerulea-Chain/target/release/cerulea-node \
  --dev --rpc-external --rpc-cors=all --rpc-port 9944 --base-path /tmp/solo \
  --unsafe-rpc-external --state-pruning archive --blocks-pruning archive &'

# 2. fund + seed (from apps/web, NODE_PATH set to its node_modules)
CHAIN_WS_ENDPOINT=ws://127.0.0.1:9944 npx tsx scripts/bootstrap/fund-personas.ts
CERULEA_WS_ENDPOINT=ws://127.0.0.1:9944 npx tsx scripts/seed-ministries/seed.ts

# 3. tunnel, then point Vercel at the new URL (see above)
ngrok http 9944
```

## Moving to Railway (permanent)

The blueprint and RAILWAY_DEPLOY.md are ready. Once the Railway build queue is unblocked
(a plan/billing matter in the Railway dashboard), the chain runs there with a stable
public URL and no dependency on this machine, and `CHAIN_WS_ENDPOINT` points at it
instead. Both `solo` (reliable single validator) and the three-validator quorum roles
are in the same image and switch by the `NODE_ROLE` variable with no rebuild.
