// apps/frontend/scripts/fix-step-dupes.cjs
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.resolve(__dirname, '..'); // apps/frontend/
const STEPS_DIR = path.join(ROOT, 'src', 'components', 'studio', 'steps');

if (!fs.existsSync(STEPS_DIR)) {
  console.error('Steps folder not found:', STEPS_DIR);
  process.exit(1);
}

const files = fs.readdirSync(STEPS_DIR).filter(f => /^Step\d+\.tsx$/i.test(f) || /^step\d+\.tsx$/i.test(f));

let changed = 0;
for (const f of files) {
  const p = path.join(STEPS_DIR, f);
  let txt = fs.readFileSync(p, 'utf8');

  // If the file was pasted multiple times, keep only the first block (split by "// FILE:")
  const first = txt.indexOf('// FILE:');
  if (first !== -1) {
    const second = txt.indexOf('// FILE:', first + 8);
    if (second !== -1) txt = txt.slice(0, second).trimEnd() + '\n';
  }

  // Keep only the first 'use client'
  const parts = txt.split(/['"]use client['"];?/);
  if (parts.length > 2) txt = `'use client';\n` + parts.slice(1).join('');

  fs.writeFileSync(p, txt, 'utf8');
  console.log('Cleaned duplicates in', f);
  changed++;
}
console.log(`Done. Cleaned ${changed} files.`);
