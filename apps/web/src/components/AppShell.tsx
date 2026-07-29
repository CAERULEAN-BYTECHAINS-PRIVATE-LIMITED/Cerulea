import Link from 'next/link';
import type { ReactNode } from 'react';
import { ChainStatus } from './ChainStatus';
import { MainNav } from './MainNav';
import { PersonaSwitcher } from './PersonaSwitcher';
import { cn } from './ui/cn';
import { Wordmark } from './Wordmark';

/**
 * The frame every console sits in.
 *
 * A Server Component on purpose — only the three genuinely interactive pieces (nav
 * highlighting, the role switcher, the connection poller) cross the client boundary, so a
 * persona page's own server-rendered content streams without waiting on them.
 */
export function AppShell({
  title,
  subtitle,
  eyebrow,
  actions,
  children,
  width = 'default',
}: {
  title?: ReactNode;
  subtitle?: ReactNode;
  /** Small label above the title, e.g. the persona's institution. */
  eyebrow?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  /** `wide` for tables and the explorer; `default` for forms and detail views. */
  width?: 'default' | 'wide';
}) {
  const container = cn(
    'mx-auto w-full px-4 sm:px-6 lg:px-8',
    width === 'wide' ? 'max-w-[1600px]' : 'max-w-7xl',
  );

  return (
    <div className="flex min-h-screen flex-col">
      <a
        href="#main"
        className="sr-only rounded-md bg-cerulea px-4 py-2 text-sm font-medium text-white focus:not-sr-only focus:absolute focus:top-3 focus:left-3 focus:z-50"
      >
        Skip to main content
      </a>

      <header className="sticky top-0 z-30 border-b border-border bg-surface/95 backdrop-blur-sm">
        <div className={cn(container, 'flex h-16 items-center justify-between gap-4')}>
          <div className="flex min-w-0 items-center gap-6">
            <Link
              href="/"
              className="rounded focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea"
            >
              <Wordmark />
            </Link>
            <MainNav className="hidden md:flex" />
          </div>

          <div className="flex shrink-0 items-center gap-3">
            <ChainStatus className="hidden lg:inline-flex" />
            <PersonaSwitcher />
          </div>
        </div>

        {/* Below `lg` the brand row runs out of room, so the connection indicator moves to
            a second row — and below `md` the nav joins it rather than collapsing into a
            hamburger, since three destinations do not warrant hiding them behind a tap. */}
        <div className={cn(container, 'flex items-center gap-4 pb-2 lg:hidden')}>
          <MainNav className="md:hidden" />
          <ChainStatus className="ml-auto" />
        </div>
      </header>

      {(title || eyebrow || actions) && (
        <div className="border-b border-border bg-surface">
          <div className={cn(container, 'flex flex-wrap items-end justify-between gap-4 py-6')}>
            <div className="min-w-0">
              {eyebrow && (
                <p className="text-xs font-semibold tracking-wide text-cerulea uppercase">
                  {eyebrow}
                </p>
              )}
              {title && (
                <h1 className="mt-1 text-2xl font-semibold tracking-tight text-ink">{title}</h1>
              )}
              {subtitle && <p className="mt-1.5 max-w-3xl text-sm text-ink-muted">{subtitle}</p>}
            </div>
            {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
          </div>
        </div>
      )}

      <main id="main" className="flex-1 py-8">
        <div className={container}>{children}</div>
      </main>

      <SiteFooter className={container} />
    </div>
  );
}

export function SiteFooter({ className }: { className?: string }) {
  return (
    <footer className="border-t border-border bg-surface">
      <div
        className={cn(
          className,
          'flex flex-wrap items-center justify-between gap-x-8 gap-y-2 py-6 text-xs text-ink-muted',
        )}
      >
        <p>
          CBC-PRAMAAN — proof-of-concept compliance infrastructure for the Public Procurement
          (Preference to Make in India) Order.
        </p>
        <p className="font-mono">
          Cerulea Chain · three-validator DCF network · decisions returned only after finality
        </p>
      </div>
    </footer>
  );
}
