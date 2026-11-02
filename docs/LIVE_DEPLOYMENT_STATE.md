# cbc-pramaan.cerulea.io — live deployment state

Updated 2026-09-16 (evening).

## What is live right now

`https://cbc-pramaan.cerulea.io` serves the CBC-PRAMAAN application (repository
CAERULEAN-BYTECHAINS-PRIVATE-LIMITED/CBC-PRAMAAN, branch `main`). Every write is a
finalized extrinsic on the Cerulea validator hosted on Railway; the application keeps a
SQLite read model that it rebuilds from the chain at boot.

```
Vercel project cbc-pramaan-edge (holds the domain, instant TLS, vercel.json rewrite)
    -> https://cbc-pramaan-poc-production.up.railway.app   Railway service cbc-pramaan-poc (Next.js app, volume /app/data)
    -> wss://cbc-alice-production.up.railway.app           Railway service cbc-alice (solo Cerulea validator, volume /data)
```

- The domain sits on Vercel deliberately: Railway's certificate issuance for the custom
  domain stayed at "validating ownership" for over an hour despite propagated DNS, while
  Vercel issues instantly. The edge project is a one-file `vercel.json` in
  `CBC-PRAMAAN/edge/` that proxies every path to the Railway app (cookies and headers
  pass through). Deployment protection on that project must stay OFF (it was on by
  default and answered 302 to Vercel SSO).
- The former web app in this repository (`apps/web`, Vercel project `cbc-pramaan`) is no
  longer aliased to the domain and is not part of the live path.
- Access control: browsers get an access-code prompt; machines send `x-api-key`. Values
  live in the Railway service variables `PRAMAAN_DEMO_PASSWORD`, `PRAMAAN_API_KEYS`,
  `PRAMAAN_SESSION_SECRET` on `cbc-pramaan-poc` (and, historically, on Vercel `cbc-pramaan`).
- Application variables on `cbc-pramaan-poc`: `CHAIN_WS_ENDPOINT`, `PRAMAAN_LEDGER_SURI`
  (`//Alice`), `PRAMAAN_LEDGER_NAMESPACE` (`cbc-pramaan`), `PRAMAAN_LEDGER_START_BLOCK`
  (`7934`, the first block of the production data set). A wiped read model rebuilds from
  the chain automatically at boot; a read model from the pre-chain release is discarded.

## Railway (project CBC-PRAMAAN, environment production)

- Service `cbc-alice` (id 1bc99a92-3ab9-4b82-a7bf-ea7607be4f2a) is the only node that
  matters. It runs the **solo** configuration via a service-level start command override:

  ```
  cerulea-node --base-path /data2 --dev --rpc-port 9944 --prometheus-port 9615 \
    --unsafe-rpc-external --rpc-cors all --state-pruning archive --blocks-pruning archive --name Solo
  ```

  The override bypasses `docker-entrypoint.sh`; `NODE_ROLE` is ignored while it is set.
- Volume `cbc-alice-data` is mounted at **/data**, which is where the entrypoint keeps the
  database. The original volume filled its 5 GB in 48 days under archive pruning and was
  deleted on 2026-09-16. A service-level start-command override was tried and removed:
  Railway's redeploy-from-image path ignores it, so all runtime configuration lives in
  variables (`NODE_ROLE=solo`, `STATE_PRUNING`, `BLOCKS_PRUNING`).
  At the same rate the new one fills around **early November 2026**: grow it in the
  dashboard (Volume settings) or relax `--state-pruning` before then.
- `cbc-bob` and `cbc-charlie` exist but have no volume and no successful build; the
  Railway `CBC-PRAMAAN` web service was deleted on 2026-09-16 (the web app is on Vercel).

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

## Access control (added 2026-09-16)

Every `/api/trigger/*` route now requires a credential (apps/web/src/lib/auth.ts):

- Browsers: the consoles show an access-code prompt on load and `POST /api/session` sets
  a 12-hour HttpOnly session cookie. The code is `PRAMAAN_DEMO_PASSWORD` on Vercel.
- Machines (GeM integration, simulator scripts): send `x-api-key: <key>`; keys are the
  comma-separated `PRAMAAN_API_KEYS`. `PRAMAAN_SESSION_SECRET` signs the cookie.
- With none of the three set the routes are open and the server logs a warning once.
- Per-role signing keys: `PERSONA_SURI_<DPIIT|MINISTRYADMIN|PROCURINGENTITY|VENDOR|AUDITOR|CVC>`
  replace the well-known dev accounts; fund any override with fund-personas first.

Rotate a credential with `vercel env rm` / `vercel env add` and a redeploy.

## Node binary comes from GitHub Actions; Railway only assembles the image

`.github/workflows/node-image.yml` builds the Dockerfile on every push to `pramaan-poc`
or `develop` that touches the node, pushes the image to GHCR (private: the org policy
blocks public packages and Railway's plan cannot use registry credentials), and ALSO
publishes the `cerulea-node` binary as a public release asset on the rolling release
`node-<branch>` (immutable `cerulea-node-<sha>` assets alongside). `Dockerfile.release`
downloads that asset onto debian-slim, which is what the Railway service `cbc-alice`
now builds (source: this repo, branch `pramaan-poc`, `dockerfilePath = Dockerfile.release`,
about 35 seconds). A failed GitHub build never touches the running node; a Railway
build only fails if the release asset is missing.

Switched on 2026-09-16 at 12:30 UTC: the node came back on the new image in under a
minute at block #7032 with the volume intact. Service variables `NODE_ROLE=solo`,
`STATE_PRUNING=100000`, `BLOCKS_PRUNING=archive` are read by docker-entrypoint.sh.

`.github/workflows/uptime.yml` (default branch `develop`) checks the live domain and the
Railway RPC every 15 minutes and fails, with an email, when either is unreachable.

## Vercel `cbc-pramaan` (legacy web app) env var

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
