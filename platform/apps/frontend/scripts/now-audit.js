// now-audit.js
// Robust repo-aware audit. Run from ANY folder (repo root or inside apps/frontend).
// Usage: node now-audit.js [--frontdir=apps/frontend] [--debug]

const fs = require('node:fs');
const path = require('node:path');

// ---------- CLI ----------
const argv = process.argv.slice(2);
const DEBUG = argv.includes('--debug');
const argFront = (argv.find(a => a.startsWith('--frontdir=')) || '').split('=')[1];

// ---------- FS helpers ----------
function safeStat(p){ try { return fs.statSync(p); } catch { return null; } }
function isDir(p){ const s = safeStat(p); return !!(s && s.isDirectory()); }
function isFile(p){ const s = safeStat(p); return !!(s && s.isFile()); }
function normalize(p){ return p.replace(/\\/g, '/'); }

function walkFiles(dir, out = []) {
  if (!isDir(dir)) return out;
  const ignore = new Set(['node_modules', '.next', '.turbo', 'dist', 'build', '.git', '.vercel', '.cache']);
  for (const name of fs.readdirSync(dir)) {
    if (ignore.has(name)) continue;
    const full = path.join(dir, name);
    const st = safeStat(full);
    if (!st) continue;
    if (st.isDirectory()) walkFiles(full, out);
    else out.push(full);
  }
  return out;
}

function findOneByRegex(files, re){ return files.find(f => re.test(normalize(f))); }
function existsByRegex(files, re){ return !!findOneByRegex(files, re); }

function readFileSafe(p){ try { return fs.readFileSync(p, 'utf8'); } catch { return ''; } }

function loadPkgJSONs(frontDir) {
  const roots = [
    path.resolve(process.cwd(), 'package.json'),
    path.resolve(frontDir, 'package.json'),
  ];
  const pkgs = [];
  for (const p of roots) {
    if (isFile(p)) { try { pkgs.push(require(p)); } catch {} }
  }
  return pkgs;
}
function pkgHas(pkgs, dep) {
  for (const pkg of pkgs) {
    if ((pkg.dependencies && pkg.dependencies[dep]) || (pkg.devDependencies && pkg.devDependencies[dep])) return true;
  }
  return false;
}

function log(...args){ if (DEBUG) console.log('[audit]', ...args); }
function line(label, ok){ console.log(`${ok ? '✅' : '❌'} ${label}`); }

// ---------- Frontend root resolution ----------
function resolveFrontendDir() {
  // 1) CLI override
  if (argFront) {
    const p = path.resolve(process.cwd(), argFront);
    if (isDir(p)) return p;
  }

  const cwd = normalize(process.cwd());
  log('cwd =', cwd);

  // 2) If path contains /apps/frontend, snap to that segment
  const marker = '/apps/frontend';
  const idx = cwd.indexOf(marker);
  if (idx !== -1) {
    const front = cwd.slice(0, idx + marker.length);
    if (isDir(front)) return front;
  }

  // 3) Probe upwards for apps/frontend (depth up to 6)
  let base = process.cwd();
  for (let i = 0; i < 6; i++) {
    const candidate = path.resolve(base, 'apps/frontend');
    if (isDir(candidate)) return candidate;
    base = path.resolve(base, '..');
  }

  // 4) As a last resort: maybe we're already at frontend root (has app/ or src/app/)
  if (isDir(path.resolve(process.cwd(), 'app')) || isDir(path.resolve(process.cwd(), 'src/app'))) {
    return process.cwd();
  }

  return null;
}

const FRONT_DIR = resolveFrontendDir();
if (!FRONT_DIR) {
  console.error('Could not locate the frontend folder. Pass --frontdir=apps/frontend or run from repo/frontend.');
  process.exit(1);
}
log('FRONT_DIR =', normalize(FRONT_DIR));

// ---------- Scan FS once ----------
const ALL_FILES = walkFiles(FRONT_DIR);
log('file count =', ALL_FILES.length);

const PKGS = loadPkgJSONs(FRONT_DIR);

// ---------- Checks ----------

// 0) Ground rules
const shellFile =
  findOneByRegex(ALL_FILES, /\/app\/(?:_studio|studio)\/page\.tsx$/i);

const stepRegistryFile =
  findOneByRegex(ALL_FILES, /\/StepRegistry\.tsx$/i);

const middlewareFile =
  findOneByRegex(ALL_FILES, /\/middleware\.(t|j)sx?$/i);

const hiddenDeepLinks = (() => {
  if (!shellFile) return false;
  const src = readFileSafe(shellFile);
  return /\?.*s=/.test(src) || /searchParams\.get\(['"`]s['"`]\)/.test(src);
})();

// 1) Foundation
const drizzleDep = pkgHas(PKGS, 'drizzle-orm');
const drizzleConfig = existsByRegex(ALL_FILES, /\/drizzle\.config\.(t|j)s$/i);

const nextAuthDep = pkgHas(PKGS, 'next-auth');
const nextAuthRoute = existsByRegex(ALL_FILES,
  /\/(app|src\/app|pages|src\/pages)\/api\/auth\/\[\.\.\.nextauth\]\/(route\.ts|route\.js|index\.(t|j)s)$/i);

const resendDep = pkgHas(PKGS, 'resend');

const uploadDep = pkgHas(PKGS, '@uploadthing/react') || pkgHas(PKGS, 'uploadthing');
const uploadRoute = existsByRegex(ALL_FILES,
  /\/app\/api\/uploadthing\/.*\.(t|j)sx?$/i) ||
  existsByRegex(ALL_FILES, /\/server\/uploadthing\.(t|j)s$/i);

const coreSchemas = existsByRegex(ALL_FILES, /\/(db|server\/db)\/schema\.(t|j)sx?$/i) ||
                    existsByRegex(ALL_FILES, /\/schemas?\/.*\.(t|j)sx?$/i);

// 2) Theme system + Glass UI
const appLayoutFile = findOneByRegex(ALL_FILES, /\/app\/layout\.tsx$/i);
const themeProviderUsed = (() => {
  if (!appLayoutFile) return false;
  const src = readFileSafe(appLayoutFile);
  return /ThemeProvider|createTheme|CssBaseline/.test(src);
})();
const glassKitPresent =
  existsByRegex(ALL_FILES, /\/components\/Glass(Card|Panel|AppBar|Button)\.tsx$/i) ||
  existsByRegex(ALL_FILES, /\/theme\/(glass|index)\.(t|j)sx?$/i);

// 3) Routing & Shell
const shellCleanUrl = !!(middlewareFile && shellFile);
const shellHeaderFooter = (() => {
  if (!shellFile) return false;
  const src = readFileSafe(shellFile);
  return /AppBar|Toolbar|LinearProgress|Stepper|Breadcrumbs|Footer/i.test(src);
})();

// 4) Autosave + Resume
const autosaveHook = existsByRegex(ALL_FILES, /\/hooks\/useAutosave\.(t|j)sx?$/i);
const draftsApi = existsByRegex(ALL_FILES,
  /\/app\/api\/projects\/[^/]+\/drafts\/route\.(t|j)s$/i);

// 5) CeruleAI (centralized)
const aiThreadsApi = existsByRegex(ALL_FILES, /\/app\/api\/ai\/threads\/route\.(t|j)s$/i);
const aiMessagesApi = existsByRegex(ALL_FILES, /\/app\/api\/ai\/messages\/route\.(t|j)s$/i);
const aiUI = existsByRegex(ALL_FILES, /\/app\/ai\/page\.tsx$/i) ||
             existsByRegex(ALL_FILES, /\/components\/AIDrawer\.tsx$/i);

// 6) Auth UI overhaul
const hasRegister = existsByRegex(ALL_FILES, /\/app\/auth\/register\/page\.tsx$/i);
const hasLogin    = existsByRegex(ALL_FILES, /\/app\/auth\/login\/page\.tsx$/i);
const hasForgot   = existsByRegex(ALL_FILES, /\/app\/auth\/(forgot|password\/(forgot|reset))\/page\.tsx$/i);
const hasReset    = existsByRegex(ALL_FILES, /\/app\/auth\/reset\/page\.tsx$/i);

// 7) Dashboard overhaul
const hasDashboard = existsByRegex(ALL_FILES, /\/app\/dashboard\/page\.tsx$/i);

// 8) Landing page overhaul
const hasLanding = existsByRegex(ALL_FILES, /\/app\/page\.tsx$/i);
const landingSEO = (() => {
  const f = findOneByRegex(ALL_FILES, /\/app\/page\.tsx$/i);
  if (!f) return false;
  const src = readFileSafe(f);
  return /export const metadata\s*=/.test(src);
})();

// 9) Studio step refactor & polish
const stepRegistryWired = !!stepRegistryFile;
const sharedFormKit =
  existsByRegex(ALL_FILES, /\/components\/(form|Form)\/(Field|index)\.tsx$/i) ||
  existsByRegex(ALL_FILES, /\/components\/form-kit\/index\.(t|j)sx?$/i);

// 10) File uploads inside Studio
const uploadUiUsage =
  existsByRegex(ALL_FILES, /\/components\/(UploadButton|AssetPicker)\.tsx$/i);

// 11) Hardening & ops
const hasErrorPage = existsByRegex(ALL_FILES, /\/app\/error\.tsx$/i);
const has404Page   = existsByRegex(ALL_FILES, /\/app\/not-found\.tsx$/i);
const rateLimits   = existsByRegex(ALL_FILES, /\/app\/api\/_lib\/rate-limit\.(t|j)s$/i) ||
                     existsByRegex(ALL_FILES, /\/middleware\.headers\.(t|j)s$/i);
const analytics    = pkgHas(PKGS, '@vercel/analytics') ||
                     pkgHas(PKGS, '@vercel/speed-insights') ||
                     existsByRegex(ALL_FILES, /\/app\/analytics\.(t|j)sx?$/i);

// ---------- Report ----------
console.log('0) Ground rules');
line('Studio Shell route (/studio or /_studio)', !!shellFile);
line('StepRegistry', !!stepRegistryFile);
line('Middleware present', !!middlewareFile);
line('Hidden deep-links (?s=) in Shell', hiddenDeepLinks);

console.log('\n1) Foundation: DB + Auth + Email + Storage');
line('Drizzle dep', drizzleDep);
line('Drizzle config file', drizzleConfig);
line('NextAuth dep', nextAuthDep);
line('NextAuth route', nextAuthRoute);
line('Resend dep', resendDep);
line('UploadThing dep', uploadDep);
line('UploadThing route (typical)', uploadRoute);
line('Core schema hints (users/projects/...)', coreSchemas);

console.log('\n2) Theme system + Glass UI');
line('ThemeProvider used', themeProviderUsed);
line('Glass tokens/components present', glassKitPresent);

console.log('\n3) Routing & Shell');
line('Shell mounted at clean URL', shellCleanUrl);
line('Header/footer wiring in Shell', shellHeaderFooter);

console.log('\n4) Autosave + Resume');
line('useAutosave hook file', autosaveHook);
line('Drafts API stub/route', draftsApi);

console.log('\n5) CeruleAI (centralized)');
line('AI threads API', aiThreadsApi);
line('AI messages API', aiMessagesApi);
line('/ai page or AI Drawer', aiUI);

console.log('\n6) Auth UI overhaul');
line('/auth/register page', hasRegister);
line('/auth/login page', hasLogin);
line('Forgot/Reset pages', hasForgot && hasReset);

console.log('\n7) Dashboard overhaul');
line('/dashboard page', hasDashboard);

console.log('\n8) Landing page overhaul');
line('/ (landing) page', hasLanding);
line('SEO tags present', landingSEO);

console.log('\n9) Studio step refactor & polish');
line('Modular StepRegistry wired', stepRegistryWired);
line('Shared form kit (heuristic)', sharedFormKit);

console.log('\n10) File uploads inside Studio');
line('Upload UI usage', uploadUiUsage);

console.log('\n11) Hardening & ops');
line('Error pages (app router)', hasErrorPage && has404Page);
line('Rate limiting/headers (heuristic)', rateLimits);
line('Analytics wiring (heuristic)', analytics);

console.log('\n--- tip ---');
console.log('Any ❌ above = pending. Use --debug to see how the frontend path was resolved.');
