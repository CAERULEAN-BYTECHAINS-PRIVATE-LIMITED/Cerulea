// apps/frontend/src/middleware.ts
import type { NextRequest } from 'next/server';
import { NextResponse } from 'next/server';
import { getToken } from 'next-auth/jwt';

const PUBLIC_PATHS = [
  '/auth/',
  '/pricing',
  '/api/',
  '/_next/',
  '/favicon.ico',
];

function isPublicPath(pathname: string): boolean {
  return PUBLIC_PATHS.some((p) => pathname.startsWith(p));
}

export async function middleware(req: NextRequest) {
  const host = req.headers.get('host') || '';
  const { pathname } = req.nextUrl;

  const isStudioHost =
    host.startsWith('studio.') ||
    host === 'studio.localhost:3000' ||
    host === 'studio.localhost';

  const isAdminHost =
    host.startsWith('control.') ||
    host === 'control.localhost:3000' ||
    host === 'control.localhost';

  // Always allow public paths through (auth, pricing, api, _next)
  if (isPublicPath(pathname)) {
    if (isStudioHost) {
      const url = req.nextUrl.clone();
      if (!url.searchParams.has('studio')) url.searchParams.set('studio', '1');
      return NextResponse.rewrite(url);
    }
    if (isAdminHost) {
      const url = req.nextUrl.clone();
      if (!url.searchParams.has('admin')) url.searchParams.set('admin', '1');
      return NextResponse.rewrite(url);
    }
    return NextResponse.next();
  }

  // Auth check — applies to all hosts
  const token = await getToken({ req, secret: process.env.NEXTAUTH_SECRET });

  if (!token) {
    // Build login URL (same host — cross-subdomain cookie sharing is unreliable on localhost;
    // on production AUTH_COOKIE_DOMAIN=.cerulea.app handles subdomain sharing via Vercel env vars).
    const loginUrl = req.nextUrl.clone();
    loginUrl.searchParams.set('next', pathname);
    loginUrl.pathname = '/auth/login';

    // If stale session cookies exist but are invalid (token was null despite cookies present),
    // route through force-signout first to wipe them — otherwise the login form ends up in a
    // redirect loop because the old encrypted JWT confuses getToken on every subsequent request.
    const hasSessionCookie =
      req.cookies.has('next-auth.session-token') ||
      req.cookies.has('__Secure-next-auth.session-token');

    if (hasSessionCookie) {
      const signoutUrl = req.nextUrl.clone();
      signoutUrl.pathname = '/api/auth/force-signout';
      signoutUrl.search = '';
      // Use the full login URL so the Location header in force-signout is unambiguous
      // (avoids double-encoding issues with nested query params)
      signoutUrl.searchParams.set('next', loginUrl.toString());
      return NextResponse.redirect(signoutUrl);
    }

    return NextResponse.redirect(loginUrl);
  }

  // Admin subdomain: only allow test@cerulea.app / isTestAccount
  if (isAdminHost) {
    const isAdminUser =
      (token.email as string) === 'test@cerulea.app' ||
      (token.isTestAccount as boolean) === true;

    if (!isAdminUser) {
      // Redirect non-admin users back to main site
      const mainUrl = req.nextUrl.clone();
      mainUrl.host = host.replace(/^control\./, '');
      mainUrl.pathname = '/dashboard';
      mainUrl.search = '';
      return NextResponse.redirect(mainUrl);
    }

    // Rewrite admin subdomain paths to the (admin) route group
    const url = req.nextUrl.clone();
    if (!url.searchParams.has('admin')) url.searchParams.set('admin', '1');
    // Map /  -> /admin, /users -> /admin/users, etc.
    if (pathname === '/') {
      url.pathname = '/admin';
    } else if (!pathname.startsWith('/admin')) {
      url.pathname = `/admin${pathname}`;
    }
    return NextResponse.rewrite(url);
  }

  // Test accounts bypass pricing gate
  if (!(token.isTestAccount as boolean)) {
    const plan = token.plan as string | undefined;
    if ((!plan || plan === 'free') && pathname !== '/pricing') {
      const pricingUrl = req.nextUrl.clone();
      // Only redirect to the main domain in production where AUTH_COOKIE_DOMAIN shares
      // the session cookie cross-subdomain. In dev, stay on the same host so the cookie
      // remains readable and the user sees their account info on the pricing page.
      if (isStudioHost && process.env.NODE_ENV === 'production') {
        pricingUrl.host = host.replace(/^studio\./, '');
      }
      pricingUrl.pathname = '/pricing';
      return NextResponse.redirect(pricingUrl);
    }
  }

  // Apply studio subdomain rewrite AFTER auth/pricing checks pass
  if (isStudioHost) {
    const url = req.nextUrl.clone();
    if (!url.searchParams.has('studio')) url.searchParams.set('studio', '1');
    return NextResponse.rewrite(url);
  }

  return NextResponse.next();
}

export const config = {
  matcher: ['/((?!api|_next/static|_next/image|favicon.ico).*)'],
};
