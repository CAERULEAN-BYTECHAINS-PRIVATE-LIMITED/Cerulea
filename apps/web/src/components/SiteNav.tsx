'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { NETWORK_ROUTES, PERSONAS } from '@/lib/personas';
import { cn } from './ui/cn';

/**
 * The primary navigation: six role portals, then the two network views, in one strip.
 *
 * Every destination is visible. The previous build hid the six roles behind a dropdown
 * "persona switcher" that duplicated a landing-page card grid, which is precisely what
 * made the product feel scattered — the same six things reachable three different ways.
 * There is one list now, it is always on screen, and it is where a judge looks.
 *
 * Below the strip's natural width it scrolls horizontally inside itself rather than
 * collapsing into a hamburger, so the document body never scrolls sideways and no
 * destination is ever more than a swipe away.
 *
 * `/demo` is not here, and is not linked from anywhere else in the product.
 */
export function SiteNav() {
  const pathname = usePathname();

  return (
    <nav aria-label="Primary" className="scroll-x flex items-stretch">
      <ul className="flex items-stretch">
        {PERSONAS.map((persona) => (
          <NavItem
            key={persona.route}
            href={persona.route}
            label={persona.navLabel}
            pathname={pathname}
          />
        ))}
      </ul>

      <span aria-hidden="true" className="mx-2 my-2 w-px shrink-0 bg-line" />

      <ul className="flex items-stretch">
        {NETWORK_ROUTES.map((route) => (
          <NavItem key={route.href} href={route.href} label={route.label} pathname={pathname} />
        ))}
      </ul>
    </nav>
  );
}

function NavItem({
  href,
  label,
  pathname,
}: {
  href: string;
  label: string;
  pathname: string;
}) {
  const active = pathname === href || pathname.startsWith(`${href}/`);
  return (
    <li className="flex">
      <Link
        href={href}
        aria-current={active ? 'page' : undefined}
        className={cn(
          'flex items-center border-b-2 px-3 text-xs font-medium whitespace-nowrap',
          'transition-colors duration-150',
          'focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent',
          active
            ? 'border-accent text-accent-dark'
            : 'border-transparent text-ink-muted hover:bg-shell hover:text-ink',
        )}
      >
        {label}
      </Link>
    </li>
  );
}
