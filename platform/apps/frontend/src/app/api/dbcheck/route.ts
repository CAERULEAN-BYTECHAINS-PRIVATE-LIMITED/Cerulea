import { NextResponse } from 'next/server';
import { getDb } from '../../../server/db/sqlite';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const db = getDb();
    const tables = db.prepare(`SELECT name FROM sqlite_master WHERE type='table'`).all();
    return NextResponse.json({ ok: true, tables }, { status: 200 });
  } catch (e: any) {
    return NextResponse.json({ ok: false, error: e?.message || 'Failed' }, { status: 500 });
  }
}
