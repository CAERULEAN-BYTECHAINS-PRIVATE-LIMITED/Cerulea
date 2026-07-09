// apps/frontend/src/lib/email.ts
import { Resend } from "resend";
const resend = new Resend(process.env.RESEND_API_KEY!);
const FROM = process.env.EMAIL_FROM!;

export async function sendPasswordResetEmail(to: string, url: string) {
  await resend.emails.send({
    from: FROM,
    to,
    subject: "Reset your Cerulea password",
    html: `<p>Click to reset your password:</p><p><a href="${url}">${url}</a></p>`,
  });
}
