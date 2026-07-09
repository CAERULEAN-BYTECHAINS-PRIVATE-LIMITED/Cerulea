import { NextResponse } from 'next/server';
import { getSession } from '@/lib/auth';

// Returns audit log stub data replace with real DB queries after audit_logs table is implemented.
export async function GET() {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });

  const logs = [
    { id: '1', actor: session.user.email, action: 'user.login', resource: 'User', resourceId: session.user.id, status: 'success', createdAt: new Date().toISOString() },
    { id: '2', actor: session.user.email, action: 'project.create', resource: 'Project', resourceId: 'proj-001', status: 'success', createdAt: new Date(Date.now() - 300000).toISOString() },
    { id: '3', actor: session.user.email, action: 'blueprint.save', resource: 'Project', resourceId: 'proj-001', status: 'success', createdAt: new Date(Date.now() - 600000).toISOString() },
  ];

  return NextResponse.json({ logs, total: logs.length });
}
