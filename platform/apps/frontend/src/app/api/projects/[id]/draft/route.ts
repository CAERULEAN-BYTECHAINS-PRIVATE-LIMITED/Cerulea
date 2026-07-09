// apps/frontend/src/app/api/projects/[id]/draft/route.ts
import { NextResponse } from "next/server";
import { getSession } from "@/lib/auth";
import { db } from "@/db/client";
import { drafts, projects } from "@/db/schema";
import { eq, and, desc } from "drizzle-orm";

type Params = { params: { id: string } };

export async function GET(_: Request, { params }: Params) {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ error: "Unauthorized" }, { status: 401 });

  const [proj] = await db.select().from(projects)
    .where(and(eq(projects.id, params.id), eq(projects.userId, session.user.id)));
  if (!proj) return NextResponse.json({ error: "Not found" }, { status: 404 });

  const [last] = await db.select().from(drafts)
    .where(eq(drafts.projectId, params.id))
    .orderBy(desc(drafts.updatedAt))
    .limit(1);
  return NextResponse.json({ draft: last || null });
}

export async function PUT(req: Request, { params }: Params) {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ error: "Unauthorized" }, { status: 401 });

  const body = await req.json(); // { step: number, payload: object }
  if (typeof body.step !== "number" || body.payload == null)
    return NextResponse.json({ error: "Bad request" }, { status: 400 });

  const [proj] = await db.select().from(projects)
    .where(and(eq(projects.id, params.id), eq(projects.userId, session.user.id)));
  if (!proj) return NextResponse.json({ error: "Not found" }, { status: 404 });

  const { randomUUID } = await import('crypto');
  const [saved] = await db.insert(drafts).values({
    id: randomUUID(),
    projectId: params.id,
    data: { step: body.step, payload: body.payload },
  }).returning();

  return NextResponse.json({ draft: saved });
}
