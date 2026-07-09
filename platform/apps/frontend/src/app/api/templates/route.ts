//apps\frontend\src\app\api\templates\route.ts
import { NextResponse } from 'next/server';
import { promises as fs } from 'fs';
import { join } from 'path';

export const dynamic = 'force-dynamic'; // read fresh from disk in dev

type Edge = { from: string; to: string; rel: string };

type Template = {
  id: string;
  projectType: 'dapp' | 'blockchain';
  title: string;
  description?: string;
  tags?: string[];
  preinstalledModules?: string[];
  modules?: string[];
  edges?: Edge[];
  ui?: { pages?: string[] };
};

type Catalog = {
  version: number;
  updatedAt: string;
  templates: Template[];
};

async function readCatalog(): Promise<Catalog> {
  const candidates = [
    join(process.cwd(), 'src', 'data', 'templates.seed.json'),
    join(process.cwd(), 'apps', 'frontend', 'src', 'data', 'templates.seed.json'),
  ];
  let lastErr: unknown = null;
  for (const p of candidates) {
    try {
      const raw = await fs.readFile(p, 'utf8');
      return JSON.parse(raw) as Catalog;
    } catch (err) {
      lastErr = err;
    }
  }
  throw lastErr ?? new Error('templates.seed.json not found');
}

// GET /api/templates?projectType=&q=&ids=a,b,c
export async function GET(req: Request) {
  try {
    const url = new URL(req.url);
    const projectType = url.searchParams.get('projectType') as
      | 'dapp'
      | 'blockchain'
      | null;
    const q = (url.searchParams.get('q') || '').toLowerCase();
    const ids = (url.searchParams.get('ids') || '')
      .split(',')
      .map(s => s.trim())
      .filter(Boolean);

    const catalog = await readCatalog();
    let list = catalog.templates.slice();

    if (projectType) list = list.filter(t => t.projectType === projectType);
    if (ids.length) list = list.filter(t => ids.includes(t.id));
    if (q) {
      list = list.filter(t => {
        const hay = [
          t.title,
          t.description || '',
          ...(t.tags || []),
          ...(t.ui?.pages || []),
        ]
          .join(' ')
          .toLowerCase();
        return hay.includes(q);
      });
    }

    return NextResponse.json(list, { status: 200 });
  } catch (err: any) {
    return NextResponse.json(
      { ok: false, error: err?.message ?? 'Unknown error' },
      { status: 500 },
    );
  }
}
