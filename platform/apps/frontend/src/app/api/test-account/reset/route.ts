import { NextResponse } from 'next/server';
import { db } from '@/db/client';
import { projects, drafts, aiThreads, aiMessages, smartContracts } from '@/db/schema';
import { eq } from 'drizzle-orm';
import { getSession } from '@/lib/auth';

export const dynamic = 'force-dynamic';

// POST /api/test-account/reset
// Wipes all data for the test account and re-seeds a clean state.
// Only callable when logged in as the test account.
export async function POST() {
  try {
    const session = await getSession();
    if (!session?.user?.id) {
      return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });
    }

    if (!session.user.isTestAccount) {
      return NextResponse.json({ ok: false, error: 'Only the test account can use this endpoint' }, { status: 403 });
    }

    const userId = session.user.id;

    // Get all project IDs for this user
    const userProjects = await db
      .select({ id: projects.id })
      .from(projects)
      .where(eq(projects.userId as any, userId));

    const projectIds = userProjects.map((p: any) => p.id);

    // Delete in dependency order
    for (const projectId of projectIds) {
      await db.delete(smartContracts).where(eq(smartContracts.projectId as any, projectId));
      await db.delete(drafts).where(eq(drafts.projectId as any, projectId));

      const threads = await db
        .select({ id: aiThreads.id })
        .from(aiThreads)
        .where(eq(aiThreads.projectId as any, projectId));

      for (const thread of threads) {
        await db.delete(aiMessages).where(eq(aiMessages.threadId as any, thread.id));
      }
      await db.delete(aiThreads).where(eq(aiThreads.projectId as any, projectId));
    }

    // Delete all projects for this user
    await db.delete(projects).where(eq(projects.userId as any, userId));

    // Also wipe any AI threads linked directly to the user (no project)
    const userThreads = await db
      .select({ id: aiThreads.id })
      .from(aiThreads)
      .where(eq(aiThreads.userId as any, userId));

    for (const thread of userThreads) {
      await db.delete(aiMessages).where(eq(aiMessages.threadId as any, thread.id));
    }
    await db.delete(aiThreads).where(eq(aiThreads.userId as any, userId));

    return NextResponse.json({ ok: true, message: 'Test account data reset successfully.' });
  } catch (err: any) {
    console.error('Test account reset failed:', err);
    return NextResponse.json({ ok: false, error: String(err) }, { status: 500 });
  }
}
