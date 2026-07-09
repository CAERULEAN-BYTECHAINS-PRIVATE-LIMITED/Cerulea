import { NextResponse } from "next/server";
import { randomUUID } from "crypto";
import { db } from "@/db/client";
import { users, verificationTokens } from "@/db/schema";
import { eq } from "drizzle-orm";

/**
 * IMPORTANT:
 * - Email sending is OPTIONAL.
 * - This route MUST NOT fail at build time.
 * - If RESEND_API_KEY is missing, we safely skip email sending.
 */

async function sendResetEmail(email: string, token: string) {
  const apiKey = process.env.RESEND_API_KEY;

  // If no email provider configured, just skip
  if (!apiKey) {
    console.warn(
      "[forgot-password] RESEND_API_KEY not set. Skipping email send."
    );
    return;
  }

  // Dynamic import so build never crashes
  const { Resend } = await import("resend");
  const resend = new Resend(apiKey);

  const resetUrl = `${process.env.NEXTAUTH_URL ?? ""}/auth/reset-password?token=${token}`;

  await resend.emails.send({
    from: "no-reply@cerulea.app",
    to: email,
    subject: "Reset your Cerulea password",
    html: `
      <p>You requested a password reset.</p>
      <p>
        <a href="${resetUrl}">
          Click here to reset your password
        </a>
      </p>
      <p>If you did not request this, you can safely ignore this email.</p>
    `,
  });
}

export async function POST(req: Request) {
  try {
    const body = await req.json();
    const email = body?.email;

    if (!email) {
      return NextResponse.json(
        { error: "Email is required." },
        { status: 400 }
      );
    }

    const user = await db
      .select()
      .from(users)
      .where(eq(users.email, email))
      .get();

    // Always respond success (security best practice)
    if (!user) {
      return NextResponse.json({ success: true });
    }

    const token = randomUUID();
    const expiresAt = new Date(Date.now() + 1000 * 60 * 60).toISOString(); // 1 hour

    await db.insert(verificationTokens).values({
      id: randomUUID(),
      identifier: email,
      token,
      expiresAt,
    });

    // Email send is best-effort only
    await sendResetEmail(email, token);

    return NextResponse.json({ success: true });
  } catch (err) {
    console.error("[forgot-password]", err);
    return NextResponse.json(
      { error: "Internal server error" },
      { status: 500 }
    );
  }
}
