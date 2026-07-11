# Cerulea Studio: Deep Audit of the Studio Monorepo

Scope: `platform/` in this repository, a pnpm + Turbo workspace whose only app is
`platform/apps/frontend` (a Next.js App Router project, roughly 27k lines of
TypeScript and React under `src/`). This document covers the route and component
map, what the step based builder can actually produce today, the gap to the
target no code product, the data model, persistence, auth and session security,
where the AI assistant is wired, and a prioritized build list.

This audit changed no code. It is a document only.

## 0. Executive verdict

The Studio is a polished, unusually complete **specification capture wizard** with
a **simulated deployment finale**. It has a large, well authored catalog (225
modules, 100 templates, a full entity catalog with typed fields), a visual module
graph, an entity/field designer, an economics form, an integrations form, and a
drag and drop UI builder. It also ships real Solidity and TypeScript compile API
routes.

What it does not have is any wire connecting capture to output. No step compiles,
generates, exports, or deploys anything. The final "Review and Deploy" step
(`step6.tsx`) is a client side animation driven by `setInterval` and
`Math.random()` with hardcoded log strings; it makes zero network calls. The
real compile routes exist but no UI code calls them. The result is an impressive
front end that captures intent but produces no chain, no dapp, no code, and no
running artifact.

Secondary findings that matter as much as the missing output pipeline:

- Persistence is fragmented and partly broken. There are four database
  bootstraps across two engines (SQLite and Postgres) and three divergent
  schemas. The Studio autosave never reaches the server. Two builder steps save
  to API routes that do not exist. Two of the wizard steps do not persist at all.
- Auth is enforced only by `middleware.ts`. The client side guard `AuthGate.tsx`
  is dead code and depends on an `/api/auth/me` route that does not exist. The
  password reset flow can never validate a token (identifier prefix mismatch).
  A seeded test account is effectively an admin backdoor.
- Large portions of the dashboard and every admin page render hardcoded mock
  data.

Sections 2 and 3 name the specific missing capabilities.

---

## 1. Route and component map

### 1.1 App shell and hosting model

- Root layout: `src/app/layout.tsx`. Wraps everything in `Providers`
  (NextAuth `SessionProvider` plus MUI theme, forced light on first paint) and
  `StudioProvider`, then renders `Background`, `NavBar`, `{children}`, and the
  global `Assistant` FAB. There is no auth logic in the layout.
- Single Next.js app, three logical surfaces selected by host, not by path:
  - Main host serves the marketing landing or the dashboard.
  - `studio.<host>` (or `?studio=1`) rewrites `/` to the full Studio shell.
    See `src/app/page.tsx:13-28` and `src/middleware.ts:121-125`.
  - `control.<host>` rewrites `/*` to `/admin/*`. See `middleware.ts:78-102`.
- Auth and gating live entirely in `src/middleware.ts`. `PUBLIC_PATHS` is
  `['/auth/', '/pricing', '/api/', '/_next/', '/favicon.ico']`
  (`middleware.ts:6-12`). Everything else requires a valid NextAuth JWT.

### 1.2 The Studio builder is mounted at `/`, not at a `/studio` route

`src/app/page.tsx` renders the marketing `Landing` (`components/landing/Hero.tsx`)
on the main host and mounts `StudioEntry` when the host starts with `studio.` or
when `?studio=1` is present. `StudioEntry` renders `StudioShell`
(`src/app/_studio/shell/StudioShell.tsx`), which drives the stepper. There is no
dedicated `/studio` route folder.

### 1.3 Page routes (real vs stub)

Verdict key: Real = fetches live data or performs a real action; Half = derived
or randomized from a real project list; Stub = hardcoded arrays only; Missing =
linked in navigation but no file exists.

Public / top level:

| Route | File | Verdict | Notes |
|---|---|---|---|
| `/` (landing) | `app/page.tsx` + `components/landing/Hero.tsx` | Real, but gated | Static marketing splash. `/` is not public, so anonymous visitors are redirected to `/auth/login`. On the studio host, `/` mounts the builder. |
| `/pricing` | `app/pricing/page.tsx` + `components/pricing/PricingPage.tsx` | Real | Plans hardcoded; checkout is a live POST to `/api/stripe/checkout`. |
| `/pricing/success` | `app/pricing/success/page.tsx` | Real | Calls `session.update()` to refresh the plan claim, then redirects. |
| `/settings/profile` | `app/settings/profile/page.tsx` | Broken dependency | GET/PUT `/api/auth/me`, which does not exist. |
| `/app/[slug]` | `app/app/[slug]/page.tsx` | Broken in prod | Runtime renderer that fetches config from a hardcoded `http://localhost:4000/deployed-apps/${slug}`. Buttons call `alert()`. |
| `/auth/login`, `/auth/register`, `/auth/forgot-password`, `/auth/reset-password` | `app/auth/*` | Real | NextAuth credentials plus custom register/reset routes (reset flow is broken, see 4.9). |

Dashboard (`/dashboard/*`, gated by middleware; `dashboard/layout.tsx` is the
sidebar shell):

| Route | Verdict | Notes |
|---|---|---|
| `/dashboard` | Mixed | Project list and delete are live (`/api/projects`). KPI tiles partly fake: "Smart Contracts" = `projects.length * 3`, "RPC Requests" hardcoded `12.4K`, Platform Status panel hardcoded all green. |
| `/dashboard/projects` | Real | Live list, delete, filters. |
| `/dashboard/networks` | Half | Test account sees 6 hardcoded networks; real users get networks derived from projects with randomized blockHeight/tps/health. |
| `/dashboard/nodes` | Half | Same pattern: hardcoded for test account, fabricated from projects otherwise. |
| `/dashboard/keys` | Stub | `STUB_API_KEYS` and `STUB_VALIDATOR_KEYS`; generate/revoke mutate local state; keys are `Math.random()` strings. |
| `/dashboard/governance` | Stub (live gate) | Checks `/api/projects` for a blockchain project, then renders stub proposals/multisig/history. |
| `/dashboard/contracts` | Derived | `components/SmartContractsScreen.tsx` shows contracts derived from Studio modules via `deriveSmartContracts()`; test account falls back to `getDemoContracts()`. Solidity shown is a stub string. No deployment. |
| `/dashboard/audit` | Stub | 10 hardcoded logs (a real `/api/dashboard/audit-logs` route exists but is not called here). |
| `/dashboard/state` | Stub | Hardcoded snapshots. |
| `/dashboard/integrations` | Stub | Hardcoded integrations with masked fake secrets. |
| `/dashboard/settings` | Stub + one live action | Fake usage meters and billing date; save flashes "Saved"; test account factory reset calls `/api/test-account/reset` (real). |

Admin (`control.` subdomain, rewritten to `/admin/*`; gated to
`test@cerulea.app` or `isTestAccount`, re-checked in `admin/layout.tsx`):

| Route | Verdict |
|---|---|
| `/admin`, `/admin/users`, `/admin/projects`, `/admin/blockchain`, `/admin/security`, `/admin/audit` | Stub (hardcoded arrays; action buttons inert). |
| `/admin/deployments`, `/admin/ai`, `/admin/billing`, `/admin/support`, `/admin/config` | Missing. Linked in the admin sidebar (`admin/layout.tsx:49-56`) but no page files exist, so they 404. |

### 1.4 API routes

Auth: `api/auth/[...nextauth]`, `register`, `forgot-password`, `reset-password`,
`force-signout`. Note the UI references `/api/auth/me` in three places
(`AuthGate.tsx:15`, `UserMenu.tsx:25`, `settings/profile/page.tsx:16,34`) but no
such route exists.

Studio data: `api/projects` (+ `[id]`, `[id]/draft`, `[id]/blueprint`,
`[id]/logic`, `[id]/schema`), `api/drafts/[[...slug]]`, `api/workspaces`,
`api/modules`, `api/templates`, plus static preset endpoints
(`api/presets/*`, `api/static/*`, `api/module-presets`).

AI: `api/ceruleai` (used by the Assistant), `api/ai/threads` and
`api/ai/threads/[id]/messages` (orphaned, no caller, see 5.3).

Compile: `api/compile/solidity`, `api/solc/compile`, `api/tools/solc/compile`,
`api/tools/ts/compile`, `api/scripts/compile`. All real, all uncalled by any UI
(see 2.6 and 5.4).

Commerce: `api/stripe/checkout`, `api/stripe/webhook`, `api/subscriptions`.

Dashboard: `api/dashboard/overview`, `api/dashboard/networks`,
`api/dashboard/audit-logs`, `api/blocks`, `api/dbcheck`, `api/test-account/reset`.

Referenced but nonexistent (dead client calls): `/api/deployments` and `/api/apps`
in `lib/apiClient.ts:27-31`; `/api/projects/[id]/integrations` (step4);
`/api/projects/[id]/ui` (step5); `/api/drafts/upsert` shape mismatch (see 4.7).

### 1.5 Component map (highlights and redundancies)

- Studio shell: `_studio/StudioEntry.tsx`, `_studio/shell/StudioShell.tsx`,
  `_studio/shell/StepRegistry.tsx`, `StudioHeader.tsx`, `StudioFooter.tsx`.
- Steps: `components/studio/steps/step0..step6.tsx` (step5 commented out of the
  registry). Plus `backup of steps/step0.tsx`, `step1.tsx` (stale duplicates).
- Canvases: `studio/logic/LogicCanvas.tsx` (used by step2),
  `studio/logic/RelationshipCanvas.tsx` (type import only, never rendered),
  `studio/relationships/RelationshipCanvas.tsx` (orphaned),
  `studio/flow/FlowCanvasCompat.tsx` (orphaned),
  `studio/logic/Logic_old.tsx` and `relationships/relationship_old.tsx` (stale).
- Inspectors: `studio/common/PropertiesDrawer.tsx` (1033 lines, comments say
  "for future exporters/codegen", never wired), `studio/logic/PropertiesDrawer.tsx`.
- Assistant: `components/AI/Assistant.tsx`, mounted globally in the layout.
- Shared: `NavBar.tsx`, `UserMenu.tsx` (unused/legacy), `AuthGate.tsx` (dead),
  `Theme/Background.tsx`, `ui/GlassPanel.tsx`, `common/InfoTooltip.tsx` and
  `shared/InfoTooltip.tsx` (near duplicates), `common/AutosaveBadge.tsx`,
  `StepperNav.tsx`, `SmartContractsScreen.tsx`.

---

## 2. What the step based builder can produce today, precisely

The active workflow is defined in `StepRegistry.tsx`. Steps are keyed 0, 1, 2, 3,
4, and then id 5 loads `step6` (the step5 module is commented out). The visible
numbering in the step UIs is inconsistent (some say "of 6", some "of 7"). The
runtime sequence is:

1. step0: Project Foundation
2. step1: Application Blueprint (module graph)
3. step2: Data and Logic
4. step3: Economics
5. step4: Integrations
6. step6: Review and Deploy

`step5` (UI Builder) is built but unreachable.

### 2.1 step0: Project Foundation (`steps/step0.tsx`)

Captures: project type (`dapp` or `blockchain`), an optional legacy strategy for
blockchain projects (`none` / `connect` / `port`), a chosen template (from
`/api/templates`, backed by `data/templates.seed.json`, 100 templates), project
name, slug, workspace, description, and a small technical detail form:

- dApp: target network (`cerulea-testnet` / `cerulea-mainnet` / `ethereum`),
  token standards (erc20/721/1155), royalties percentage.
- Blockchain: consensus (PoA/PoS), region, initial validators, native token
  symbol and decimals, and a fee model (base gas, burn percent, validator share).

Produces: a real DB row. `onInitialize` POSTs to `/api/projects`
(`step0.tsx:298-328`) and stores `projectId`, `slug`, and `appMetadata` in
`StudioContext`. It also seeds `localStorage` keys used by later steps
(`cerulea.projectType`, `cerulea.templateModules`). This is the one step that
persists reliably to the server.

Note: template details captured here (`dappDetails`, `chainDetails`) are sent as
`details` in the create payload, but the `projects` table has no column for them,
so they are dropped server side (see 4).

### 2.2 step1: Application Blueprint (`steps/step1.tsx`)

Captures: a visual module graph. The user loads a module library from
`/api/modules` (backed by `data/modules.seed.json`, 225 modules: 125 dApp, 100
blockchain, every one carrying a `configSchema`). Modules are placed as ReactFlow
nodes; the user can add modules via a spotlight search, set a free text logical
group, fill a per module parameter form generated from `configSchema`, and draw
typed edges between modules (reads / writes / triggers / feeds / calls / pays /
custom). Dependency suggestions can be added with one click.

Produces: JSON. On change it writes `localStorage['cerulea.step1.graph']`
(`{nodes, edges}`), syncs `selectedModules` into `StudioContext`, and (after a 2s
timer) PATCHes `/api/projects/{id}/blueprint` with `{modules, graph}`
(`step1.tsx:443-471`). This is the second reliable persistence path. The graph is
stored as an opaque JSON blob on the `projects` row; it is never compiled into
anything.

### 2.3 step2: Data and Logic (`steps/step2.tsx`)

Captures, across four phases:

- Data: per module, per entity field tables. Each field has a name, a type
  (uuid/string/text/int/float/boolean/datetime/json/address/uint256/bytes32/ipfs-hash),
  a storage strategy (database / on-chain / ipfs), and constraint toggles
  (required / unique / encrypted), plus a default value. Entities are seeded from
  presets or generated fallbacks. The user can add catalog entities or blank
  custom entities.
- Access Rules (Governance): per entity Create/Read/Update/Delete selectors
  (public/auth/owner/admin). These are rendered with `defaultValue` only and are
  not wired to state, so the selections are discarded.
- Logic and Triggers (Behavior): a per module toggle between "cards" (read only
  hardcoded trigger seeds; the add button is inert), "visual" (renders
  `LogicCanvas`), and "script" (renders `CustomScriptPanel`).
- API and Visibility (Exposure): per entity On-Chain / API Public switches and an
  encryption chip. The switches have no `onChange` and no state; they are
  decorative.

Produces: JSON. `handleSave` writes `localStorage['draft:local:3']` and
`StudioContext.schemaJson` with `{moduleEntities, relationships}` only
(`step2.tsx:464-469`). Governance and exposure selections are not in the snapshot.
The `relationships` array is only ever populated by loading a draft, never edited
in the UI (the relationship canvas is not rendered here). No DDL, no schema
emission, no contract structs are generated, despite UI copy claiming "each entity
becomes a database table or smart contract struct" and "Cerulea auto-generates
REST and GraphQL endpoints".

### 2.4 step3: Economics (`steps/step3.tsx`)

Captures (branches on project type): for dApps, monetization (billing model,
currency, trial, metered rates, tiers), payments (fiat/crypto, provider, treasury,
accepted tokens), assets (ERC-20 and NFT parameters), fees and splits, and
compliance (KYC provider, geo block, GDPR, legal URLs). For chains: tokenomics,
gas policy, staking, and governance.

Produces: nothing. `handleSave` is `if (goNext) goNext();` (`step3.tsx:266-268`).
None of the economics state is written to `localStorage`, `StudioContext`, or an
API. All of it is lost on navigation or refresh. (The progress bar even looks for
`cerulea.step3.economics` / `draft:local:4`, which this step never writes.)

### 2.5 step4: Integrations (`steps/step4.tsx`)

Captures: for a static catalog of about 35 providers across 7 categories
(Payments, Auth, Communication, Storage, Data, Analytics, Webhooks), per provider
enable toggle, test/live environment, and credential fields with per field regex
validation. A "Test Connection" button is fake: it `setTimeout`s 1200ms then does
a local non empty plus regex check, with no real request.

Produces: an attempt only. It loads via `GET /api/projects/{id}` and saves via
`PATCH /api/projects/{id}/integrations`, but that route does not exist on disk, so
the save silently 404s. Credentials live in component state only, despite UI copy
saying "stored encrypted".

### 2.6 step6: Review and Deploy (`steps/step6.tsx`)

This is the deployment step in the active flow. It is a fully simulated animation:

- `startDeployment()` fabricates a `deployId` from `generateRandomHex(8)`, a
  hardcoded region `us-east-1`, then drives a progress bar and a terminal with
  `setInterval` timers (`step6.tsx:277-407`).
- Log lines come from hardcoded arrays: `SCRIPTED_LOGS` ("Generating Solidity
  contracts...", "Compiling contracts with Hardhat...", "Build complete:
  artifacts/ (12MB)"), `INFRA_LOG_POOL`, and `BLOCKCHAIN_LOG_POOL`, selected at
  random.
- CPU/RAM/network/storage metrics are produced by a random walk helper.
- Progress speed is deliberately near frozen in the middle phases so it "takes
  hours"; a banner says "Do not close this window". The final "Open Dashboard"
  button is permanently disabled.
- The file makes zero `fetch` or API calls. Nothing is validated, generated,
  provisioned, or deployed.

### 2.7 Adjacent surfaces that also produce nothing

- `LogicCanvas.tsx` (used inside step2's visual mode) is a full ReactFlow flow
  editor with 13 typed block kinds (auth/ai/control/data/http/web3/util) and rich
  edge properties. But it is rendered with no props or callbacks, so its
  nodes/edges are ephemeral local state that is discarded on unmount. Nothing is
  saved, exported, or compiled.
- `SmartContractsScreen.tsx` plus `lib/smartContracts.ts` derive a list of
  contract descriptions (name, why it exists, if disabled, constructor parameter
  specs) from the selected modules. The `sourceSolidity` and `abi` fields are
  defined in the type but never populated; the Solidity shown is a hardcoded stub
  or a placeholder reading "Implementation generated by Cerulea at deploy time".
  There is no code generation.
- `CustomScriptPanel.tsx` (the script escape hatch) has a "Validate" button that
  is fake: TS returns a canned "syntax looks OK" after a timeout; Solidity only
  checks for the substring `contract` and matching braces. It states "Full compile
  will run during codegen/deploy", a path that does not exist.

### 2.8 Precise summary of buildable output today

The builder can produce exactly two persisted structures per project, plus one
in memory context object:

1. A `projects` row with name, slug, type, and workspace (step0).
2. A `blueprint` JSON blob on that row: the module graph nodes and edges plus
   per module config (step1).
3. In memory / localStorage only: a data schema snapshot (step2), which reaches
   `StudioContext.schemaJson` and `localStorage['draft:local:3']` but is not
   reliably written to the server by the Studio autosave (see 4.7).

Everything else the user enters (economics, integrations, UI layout, logic flows,
governance rules) is either not saved, saved to a nonexistent endpoint, or
discarded. And none of the captured data is ever transformed into a chain spec,
contract bytecode, a dapp scaffold, or a running deployment.

---

## 3. The gap between today and the target

Target: a no code builder where a developer, an enterprise, or a government can
construct a private chain and a dapp without writing code, with a code editor only
as a rare escape hatch.

Today's product captures a rich specification and then plays a deployment movie.
The gap is not primarily UI; the capture UI is largely built. The gap is the
entire output half of the product: turning a saved spec into real, running
software, and making the spec fully persisted and multi tenant safe on the way
there. The specific missing capabilities:

### 3.1 There is no code or config generation of any kind

- No chain spec / genesis generator. A `blockchain` project captures consensus,
  validators, native token, and fee model in step0 and (unsaved) tokenomics in
  step3, but nothing turns that into a genesis file, chain ID, node keys, or a
  bootnode/validator topology. The repository has a real chain in
  `cerulea-node`/`cerulea-runtime`/`cerulea-pallets`, but Studio has no path that
  parameterizes or builds it.
- No smart contract source or bytecode generation. `deriveSmartContracts()`
  produces descriptions and constructor parameter specs but never real Solidity
  or ABIs. The `sourceSolidity`/`abi` fields are perpetually empty.
- No dapp scaffold generation. The module graph, entity schema, and UI layout are
  never emitted as a Next.js app, contracts, or API. `/app/[slug]` renders a
  trivial component list from a JSON file on `localhost:4000`; it is a mock viewer,
  not generated code.
- No SDK/types generation (no Typechain style bindings, no client SDK).

### 3.2 There is no build, deploy, or provisioning pipeline

- No backend job that compiles contracts (the solc routes exist but are never
  called), provisions infra, starts nodes, or deploys a frontend.
- No `/api/deployments` route or `deployments` table, despite
  `apiClient.getDeployments()` calling it. No record of deployed addresses, tx
  hashes, network, environment, status, or logs.
- No environment model (testnet vs mainnet), no deployer key or secret storage, no
  RPC endpoint issuance. step6 fabricates a fake RPC/dashboard URL.

### 3.3 The captured spec is incomplete and lossy

- step3 economics is not saved at all.
- step4 integrations and step5 UI save to nonexistent endpoints (404).
- step2 governance and exposure controls are not wired to state.
- The Studio autosave (`StudioShell`) never contacts the server (see 4.7), so the
  aggregate project snapshot it builds is localStorage only.
- step0 chain/dapp detail forms are POSTed but have no column to land in.
- Logic flows built in `LogicCanvas` are never saved.

The consequence: even if a generator existed, most of the inputs it would need are
not durably captured today.

### 3.4 No verification, simulation, or preview of the real thing

- No dry run or validation of the configuration (step6's "Validation and Security"
  is scripted text).
- No testnet sandbox, no contract simulation, no schema/relationship consistency
  checks, no dependency solver enforcing that, for example, `nft-mint` implies
  `erc721` (the UI only suggests).
- No live preview of the generated dapp.

### 3.5 Not multi tenant safe for enterprise/government

- Draft rows carry no `userId`; isolation depends on the parent project's
  `userId`, which is nullable and was added by an ad hoc column migration.
- `/api/workspaces` is unauthenticated and backed by a process global in memory
  array shared across all users and lost on restart.
- There is no team, membership, or role model (profiles.role is free text). A
  government or enterprise buyer needs org accounts, RBAC, audit trails, and data
  residency, none of which exist as real backend features.
- The whole app sits behind a pricing wall by middleware, which is at odds with a
  self serve enterprise trial and with any public documentation surface.

### 3.6 The escape hatch is not real

The target wants a code editor as a rare escape hatch. Today `CustomScriptPanel`
is a textarea with a fake validator and no compile, no save into the project, and
no execution. The real compile routes are not connected to it.

### 3.7 Honesty gap

step6 and the assistant system prompt assert that contracts are compiled, infra is
provisioned, and the app goes live. The assistant is explicitly instructed to
never say "demo", "fake", "mock", or "coming soon". Because none of that happens,
the product currently overstates itself to users, which is a trust and (for
regulated buyers) compliance risk.

---

## 4. Data model and persistence; auth and session security

### 4.1 Engines and files (four bootstraps, two engines, three schemas)

| Module | Engine | Location | Status |
|---|---|---|---|
| `src/db/client.ts` | better-sqlite3 + Drizzle | `DRIZZLE_SQLITE_PATH` or `.data/cerulea.sqlite` | Active. All `/api/*` routes and `lib/auth.ts` use this. |
| `src/db/index.ts` | node-postgres Pool + Drizzle | `DATABASE_URL` | Postgres pool created, re-exports `./schema` and `./client`; not used by audited routes. |
| `src/server/db/sqlite.ts` | better-sqlite3 (raw) | `DB_FILE` or `apps/frontend/data/cerulea.db` | Used by `api/dbcheck` and `server/db/queries.ts`; different schema. |
| `src/lib/sqlite.ts` | better-sqlite3 (raw) | `var/data/cerulea.db` | Effectively unused. |

So there are three distinct SQLite file paths plus a Postgres connection.
`client.ts` also runs its own imperative `ensureColumn` migrations and `updatedAt`
triggers on every import, in parallel to the Drizzle migration in `drizzle/`.

### 4.2 Active schema (`src/db/schema.ts`, SQLite, all columns TEXT)

Tables: `workspaces` (id, name, createdAt), `projects` (id, name, slug,
description, projectType, workspaceId, userId, selectedTemplateIds, blueprint,
graph, schemaJson, logicJson, economics, legacyMode, status, timestamps; all JSON
stored as stringified TEXT), `users` (id, email, hashedPassword nullable, name,
isTestAccount, timestamps), `profiles`, `verificationTokens` (id, identifier,
token, expiresAt, createdAt), `drafts` (id, projectId, data, timestamps; no
userId, no step, no version), `aiThreads`, `aiMessages`, `subscriptions`,
`smartContracts` (rich, but no API route reads or writes it),
`userPlanSelections` (unused).

### 4.3 Two other schemas that disagree

- `server/db/sqlite.ts` defines different tables in a different file, including
  `blueprints`, `schemas`, `logic` (one JSON row per project) and a
  `projects.templateId` column. `server/db/queries.ts` reads those tables and that
  column, which do not exist in the active schema, so those functions would throw
  if called.
- `drizzle/0000_mute_mimic.sql` is Postgres (jsonb, snake_case, real FKs). Its
  `drafts` table has `user_id`, `project_id`, `step`, `payload_json`,
  `autosave_version`, and `UNIQUE(project_id, step)`. `drizzle.config.ts` points at
  a `./db/schema.ts` Postgres schema, not `src/db/schema.ts`. These migrations can
  never apply to the SQLite DB the app actually runs on.

### 4.4 Three competing draft mechanisms

- `/api/drafts/[[...slug]]` (bucket model): id = `${projectId}::${bucket}`, one row
  per project+bucket, overwrite, no history.
- `/api/projects/[id]/draft` (append model): inserts a new row with `randomUUID()`
  on every PUT; GET returns latest by `updatedAt`. Unbounded, no dedup, contradicts
  both the bucket model and the migration's uniqueness.
- Project embedded JSON: blueprint/schema/logic overwritten wholesale on the
  `projects` row.

There is no coherent versioning despite `autosave_version` existing in the unused
Postgres migration.

### 4.5 Redundant and dead persistence code

- Two byte identical autosave hooks: `src/hooks/useAutoSave.ts` and
  `src/lib/useAutoSave.ts`.
- `src/lib/idb.ts` implements a full IndexedDB draft store that the autosave path
  never uses.
- Dead API clients in `apiClient.ts`: `getApps`, `createApp`, `getDrafts`,
  `getDeployments` target routes that do not exist.
- Module catalogs are duplicated: `modules/module-registry.ts` (24 hardcoded MVP
  modules) versus the 225 module `data/modules.seed.json` served by `/api/modules`.
  Templates are duplicated: `config/templates.ts` (7 legacy entries, unused)
  versus 100 in `data/templates.seed.json`.

### 4.6 Authorization on data endpoints (IDOR review)

Properly scoped to `session.user.id` in the WHERE clause: `GET/POST /api/projects`,
`/api/projects/[id]` (GET/DELETE), and the `[id]/draft`, `[id]/blueprint`,
`[id]/schema`, `[id]/logic` routes (via an ownership helper), `/api/drafts/*`, and
`/api/subscriptions`.

Weak or missing:

- `/api/workspaces`: no auth at all; GET/POST/DELETE operate on a process global in
  memory array shared across every user.
- `/api/dbcheck`: no auth; returns table names (minor info disclosure).
- Ownership depends on `projects.userId`, which is nullable and added by
  `ensureColumn` after the fact. Any row with `userId IS NULL` is unguardable.
- The project create slug uniqueness scan is global, not per user, leaking the
  existence of other users' slugs via suffixing.

### 4.7 The Studio autosave never reaches the server

`StudioShell` calls `useAutoSave` with `projectId ?? 'local'`, `stepCode:
'project'`, and no `remote` config (`StudioShell.tsx:189-193`). With `remote` null,
the hook only writes `localStorage['draft:${projectId}:${stepCode}']` and returns
before any network call. Separately, `StepperNav` calls `saveToBackend(...)` which
POSTs `{id, appId, step, payload}` to `/api/drafts/upsert`, but that route expects
`{projectId, bucket, data}` and returns 400. So the "save on Next" button is dead
too. The only server writes from the builder are step0's project create and step1's
blueprint PATCH.

### 4.8 Data model gaps for the target domain

Missing entirely: chain configuration (genesis, chain ID, consensus/validator set,
node params, RPC endpoints, network topology), a deployments table (addresses, tx
hashes, network, status, logs), environments and secret storage, coherent version
history and rollback, collaboration (workspace membership, teams, real RBAC), and
structured/queryable domain data (economics, blueprint, logic, schema are all
untyped JSON text). Integrity is weak: JSON stored as TEXT, FKs depend on a per
connection pragma, nullable `userId` undermines tenant isolation.

### 4.9 Auth and session security review

Mechanism: NextAuth with a single Credentials provider (email + password), JWT
session strategy, custom claims `userId`, `plan`, `isTestAccount`
(`lib/auth.ts`). Cookie flags are correct: `httpOnly`, `secure` in production,
`sameSite=lax`, optional cross subdomain domain from `AUTH_COOKIE_DOMAIN`. Session
maxAge is unset, so it defaults to 30 days; the client never revalidates
(`refetchInterval={0}`).

Password hashing: bcrypt cost 10 (`lib/passwords.ts`). Adequate algorithm, cost is
below the current recommendation of 12+.

Findings, ranked:

Critical:

1. Hardcoded admin backdoor. `scripts/seed-test-account.cjs` seeds
   `test@cerulea.app` / `test1234` with `isTestAccount='true'`. Any account with
   `isTestAccount` becomes an admin on the control subdomain and is granted a free
   `pro` plan (`auth.ts`, `middleware.ts:78-105`). Dangerous if seeded in
   production.
2. Password reset is entirely broken. `forgot-password` stores `identifier =
   email`, but `reset-password` requires `identifier` to start with `pwd:` and
   derives the userId by stripping that prefix. No code path writes a `pwd:`
   identifier, so every reset token fails validation. It fails closed (not
   exploitable) but account recovery does not work, and the mismatch shows the
   flow was never exercised end to end.

High:

3. Open redirect in `force-signout`: the `next` query param is written straight
   into the `Location` header with no validation, so
   `/api/auth/force-signout?next=https://evil.com` redirects offsite.
4. No password validation on reset: `reset-password` only checks that the password
   is non empty, so a 1 character password is accepted, bypassing the register
   rule of min length 8.
5. `/api/auth/me` route is missing. `AuthGate.tsx`, `UserMenu.tsx`, and the profile
   page call it. Because `/api/*` is excluded from middleware, the call 404s;
   `AuthGate` then treats every user as unauthenticated. `AuthGate` is dead code
   and the profile page cannot load or save. Real enforcement rests solely on
   middleware.

Medium:

6. Email enumeration on registration (409 "Email already in use"), which
   contradicts the deliberately neutral forgot-password response.
7. No DB level unique constraint on `users.email` plus check then insert: a TOCTOU
   race can create duplicate accounts.
8. No CSRF protection on the custom `register`/`forgot-password`/`reset-password`
   POSTs, and logout CSRF on the GET `force-signout` (any page can trigger it via
   an image tag).
9. Reset tokens are stored in plaintext and travel in the URL query string
   (referrer and log leakage).

Low:

10. bcrypt cost 10 (raise to 12+).
11. Session maxAge unset (30 day default), no client side session refresh.
12. The entire site, including the marketing landing, is gated behind login and
    then a pricing wall. Likely unintended and bad for SEO and enterprise trials.
13. `forgot-password` does not lowercase the email lookup, unlike register/login,
    so mixed case addresses will not match.
14. `/api/ceruleai` and all five compile routes are unauthenticated (see 5).

Positive: cookie flags are correct; forgot-password is enumeration safe; the reset
token is single use with a 1 hour expiry; `test-account/reset` is properly gated.

---

## 5. Where the assistant is wired and what it can do

### 5.1 Provider and mounting

The live assistant ("CeruleAI") uses Google Gemini via `@google/generative-ai`.
The endpoint the UI actually calls is `src/app/api/ceruleai/route.ts`, model
`process.env.GEMINI_MODEL || 'gemini-2.5-flash'`, key `GEMINI_API_KEY` (server
only). The `Assistant` component is mounted globally in the root layout, so the FAB
appears on every route.

### 5.2 What it can do (chat only)

- It reads Studio state to build context: `projectId`, `projectType`, `templateId`,
  `selectedModules`, `appMetadata`, `appGoal` from `useStudio()`, plus a
  client side "project memory" in `localStorage['ceruleai:memory:<projectId>']`.
- Its only mutation is writing that localStorage memory cache. It does not modify
  the draft, modules, entities, or any DB record.
- It has no tool calling or function calling and no apply-changes flow, so there is
  nothing for a user to "accept". The reply is plain text streamed with a fake
  typewriter effect. This satisfies the stated policy that AI must not silently
  mutate user data, but only because it cannot mutate anything.
- The system prompt is a large hardcoded persona that describes the wizard,
  pricing (INR figures that disagree with the pricing page), and claims
  capabilities like "Smart Contract Compilation Guidance" and "API Integration
  Generation". It instructs the model to never say "demo", "fake", "mock", or
  "coming soon" and to never hallucinate features, while asserting features that
  do not exist.

### 5.3 Orphaned assistant backend

`api/ai/threads` and `api/ai/threads/[id]/messages` are a more complete,
DB backed (`aiThreads`/`aiMessages`), authenticated chat implementation, but no UI
code calls them. The live path (`/api/ceruleai`) is stateless and unauthenticated.

### 5.4 Compile routes exist but are unused and unauthenticated

Five routes run real compilers via dynamic import: three near duplicate solc
routes (`compile/solidity`, `solc/compile`, `tools/solc/compile`), a TS transpile
route (`tools/ts/compile`), and a language switched route (`scripts/compile`). No
UI code calls any of them. They are unauthenticated and have no input size limits,
rate limiting, or timeouts, so they are a resource exhaustion / DoS surface (solc
on adversarial input is CPU and memory heavy). There is no `eval` and no shelling
out, so there is no direct RCE path.

### 5.5 Payments

Stripe is a real integration wired to the pricing UI: `api/stripe/checkout`
(auth gated), `api/stripe/webhook` (real signature verification), and
`api/subscriptions`. One notable risk: if `STRIPE_SECRET_KEY` is unset, checkout
takes a dev fallback that directly activates a paid subscription in the DB with no
payment. Safe in dev, dangerous if shipped without the key. There is also a price
mismatch between the pricing page and the assistant prompt.

---

## 6. Prioritized build list to close the gap

Ordered by user value: the fastest path to a product that actually produces
something a user can run, then the platform features enterprise and government
buyers require, then cleanup. Each item names the concrete work.

### P0: Make the product produce a real, runnable artifact

1. Persist the full spec durably and per user. Fix the Studio autosave to write to
   the server (pass a `remote` config, or replace with a single canonical draft
   endpoint). Add `userId` (NOT NULL) to drafts. Save step3 economics, step4
   integrations, and step5 UI. Create the missing `/api/projects/[id]/integrations`
   and `/api/projects/[id]/ui` routes, or repoint the steps to existing ones. Wire
   step2 governance and exposure controls to state. Save `LogicCanvas` flows. This
   is the precondition for any generator.
2. Build the first real generator: contracts. Turn `selectedModules` plus module
   `configSchema` values and the entity schema into real Solidity (start from
   OpenZeppelin templates), then call the existing solc route (now authenticated
   and resource limited) to produce ABI and bytecode. Persist into the existing
   `smartContracts` table (currently unused) with real `sourceSolidity`, `abi`,
   `bytecode`.
3. Build a real deploy pipeline for one target end to end. Create a `deployments`
   table and `/api/deployments` route and a backend worker that: validates the
   config, deploys the generated contracts to `cerulea-testnet`, records addresses
   and tx hashes, and returns a real RPC/dashboard URL. Replace the step6 animation
   with live status from that job. This alone converts the wizard from a demo into
   a product.
4. Chain spec generation for the blockchain track. Map step0/step3 chain inputs
   (consensus, validators, native token, fees) to a genesis/chain spec for the
   `cerulea-node` runtime, provision a testnet node set, and surface node status.

### P1: Complete the dapp output and the escape hatch

5. Dapp scaffold generation. Emit a runnable frontend from the entity schema, UI
   layout (step5, which needs to be re-enabled), and contract ABIs: pages, forms,
   contract client, and API. Replace the `localhost:4000` mock viewer in
   `/app/[slug]` with the generated app plus a downloadable export (zip or repo).
6. Real escape hatch. Turn `CustomScriptPanel` into a genuine editor that saves per
   project, compiles via the real routes, shows real diagnostics, and feeds the
   generated output back into the project. Remove the fake validators.
7. Verification and preview. Add config validation (dependency solver so, for
   example, `nft-mint` requires `erc721`), schema consistency checks, a contract
   simulation/dry run, and a live preview of the generated dapp before deploy.

### P2: Enterprise and government readiness

8. Multi tenant backend. Real workspaces (replace the in memory
   `/api/workspaces`), org accounts, team membership, and enforced RBAC. Consolidate
   onto one database engine and one schema; delete the dead engines and the
   divergent `server/db` and Postgres migration if not adopting Postgres.
9. Environments, secrets, and deployment history. Testnet vs mainnet, encrypted
   secret storage for deployer keys and integration credentials (step4 credentials
   are plaintext in state today), and a real deployments/audit history that the
   dashboard and admin pages read (they are mostly hardcoded arrays now).
10. Real dashboard and admin data. Replace the mock arrays in keys, governance,
    audit, state, integrations, settings billing, and every admin page with live
    data; build the five missing admin pages (deployments, ai, billing, support,
    config).

### P3: Security, correctness, and trust cleanup

11. Fix the auth findings from section 4.9: remove or gate the test account
    backdoor, repair the password reset identifier mismatch, validate the
    `force-signout` redirect, enforce reset password strength, add the missing
    `/api/auth/me` route (or delete `AuthGate`/`UserMenu`), add CSRF protection,
    add a DB unique constraint on email, raise bcrypt cost, and stop gating the
    public landing behind login.
12. Authenticate and rate limit the AI and compile routes; unify the assistant on
    the DB backed threads implementation and delete the orphaned one; align the
    assistant prompt and pricing figures with reality and stop instructing it to
    deny that features are incomplete.
13. Remove dead and duplicate code: `backup of steps/*`, `*_old.tsx`, the orphaned
    canvases and inspectors, the duplicate autosave hook, the duplicate module and
    template catalogs, the dead `apiClient` methods, and the unused IndexedDB layer.

---

## Appendix: catalog inventory

- Modules: 225 in `data/modules.seed.json` (125 dApp, 100 blockchain), each with a
  `configSchema`. A parallel, unused list of 24 modules lives in
  `modules/module-registry.ts`.
- Templates: 100 in `data/templates.seed.json` (dApp and blockchain, including L2/L3
  rollups, consortium chains, and enterprise ledgers). A parallel, unused list of 7
  lives in `config/templates.ts`.
- Entities: a typed entity catalog in `data/entities-catalog.ts` with fields,
  storage strategies, and constraints, plus large preset files
  (`data/module-entity-presets.json`, `data/blocks.seed.json`).

The catalog is the strongest asset in the codebase. The work ahead is to connect it
to real generation and deployment rather than to a simulated one.
