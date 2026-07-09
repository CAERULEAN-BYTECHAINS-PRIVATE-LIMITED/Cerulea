// Build template -> entities by expanding template.preinstalledModules
import { NextResponse } from "next/server";
import { promises as fs } from "fs";
import path from "path";

export const dynamic = "force-dynamic";

async function tryRead(p: string) { try { return await fs.readFile(p, "utf-8"); } catch { return null; } }
async function loadJson(filename: string) {
  const roots = [
    path.join(process.cwd(), "src", "data", filename),
    path.join(process.cwd(), "apps", "frontend", "src", "data", filename),
  ];
  for (const p of roots) { const s = await tryRead(p); if (s) return JSON.parse(s); }
  return null;
}

async function getModulePreset(moduleId: string, track: string) {
  const res = await fetch(`${process.env.NEXT_PUBLIC_BASE_URL || ""}/api/presets/modules?id=${encodeURIComponent(moduleId)}&track=${track}`).catch(() => null);
  if (res?.ok) return res.json();
  return [];
}

export async function GET(req: Request) {
  const url = new URL(req.url);
  const id = url.searchParams.get("id");
  const track = url.searchParams.get("track") || "dapp";
  if (!id) return NextResponse.json([], { status: 200 });

  const templates = await loadJson("templates.seed.json");
  const t = (templates?.templates || templates || []).find((x: any) => x.id === id);
  if (!t) return NextResponse.json([], { status: 200 });

  const mods: string[] = (t.preinstalledModules || t.modules || []);
  const out: any[] = [];
  for (const mid of mods) {
    const arr = await getModulePreset(mid, track);
    for (const e of arr) {
      if (!out.find(x => x.id === e.id)) out.push(e);
    }
  }
  return NextResponse.json(out, { status: 200 });
}
