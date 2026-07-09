import { NextRequest, NextResponse } from 'next/server';
import { db } from '@/db/client';
import { projects } from '@/db/schema';
import { eq, and } from 'drizzle-orm';
import { getSession } from '@/lib/auth';

export const dynamic = 'force-dynamic';

async function getOwnedProject(projectId: string, userId: string) {
  const [row] = await db
    .select()
    .from(projects)
    .where(and(eq(projects.id as any, projectId), eq(projects.userId as any, userId)))
    .limit(1);
  return row ?? null;
}

export async function GET(req: NextRequest, { params }: { params: { id: string } }) {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });

  const row = await getOwnedProject(params.id, session.user.id);
  if (!row) return NextResponse.json({ ok: false, error: 'Not found' }, { status: 404 });

  let schemaData: any = (row as any).schemaJson ?? null;
  if (typeof schemaData === 'string') {
    try { schemaData = JSON.parse(schemaData); } catch { /* keep raw */ }
  }
  return NextResponse.json(schemaData ?? { entities: [], relationships: [], track: 'dapp' });
}

export async function PUT(req: NextRequest, { params }: { params: { id: string } }) {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });

  const row = await getOwnedProject(params.id, session.user.id);
  if (!row) return NextResponse.json({ ok: false, error: 'Not found' }, { status: 404 });

  const body = await req.json();
  const clean = {
    entities: Array.isArray(body.entities) ? body.entities : [],
    relationships: Array.isArray(body.relationships) ? body.relationships : [],
    track: body.track === 'blockchain' ? 'blockchain' : 'dapp',
  };

  await db.update(projects).set({ schemaJson: JSON.stringify(clean) as any }).where(eq(projects.id as any, params.id));
  return NextResponse.json({ ok: true });
}

export const PATCH = PUT;
