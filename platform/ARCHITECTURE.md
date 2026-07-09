# Cerulea — Architecture

## What this project is
Cerulea is a deployable web platform that includes:
- A public Cerulea website (homepage and marketing pages)
- Authentication and user account system
- An authenticated dashboard (post-login)
- Documentation pages
- Cerulea Studio (no-code builder) hosted on a dedicated subdomain: `studio.<domain>`

Cerulea Studio is a major module inside Cerulea, but it is not the entire product.

Some modules may not be fully implemented yet; however, the target architecture assumes they should exist and be completed to form a fully functional deployable platform.

## Product surfaces
### 1) Public site (Cerulea)
- Homepage (editable and separately owned; may be reworked by a freelancer later)
- Product/marketing pages (as needed)
- Documentation entry points

### 2) Authentication and accounts
- Sign up
- Login
- Logout
- Forgot password / reset password
- Profile / account settings
- Session handling and protected routes

### 3) Dashboard (authenticated)
- Landing page after login
- Links/entry to Cerulea Studio (`studio.<domain>`)
- User projects/drafts overview (as applicable)
- Account and billing (if/when added)

### 4) Documentation
- Documentation routes/pages
- Search or navigation
- Publicly accessible

### 5) Cerulea Studio (subdomain)
Cerulea Studio is the guided builder experience (step-based workflow) that helps users create and configure applications and related artifacts. It includes:
- Stepper workflow (step0..stepN)
- Draft persistence (create/update/load)
- Controlled AI assistant integrated inside Studio (real Gemini-backed AI)

## High-level module map
### Frontend (current reality)
- Next.js App Router pages for:
  - public site
  - auth pages
  - dashboard
  - docs
  - studio routes (or studio app if separated)
- UI components (MUI)
- Studio steps and builder UI

### Backend (planned / evolving)
Backend responsibilities should exist (even if currently partial) for:
- Authentication/session
- Draft persistence and versioning
- User/project data
- Deployment/export orchestration
- Audit logs/events

If backend is not fully separated yet, changes must be incremental and must not break user-facing flows.

### AI services (current reality)
- Cerulea Studio AI is real, It's called CeruleAI and uses Gemini.
- AI must be controlled and deterministic in behavior from a product standpoint:
  - It answers user questions and explains options.
  - It must not silently mutate user data without explicit acceptance.

## Routing and domain model
- Public Cerulea site: `<domain>` (main domain)
- Cerulea Studio: `studio.<domain>` (subdomain)

The homepage must remain independently editable (treat it as its own surface) because it may be redesigned/re-implemented later.

## Key user flows
### 1) Standard platform flow
1. User visits `<domain>` and views the public site.
2. User signs up or logs in.
3. User lands in the dashboard.
4. User enters Studio via `studio.<domain>` to create/build.

### 2) Studio flow
1. Authenticated user lands in Studio.
2. User proceeds through steps sequentially.
3. Draft is saved via explicit save actions or controlled autosave.
4. User exports or deploys.
5. Each user must have their own seperate persistent saved data (drafts).

### 3) Draft persistence flow
- Draft is the core unit of work for Studio.
- Saves must be throttled and deduplicated to prevent excessive API calls and invocation spikes.
- Avoid loops: state updates triggering saves triggering state updates.

## Repository structure (expected)
This repo should be organized so the major product surfaces are clear:
- Public site (homepage and marketing pages)
- Auth system
- Dashboard
- Docs
- Studio

If Studio is inside the same Next.js app, its routes/components should be clearly separated.
If Studio is a separate app, it should live under its own folder (e.g., `apps/studio/`).

## Guardrails for changes
- Do not introduce new frameworks or libraries without recording the decision.
- Keep Studio step navigation consistent across step files via shared components.
- Never introduce placeholders in code. Every implementation must be complete and correct.
- Any cleanup/deletion must be proven safe (build/lint run successfully).

## How to reason about common failures
Common failure patterns include:
- Excessive API calls due to React effects or non-debounced draft saves
- Authentication state not correctly controlling headers/routes
- Static header shown across public and authenticated sections
- Duplicate patterns across Studio steps instead of shared components
