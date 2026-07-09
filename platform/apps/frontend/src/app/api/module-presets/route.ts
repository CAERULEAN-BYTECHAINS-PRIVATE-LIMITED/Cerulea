import { NextResponse } from "next/server";
import { promises as fs } from "fs";
import path from "path";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET() {
  const candidates = [
    path.join(process.cwd(), "src/data/module-entity-presets.json"),
    path.join(process.cwd(), "apps/frontend/src/data/module-entity-presets.json"),
  ];
  for (const p of candidates) {
    try {
      const buf = await fs.readFile(p, "utf8");
      return NextResponse.json({ ok:true, data: JSON.parse(buf) });
    } catch {}
  }
  return NextResponse.json({ ok:false, error:"module-entity-presets.json not found" }, { status:404 });
}
