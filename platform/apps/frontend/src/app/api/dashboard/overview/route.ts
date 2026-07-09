import { NextResponse } from 'next/server';
import { getSession } from '@/lib/auth';
import { db } from '@/db/client';
import { projects } from '@/db/schema';
import { eq } from 'drizzle-orm';

export async function GET() {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });

  try {
    const userId = session.user.id;

    const allProjects = await db
      .select({ id: projects.id, name: projects.name, slug: projects.slug, status: projects.status, projectType: projects.projectType, createdAt: projects.createdAt })
      .from(projects)
      .where(eq(projects.userId, userId));

    const totalProjects = allProjects.length;
    const activeDeployments = allProjects.filter((p) => p.status === 'active').length;
    const draftProjects = allProjects.filter((p) => p.status !== 'active').length;

    return NextResponse.json({
      totalProjects,
      activeDeployments,
      draftProjects,
      recentProjects: allProjects.slice(0, 5),
      rpcCallsToday: null,
      governanceProposals: null,
    });
  } catch (e: any) {
    return NextResponse.json({ error: e.message }, { status: 500 });
  }
}
