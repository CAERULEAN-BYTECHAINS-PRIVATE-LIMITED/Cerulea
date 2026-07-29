import Link from 'next/link';
import type { ReactNode } from 'react';
import { ChainStatus } from './ChainStatus';
import { SiteNav } from './SiteNav';
import { cn } from './ui/cn';
import { Wordmark } from './Wordmark';

/**
 * The one frame every route sits in. There are no variants and no per-page chrome.
 *
 * Four bands, top to bottom:
 *
 *   1. MASTHEAD — dark, 40px. Identity on the left, network state on the right. This is
 *      the only large dark area in the product and it is what makes the page read as a
 *      government portal rather than an application dashboard.
 *   2. NAVIGATION — white, 40px. Every destination, always visible.
 *   3. PAGE HEAD — white. Breadcrumb, title, and the identity this console is acting as.
 *      One line of `note` is permitted and is usually not used.
 *   4. CONTENT — on the shell tint, so every panel inside reads as a sheet on a desk.
 *
 * A Server Component: only the three genuinely interactive pieces (nav highlighting, the
 * connection poller) cross the client boundary, so a page's own content streams without
 * waiting on them.
 */
export function AppShell({
  breadcrumb,
  title,
  actingAs,
  note,
  actions,
  children,
  width = 'default',
}: {
  /** Where this page sits. Two levels at most: "Role portals", "Network". */
  breadcrumb?: string;
  title: string;
  /** The organisation the console is signed in as. Right-aligned beside the title. */
  actingAs?: string;
  /** One clause, only where the title alone would mislead. Not a description. */
  note?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  /** `wide` for tables and the explorer; `default` for forms and record views. */
  width?: 'default' | 'wide';
}) {
  const container = cn(
    'mx-auto w-full px-4 lg:px-6',
    width === 'wide' ? 'max-w-[1680px]' : 'max-w-[1280px]',
  );

  return (
    <div className="flex min-h-screen flex-col">
      <a
        href="#main"
        className="sr-only rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-white focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-40"
      >
        Skip to main content
      </a>

      {/* 1 — Masthead */}
      <div className="bg-masthead">
        <div className={cn(container, 'flex h-10 items-center justify-between gap-4')}>
          <div className="flex min-w-0 items-center gap-3">
            <Link
              href="/"
              className="rounded-sm focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-white"
            >
              <Wordmark />
            </Link>
            <span aria-hidden="true" className="hidden h-4 w-px bg-white/20 sm:block" />
            <p className="hidden truncate text-2xs text-white/60 sm:block">
              Make in India compliance verification · Proof of concept
            </p>
          </div>
          <ChainStatus className="shrink-0" />
        </div>
      </div>

      {/* 2 — Navigation */}
      <div className="sticky top-0 z-20 border-b border-line bg-paper">
        <div className={cn(container, 'flex h-10 items-stretch')}>
          <SiteNav />
        </div>
      </div>

      {/* 3 — Page head */}
      <header className="border-b border-line bg-paper">
        <div className={cn(container, 'flex flex-wrap items-end justify-between gap-x-6 gap-y-2 py-3')}>
          <div className="min-w-0">
            {breadcrumb && (
              <p className="text-2xs font-medium tracking-wide text-ink-muted uppercase">
                {breadcrumb}
              </p>
            )}
            <h1 className="mt-0.5 text-xl font-semibold tracking-tight text-ink">{title}</h1>
            {note && <p className="mt-1 max-w-3xl text-xs text-ink-muted">{note}</p>}
          </div>

          <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2">
            {actingAs && (
              <p className="text-2xs text-ink-muted">
                <span className="tracking-wide uppercase">Acting as</span>{' '}
                <span className="font-medium text-ink">{actingAs}</span>
              </p>
            )}
            {actions}
          </div>
        </div>
      </header>

      {/* 4 — Content */}
      <main id="main" className="flex-1 py-5">
        <div className={container}>{children}</div>
      </main>

      <footer className="border-t border-line bg-paper">
        <div
          className={cn(
            container,
            'flex flex-wrap items-center justify-between gap-x-6 gap-y-1 py-3 text-2xs text-ink-muted',
          )}
        >
          <p>
            Public Procurement (Preference to Make in India) Order, 2017 —
            P-45021/2/2017-PP(BE-II), as amended 19.07.2024.
          </p>
          <p className="font-mono">Cerulea · three-validator DCF network</p>
        </div>
      </footer>
    </div>
  );
}
