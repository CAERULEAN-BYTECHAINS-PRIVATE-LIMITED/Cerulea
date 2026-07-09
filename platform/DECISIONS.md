# Engineering Decisions — Cerulea

This file is the source of truth for key project decisions. If a direction changes, update this file.

## Product naming
- Product: Cerulea
- Module: Cerulea Studio
- Company: Caerulean Bytechains Private Limited

## Frontend framework
- Next.js (App Router)
- TypeScript
- React
- Material UI (MUI)

Do not introduce Tailwind/Chakra/PostCSS stacks unless explicitly decided.

## Product surfaces and routing
Cerulea is not only Studio. It includes:
- Public site (homepage, marketing pages)
- Auth system
- Dashboard
- Documentation pages
- Studio

Studio must be hosted at:
- `studio.<domain>` (subdomain)

## Homepage ownership
The Cerulea homepage must remain separately editable and treated as its own surface because it may be re-implemented later by a freelancer. Avoid coupling homepage code tightly to Studio logic.

## Draft persistence
- Draft is the core unit of work for Studio.
- Draft saves must be throttled and deduplicated.
- Avoid save loops caused by effects and state hydration.

If a change increases API calls significantly, it must be fixed before merging.

## Studio navigation consistency
- Studio is step-based.
- Next/Back controls must be consistent across steps:
  - shared component
  - shared styling
  - shared placement

## Authentication and header behavior
Expected behavior:
- Public routes show public header
- Authenticated routes show authenticated header (profile/logout/etc.)
- Header must not be static across all routes

## AI integration policy (Studio)
- AI in Cerulea Studio is real and uses Gemini.
- AI must not silently modify user data.
- If AI suggests changes, the user must accept explicitly.

## No placeholders policy
No placeholders are allowed in production code unless explicitly requested.
Examples of disallowed placeholders:
- TODOs intended to be left unfinished
- fake implementations returning dummy values
- partial code “sketches”
- “implement later” comments

All generated code must be complete, runnable, and aligned with existing architecture.

## Repository cleanliness
- Do not commit local sqlite or local caches.
- Cleanup tasks must prove safety:
  - build passes
  - lint passes
  - app boots

## Changes that require explicit approval
Do not do these without recording the decision here:
- switching frameworks or UI library
- changing routing/subdomain architecture significantly
- changing auth strategy
- changing draft schema in a breaking way
- adding paid third-party dependencies
