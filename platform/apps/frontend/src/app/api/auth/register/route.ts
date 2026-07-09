// apps/frontend/src/app/api/auth/register/route.ts
import { NextResponse } from "next/server";
import { db } from "@/db/client";
import { users, profiles } from "@/db/schema";
import { eq } from "drizzle-orm";
import { z } from "zod";
import { hashPassword } from "@/lib/passwords";
import { randomUUID } from "crypto";

const RegisterSchema = z.object({
  email: z.string().email(),
  password: z.string().min(8),
  name: z.string().min(2).max(80),
  company: z.string().optional(),
  role: z.string().optional(),
});

export async function POST(req: Request) {
  const body = await req.json();
  const parsed = RegisterSchema.safeParse(body);
  if (!parsed.success) return NextResponse.json({ error: "Invalid inputs" }, { status: 400 });

  const email = parsed.data.email.toLowerCase();
  const [existing] = await db.select().from(users).where(eq(users.email, email));
  if (existing) return NextResponse.json({ error: "Email already in use" }, { status: 409 });

  const hashed = await hashPassword(parsed.data.password);
  const userId = randomUUID();
  const [newUser] = await db.insert(users)
    .values({ id: userId, email, hashedPassword: hashed, name: parsed.data.name })
    .returning({ id: users.id });

  await db.insert(profiles).values({
    id: randomUUID(),
    userId: newUser.id,
    displayName: parsed.data.name,
    company: parsed.data.company,
    role: parsed.data.role,
  });

  return NextResponse.json({ ok: true });
}
