// Build entity presets for a given module id at request time.
// Tries src/data first, then apps/frontend/src/data to match your pattern.
import { NextResponse } from "next/server";
import { promises as fs } from "fs";
import path from "path";

export const dynamic = "force-dynamic";

async function tryRead(p: string) {
  try { return await fs.readFile(p, "utf-8"); } catch { return null; }
}
async function loadJson(filename: string) {
  const roots = [
    path.join(process.cwd(), "src", "data", filename),
    path.join(process.cwd(), "apps", "frontend", "src", "data", filename),
  ];
  for (const p of roots) {
    const s = await tryRead(p);
    if (s) return JSON.parse(s);
  }
  return null;
}

export async function GET(req: Request) {
  const url = new URL(req.url);
  const id = url.searchParams.get("id");
  const track = url.searchParams.get("track") || "dapp";
  if (!id) return NextResponse.json([], { status: 200 });

  // optional explicit presets file
  const explicit = await loadJson("module-entity-presets.json");
  if (explicit && explicit[id]) {
    return NextResponse.json(explicit[id], { status: 200 });
  }

  // synthesize from modules.seed.json if explicit mapping missing
  const modules = await loadJson("modules.seed.json");
  const m = (modules?.modules || modules || []).find((x: any) => x.id === id);
  if (!m) return NextResponse.json([], { status: 200 });

  const baseFields = [
    { name: "id", type: "string", required: true, unique: true, indexed: true },
    { name: "createdAt", type: "datetime", indexed: true },
    { name: "updatedAt", type: "datetime", indexed: true },
  ];

  const lower = (m.name || m.id || "Entity").toString();
  const n = lower.charAt(0).toUpperCase() + lower.slice(1);

  // heuristic by tags/category
  const tags = (m.tags || m.keywords || m.categories || []).map((t: string) => t.toLowerCase());
  const f: any[] = [];

  if (tags.some((t: string) => ["auth","user","profile"].includes(t))) {
    f.push({ name: "email", type: "string", unique: true, indexed: true });
    f.push({ name: "passwordHash", type: "string" });
    f.push({ name: "role", type: "string", indexed: true });
  }
  if (tags.includes("payment") || tags.includes("order")) {
    f.push({ name: "status", type: "string", indexed: true });
    f.push({ name: "amount", type: "decimal" });
    f.push({ name: "currency", type: "string" });
  }
  if (tags.includes("nft")) {
    f.push({ name: "tokenId", type: "string", indexed: true });
    f.push({ name: "owner", type: "address", indexed: true });
    f.push({ name: "metadata", type: "json" });
  }
  if (tags.includes("erc20") || tags.includes("token")) {
    f.push({ name: "tokenAddress", type: "address", indexed: true });
    f.push({ name: "holder", type: "address", indexed: true });
    f.push({ name: "balance", type: "bigint" });
  }

  const entity = {
    id: `ent_${id}`,
    name: n,
    fields: [...baseFields, ...f],
  };

  return NextResponse.json([entity], { status: 200 });
}
