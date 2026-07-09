// apps/frontend/src/app/api/modules/route.ts
import { NextResponse } from 'next/server';

// Import the catalog directly; no filesystem lookups.
// Make sure your file lives at: apps/frontend/src/data/modules.seed.json
import catalogJson from '@/data/modules.seed.json';

export const dynamic = 'force-dynamic';

type Module = {
  moduleId: string;
  title: string;
  projectType: 'dapp' | 'blockchain';
  category: string;
  tags?: string[];
  blurb?: string;
  longDescription?: string[];
  dependsOn?: string[];
  recommends?: string[];
  reasonByDepId?: Record<string, string>;
  configSchema?: any;
};

type Catalog = { version: number; updatedAt: string; modules: Module[] };

const CATALOG: Catalog = catalogJson as unknown as Catalog;

export async function GET(req: Request) {
  try {
    const url = new URL(req.url);

    const projectType = url.searchParams.get('projectType') as
      | 'dapp'
      | 'blockchain'
      | null;

    const q = (url.searchParams.get('q') || '').toLowerCase();
    const category = (url.searchParams.get('category') || '').toLowerCase();

    const ids = (url.searchParams.get('ids') || '')
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);

    let mods = CATALOG.modules.slice();

    if (projectType) mods = mods.filter((m) => m.projectType === projectType);

    if (category) mods = mods.filter((m) => m.category.toLowerCase() === category);

    if (ids.length) {
      const set = new Set(ids);
      mods = mods.filter((m) => set.has(m.moduleId));
    }

    if (q) {
      mods = mods.filter((m) => {
        const hay = [
          m.title,
          m.blurb ?? '',
          (m.tags ?? []).join(' '),
          ...(m.longDescription ?? []),
          m.category,
          m.moduleId,
        ]
          .join(' ')
          .toLowerCase();
        return hay.includes(q);
      });
    }

    return NextResponse.json(mods, { status: 200 });
  } catch (err: any) {
    return NextResponse.json(
      { ok: false, error: err?.message ?? 'Unknown error' },
      { status: 500 }
    );
  }
}
