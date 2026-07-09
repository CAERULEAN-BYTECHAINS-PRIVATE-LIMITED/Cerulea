import { NextResponse } from "next/server";
import presets from "@/data/module-entity-presets.json";
export const dynamic = "force-static";
export async function GET() { return NextResponse.json(presets); }
