import { NextResponse } from 'next/server';
import { db } from '@/db/client';
import { drafts, projects } from '@/db/schema';
import { eq, and } from 'drizzle-orm';
import { getSession } from '@/lib/auth';

export const dynamic = 'force-dynamic';

// Verify the project belongs to the current user before reading/writing its drafts
async function verifyProjectOwnership(projectId: string, userId: string) {
  const [row] = await db
    .select({ id: projects.id })
    .from(projects)
    .where(and(eq(projects.id as any, projectId), eq(projects.userId as any, userId)))
    .limit(1);
  return !!row;
}

// POST /api/drafts  — create / upsert draft
export async function POST(req: Request) {
  try {
    const session = await getSession();
    if (!session?.user?.id) return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });

    const body = await req.json().catch(() => null);
    const { projectId, bucket, data } = body || {};

    if (!projectId || !bucket) {
      return NextResponse.json({ ok: false, error: 'projectId and bucket are required' }, { status: 400 });
    }

    if (!(await verifyProjectOwnership(projectId, session.user.id))) {
      return NextResponse.json({ ok: false, error: 'Forbidden' }, { status: 403 });
    }

    const id = `${projectId}::${bucket}`;
    const stored = JSON.stringify(data);

    // Upsert: check if exists then insert or update
    const [existing] = await db.select({ id: drafts.id }).from(drafts).where(eq(drafts.id as any, id)).limit(1);
    if (existing) {
      await db.update(drafts).set({ data: stored as any }).where(eq(drafts.id as any, id));
    } else {
      await db.insert(drafts).values({ id, projectId, data: stored as any });
    }

    return NextResponse.json({ ok: true, updatedAt: Date.now() });
  } catch (err: any) {
    return NextResponse.json({ ok: false, error: String(err) }, { status: 500 });
  }
}

export const PUT = POST;

// GET /api/drafts?projectId=&bucket=
export async function GET(req: Request) {
  try {
    const session = await getSession();
    if (!session?.user?.id) return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });

    const url = new URL(req.url);
    const projectId = url.searchParams.get('projectId');
    const bucket = url.searchParams.get('bucket');

    if (!projectId || !bucket) {
      return NextResponse.json({ ok: false, error: 'projectId and bucket are required' }, { status: 400 });
    }

    if (!(await verifyProjectOwnership(projectId, session.user.id))) {
      return NextResponse.json({ ok: false, error: 'Forbidden' }, { status: 403 });
    }

    const id = `${projectId}::${bucket}`;
    const [row] = await db.select().from(drafts).where(eq(drafts.id as any, id)).limit(1);

    if (!row) return NextResponse.json({ ok: false, error: 'Not found' }, { status: 404 });

    let data: any = (row as any).data;
    if (typeof data === 'string') {
      try { data = JSON.parse(data); } catch { /* keep raw */ }
    }

    return NextResponse.json({ ok: true, data, updatedAt: (row as any).updatedAt });
  } catch (err: any) {
    return NextResponse.json({ ok: false, error: String(err) }, { status: 500 });
  }
}
