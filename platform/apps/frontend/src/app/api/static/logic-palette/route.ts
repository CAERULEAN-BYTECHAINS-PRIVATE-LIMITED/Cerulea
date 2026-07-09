import { NextRequest, NextResponse } from "next/server";
import palette from "@/data/logic-palette.seed.json";

export const dynamic = "force-static";
export async function GET(req: NextRequest) {
  const url = new URL(req.url);
  const track = (url.searchParams.get("track") || "dapp") as "dapp"|"blockchain";
  const items = (palette as any)[track] || [];
  return NextResponse.json({ items });
}
