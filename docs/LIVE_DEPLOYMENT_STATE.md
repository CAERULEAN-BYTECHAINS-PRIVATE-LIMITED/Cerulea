# cbc-pramaan.cerulea.io — live deployment state

Updated 2026-09-16.

## What is live right now

`https://cbc-pramaan.cerulea.io` serves the CBC-PRAMAAN web app (Vercel) and answers real,
finalized, on-chain compliance decisions from a Cerulea validator hosted on Railway.

```
Vercel (cbc-pramaan.cerulea.io)  ->  wss://cbc-alice-production.up.railway.app  ->  Railway service cbc-alice
   web app + API routes                public WebSocket RPC                       solo --dev validator, volume at /data2
```

Verified on 2026-09-16 against the live domain: `/api/chain/status` connected, 21 ministries
and 76 HSN mappings seeded and read back, a GREEN bid, a RED bid and a YELLOW bid with a
cross-tender consistency flag, each returning a tx hash and finalized block number in
under 3 s.

## Railway (project CBC-PRAMAAN, environment production)

- Service `cbc-alice` (id 1bc99a92-3ab9-4b82-a7bf-ea7607be4f2a) is the only node that
  matters. It runs the **solo** configuration via a service-level start command override:

  ```
  cerulea-node --base-path /data2 --dev --rpc-port 9944 --prometheus-port 9615 \
    --unsafe-rpc-external --rpc-cors all --state-pruning archive --blocks-pruning archive --name Solo
  ```

  The override bypasses `docker-entrypoint.sh`; `NODE_ROLE` is ignored while it is set.
- Volume `cbc-alice-volume-2` is mounted at **/data2** (not /data). The original /data
  volume filled its 5 GB in 48 days under archive pruning and was deleted on 2026-09-16.
  At the same rate the new one fills around **early November 2026**: grow it in the
  dashboard (Volume settings) or relax `--state-pruning` before then.
- `cbc-bob`, `cbc-charlie` and the Railway `CBC-PRAMAAN` web service are not part of the
  live path. Their last builds failed, and the web service still runs a July deployment.

## Rebuilding the image on Railway is unreliable

Rust builds of this repo on Railway's builder die silently (no error line, `BUILD_IMAGE`
failure) roughly 40 minutes in, at the crate after `quanta`. Two of four attempts in July
and one of one on 2026-09-16 failed this way. Any change that forces a rebuild (volume
mount changes do) can therefore leave the service with no running deployment.

Recovery that works in ~2 minutes: redeploy a previously successful deployment's image
via the GraphQL API (the dashboard's deployment history "Redeploy" does the same). The
July image is deployment `f0bc8335-c3a1-485c-81e9-7a90d9b06799`:

```graphql
mutation { deploymentRedeploy(id: "f0bc8335-c3a1-485c-81e9-7a90d9b06799", usePreviousImageTag: true) { id status } }
```

The Railway CLI's session token lives at `~/.railway/config.json` (`user.accessToken`).

## The one Vercel env var

`CHAIN_WS_ENDPOINT` (Production) = `wss://cbc-alice-production.up.railway.app`.
A new value only takes effect on a new deployment:

```bash
cd apps/web
vercel env rm CHAIN_WS_ENDPOINT production --yes
echo "wss://<host>" | vercel env add CHAIN_WS_ENDPOINT production
vercel redeploy <current production deployment url>
```

## After any chain reset (empty volume)

```bash
export NODE_PATH="$PWD/apps/web/node_modules"
CHAIN_WS_ENDPOINT=wss://cbc-alice-production.up.railway.app npx tsx scripts/bootstrap/fund-personas.ts
CERULEA_WS_ENDPOINT=wss://cbc-alice-production.up.railway.app npx tsx scripts/seed-ministries/seed.ts
```

Run them in that order and not concurrently: both sign with //Alice.

## Retired

The interim WSL-plus-ngrok host described in earlier revisions is gone: the WSL distro
on that machine is corrupted and the ngrok URL is dead. Do not point Vercel back at it.
