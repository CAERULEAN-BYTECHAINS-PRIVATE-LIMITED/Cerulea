// Hard sign-out: expires the NextAuth session cookie directly via response headers.
// Used as a fallback when signOut() doesn't clear the cookie (e.g. httpOnly cookie issues).
// GET /api/auth/force-signout?next=/dashboard
import { NextRequest, NextResponse } from 'next/server';

const COOKIE_NAMES = [
  'next-auth.session-token',
  '__Secure-next-auth.session-token',
  'next-auth.csrf-token',
  '__Host-next-auth.csrf-token',
];

export async function GET(req: NextRequest) {
  const next = req.nextUrl.searchParams.get('next') || '/';
  const cookieDomain = process.env.AUTH_COOKIE_DOMAIN;

  const headers = new Headers();
  headers.set('Location', next);

  COOKIE_NAMES.forEach((name) => {
    const base = `${name}=; Max-Age=0; Path=/; HttpOnly; SameSite=Lax`;
    headers.append('Set-Cookie', cookieDomain ? `${base}; Domain=${cookieDomain}` : base);
    // Also clear without domain in case host-specific cookie exists
    headers.append('Set-Cookie', base);
  });

  return new NextResponse(null, { status: 302, headers });
}
