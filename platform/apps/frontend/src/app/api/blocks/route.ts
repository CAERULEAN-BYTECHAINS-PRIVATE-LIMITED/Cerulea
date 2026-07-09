// apps/frontend/src/app/api/blocks/route.ts
import { NextResponse } from 'next/server';
import { promises as fs } from 'fs';
import path from 'path';

async function loadJson(relPath: string) {
  const roots = [
    path.join(process.cwd(), 'src', 'data', relPath),
    path.join(process.cwd(), 'apps', 'frontend', 'src', 'data', relPath),
  ];
  for (const p of roots) {
    try {
      const buf = await fs.readFile(p, 'utf8');
      return JSON.parse(buf);
    } catch (_) {}
  }
  throw new Error(`Not found: ${relPath}`);
}

export async function GET(req: Request) {
  try {
    const url = new URL(req.url);
    const track = (url.searchParams.get('track') || '').toLowerCase(); // 'dapp'|'blockchain'|''
    const all = await loadJson('blocks.seed.json'); // { groups: [...], blocks: [...] }

    let blocks = all.blocks || [];
    if (track) {
      blocks = blocks.filter((b: any) => !b.track || b.track === track || b.track === 'common');
    }
    return NextResponse.json({
      ok: true,
      groups: all.groups || [],
      blocks,
      edgeDefaults: all.edgeDefaults || {},
    });
  } catch (err: any) {
    return NextResponse.json({ ok: false, error: err.message }, { status: 500 });
  }
}
