export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

import { NextResponse } from 'next/server';
import { db } from '@/db/client';
import { projects } from '@/db/schema';
import { eq, and } from 'drizzle-orm';
import { getSession } from '@/lib/auth';

async function getOwnedProject(projectId: string, userId: string) {
  const [row] = await db
    .select()
    .from(projects)
    .where(and(eq(projects.id as any, projectId), eq(projects.userId as any, userId)))
    .limit(1);
  return row ?? null;
}

export async function GET(req: Request, { params }: { params: { id: string } }) {
  try {
    const session = await getSession();
    if (!session?.user?.id) return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });

    const row = await getOwnedProject(params.id, session.user.id);
    if (!row) return NextResponse.json({ ok: false, error: 'Not found' }, { status: 404 });

    let blueprint: any = (row as any).blueprint ?? null;
    if (typeof blueprint === 'string') {
      try { blueprint = JSON.parse(blueprint); } catch { /* keep raw */ }
    }

    return NextResponse.json({ ok: true, blueprint });
  } catch (err: any) {
    return NextResponse.json({ ok: false, error: String(err) }, { status: 500 });
  }
}

export async function PATCH(req: Request, { params }: { params: { id: string } }) {
  try {
    const session = await getSession();
    if (!session?.user?.id) return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });

    const row = await getOwnedProject(params.id, session.user.id);
    if (!row) return NextResponse.json({ ok: false, error: 'Not found' }, { status: 404 });

    const body = await req.json();
    await db.update(projects).set({ blueprint: JSON.stringify(body) as any }).where(eq(projects.id as any, params.id));

    return NextResponse.json({ ok: true });
  } catch (err: any) {
    return NextResponse.json({ ok: false, error: String(err) }, { status: 500 });
  }
}
