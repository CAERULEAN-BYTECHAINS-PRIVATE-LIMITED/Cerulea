// src/app/api/workspaces/route.ts
import { NextResponse } from 'next/server';

let WS: { id: string; name: string; slug: string; createdAt: string }[] = [
  { id: 'ws_personal', name: 'Personal Workspace', slug: 'personal', createdAt: new Date().toISOString() },
];

export async function GET() {
  return NextResponse.json(WS, { status: 200 });
}

export async function POST(req: Request) {
  const { name } = await req.json();
  if (!name || String(name).trim().length < 2) {
    return NextResponse.json({ error: 'Name too short' }, { status: 400 });
  }
  const id = `ws_${Math.random().toString(36).slice(2, 9)}`;
  const slug = name.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '').slice(0, 40);
  const item = { id, name, slug, createdAt: new Date().toISOString() };
  WS = [item, ...WS];
  return NextResponse.json(item, { status: 201 });
}

export async function DELETE(req: Request) {
  const url = new URL(req.url);
  const id = url.searchParams.get('id');
  if (!id) return NextResponse.json({ error: 'id required' }, { status: 400 });
  const before = WS.length;
  WS = WS.filter(w => w.id !== id);
  if (WS.length === before) return NextResponse.json({ error: 'Not found' }, { status: 404 });
  return NextResponse.json({ ok: true }, { status: 200 });
}
