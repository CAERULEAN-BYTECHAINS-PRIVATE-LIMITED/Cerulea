// apps/frontend/src/app/api/auth/reset-password/route.ts
import { NextResponse } from "next/server";
import { db } from "@/db/client";
import { users, verificationTokens } from "@/db/schema";
import { and, eq } from "drizzle-orm";
import { hashPassword } from "@/lib/passwords";

export async function POST(req: Request) {
  const { token, password } = await req.json();
  if (!token || !password) return NextResponse.json({ error: "Invalid" }, { status: 400 });

  const now = new Date();
  const [row] = await db.select().from(verificationTokens)
    .where(eq(verificationTokens.token, token));
  if (!row || new Date(row.expiresAt) < now || !row.identifier.startsWith("pwd:")) {
    return NextResponse.json({ error: "Token invalid or expired" }, { status: 400 });
  }
  const userId = row.identifier.replace("pwd:", "");

  await db.update(users)
    .set({ hashedPassword: await hashPassword(password) })
    .where(eq(users.id, userId));

  // burn token
  await db.delete(verificationTokens).where(and(
    eq(verificationTokens.identifier, row.identifier),
    eq(verificationTokens.token, token),
  ));

  return NextResponse.json({ ok: true });
}
