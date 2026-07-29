'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { GLOBAL_ROUTES } from '@/lib/personas';
import { cn } from './ui/cn';

/**
 * The three cross-cutting destinations. Persona consoles are reached through the persona
 * switcher instead, which keeps the top-level nav to a length a judge can scan at a glance.
 *
 * The active item is marked with `aria-current="page"` as well as a colour change, so the
 * current location is available to a screen reader and not only to the eye.
 */
export function MainNav({ className }: { className?: string }) {
  const pathname = usePathname();

  return (
    <nav aria-label="Primary" className={cn('flex items-center gap-1', className)}>
      {GLOBAL_ROUTES.map((route) => {
        const active = pathname === route.href || pathname.startsWith(`${route.href}/`);
        return (
          <Link
            key={route.href}
            href={route.href}
            aria-current={active ? 'page' : undefined}
            className={cn(
              'rounded-md px-3 py-1.5 text-sm font-medium',
              'transition-colors duration-150 ease-out',
              'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea',
              active
                ? 'bg-cerulea-light text-cerulea-dark'
                : 'text-ink-muted hover:bg-surface-sunken hover:text-ink',
            )}
          >
            {route.label}
          </Link>
        );
      })}
    </nav>
  );
}
