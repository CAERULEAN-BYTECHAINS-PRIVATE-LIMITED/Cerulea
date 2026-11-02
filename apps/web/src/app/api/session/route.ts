/**
 * Browser sign-in for the persona consoles.
 *
 *   GET    -> { protected, authenticated, mode }
 *   POST   { password } -> sets the session cookie when the demo access code matches
 *   DELETE -> clears the cookie
 *
 * See src/lib/auth.ts for what the cookie is and why an API key is the alternative.
 */

import {
  accessControlEnabled,
  authorize,
  passwordMatches,
  sessionCookieHeader,
  sessionCookieValue,
} from '@/lib/auth';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function GET(request: Request): Promise<Response> {
  const isProtected = accessControlEnabled();
  const auth = authorize(request);
  return Response.json({ protected: isProtected, authenticated: auth.ok, mode: auth.mode });
}

export async function POST(request: Request): Promise<Response> {
  if (!accessControlEnabled()) {
    return Response.json({ protected: false, authenticated: true, mode: 'open' });
  }
  let password = '';
  try {
    const body = (await request.json()) as { password?: unknown };
    password = typeof body.password === 'string' ? body.password : '';
  } catch {
    return Response.json(
      { error: 'Request body must be JSON with a password field.' },
      { status: 400 },
    );
  }
  if (!passwordMatches(password)) {
    return Response.json({ error: 'Incorrect access code.' }, { status: 401 });
  }
  return new Response(
    JSON.stringify({ protected: true, authenticated: true, mode: 'session' }),
    {
      status: 200,
      headers: {
        'Content-Type': 'application/json',
        'Set-Cookie': sessionCookieHeader(sessionCookieValue()),
      },
    },
  );
}

export async function DELETE(): Promise<Response> {
  return new Response(null, {
    status: 204,
    headers: { 'Set-Cookie': sessionCookieHeader(null) },
  });
}
