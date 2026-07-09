#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

has_file(){ [ -f "$ROOT/$1" ] && echo "✅" || echo "❌"; }
has_dir(){ [ -d "$ROOT/$1" ] && echo "✅" || echo "❌"; }
has_dep(){ node -e "const p=require('$ROOT/package.json'); const h=d=>!!((p.dependencies||{})[d]||(p.devDependencies||{})[d]); process.exit(h(process.argv[1])?0:1);" "$1" >/dev/null 2>&1 && echo "✅" || echo "❌"; }
grep_any(){ rg -q "$2" "$ROOT/$1" >/dev/null 2>&1 && echo "✅" || echo "❌"; } # needs ripgrep (rg). swap to grep -R if needed.

echo "0) Ground rules"
echo "   Studio Shell route (/studio or /_studio): $(has_file apps/frontend/src/app/studio/page.tsx) / $(has_file apps/frontend/src/app/_studio/page.tsx)"
echo "   StepRegistry:                              $(has_file apps/frontend/src/studio/StepRegistry.tsx)"
echo "   Middleware present:                        $(has_file apps/frontend/src/middleware.ts)"
echo "   Hidden deep-links (?s=) in Shell:          $(grep_any apps/frontend/src/app 'history\\.replaceState|\\?s=')"

echo
echo "1) Foundation: DB + Auth + Email + Storage"
echo "   Drizzle dep:                               $(has_dep drizzle-orm)"
echo "   Drizzle config file:                       $(has_file drizzle.config.ts)"
echo "   NextAuth dep:                              $(has_dep next-auth)"
echo "   NextAuth route:                            $(has_file apps/frontend/src/app/api/auth/[...nextauth]/route.ts)"
echo "   Resend dep:                                $(has_dep resend)"
echo "   UploadThing dep:                           $( (has_dep uploadthing && echo ✅) || (has_dep @uploadthing/react) )"
echo "   UploadThing route (typical):               $(has_file apps/frontend/src/app/api/uploadthing/core.ts)"
echo "   Core schema hints (users/projects/...):    $(grep_any 'apps|packages|libs' 'users|profiles|projects|drafts|ai_threads|ai_messages|assets')"

echo
echo "2) Theme system + Glass UI"
echo "   ThemeProvider used:                        $(grep_any apps/frontend/src 'ThemeProvider')"
echo "   Glass tokens/components present:           $(grep_any apps/frontend/src 'Glass(Card|Panel|AppBar|Button)|backdrop-filter|backdropFilter')"

echo
echo "3) Routing & Shell"
echo "   Shell mounted at clean URL:                $(grep_any apps/frontend/src/app/middleware.ts '/_studio|/studio')"
echo "   Header/footer wiring in Shell:             $(grep_any apps/frontend/src/app '_studio|studio/page\\.tsx' 'LinearProgress|Saved')"

echo
echo "4) Autosave + Resume"
echo "   useAutosave hook file:                     $(has_file apps/frontend/src/hooks/useAutosave.ts)"
echo "   Drafts API stub/route:                     $(has_file apps/frontend/src/app/api/projects/[id]/drafts/route.ts)"

echo
echo "5) CeruleAI (centralized)"
echo "   AI threads API:                            $(has_file apps/frontend/src/app/api/ai/threads/route.ts)"
echo "   AI messages API:                           $(has_file apps/frontend/src/app/api/ai/messages/route.ts)"
echo "   /ai page or AI Drawer:                     $( (has_file apps/frontend/src/app/ai/page.tsx && echo ✅) || (grep_any apps/frontend/src 'AIDrawer|Global AI Drawer') )"

echo
echo "6) Auth UI overhaul"
echo "   /auth/register page:                       $(has_file apps/frontend/src/app/auth/register/page.tsx)"
echo "   /auth/login page:                          $(has_file apps/frontend/src/app/auth/login/page.tsx)"
echo "   Forgot/Reset pages:                        $( (has_file apps/frontend/src/app/auth/forgot/page.tsx && echo ✅) || (has_file apps/frontend/src/app/auth/reset/page.tsx && echo ✅) || echo ❌ )"

echo
echo "7) Dashboard overhaul"
echo "   /dashboard page:                           $(has_file apps/frontend/src/app/dashboard/page.tsx)"

echo
echo "8) Landing page overhaul"
echo "   / (landing) page:                          $(has_file apps/frontend/src/app/page.tsx)"
echo "   SEO tags present:                          $(grep_any apps/frontend/src/app/page.tsx '<meta|export const metadata')"

echo
echo "9) Studio step refactor & polish"
echo "   Modular StepRegistry wired:                $(has_file apps/frontend/src/studio/StepRegistry.tsx)"
echo "   Shared form kit (heuristic):               $(grep_any apps/frontend/src 'FormProvider|react-hook-form|zod')"

echo
echo "10) File uploads inside Studio"
echo "   Upload UI usage:                           $(grep_any apps/frontend/src '@uploadthing/react|UploadButton|UploadDropzone')"

echo
echo "11) Hardening & ops"
echo "   Error pages (app router):                  $( (has_file apps/frontend/src/app/error.tsx && echo ✅) || (has_file apps/frontend/src/app/not-found.tsx && echo ✅) || echo ❌ )"
echo "   Rate limiting/headers (heuristic):         $(grep_any apps/frontend/src/app 'rateLimit|next-safe|helmet|secureHeaders')"
echo "   Analytics wiring (heuristic):              $(grep_any apps/frontend/src 'posthog|plausible|telemetry|analytics')"

echo
echo "--- tip ---"
echo "Any ❌ above = pending. Open and inspect the paths right next to them."
