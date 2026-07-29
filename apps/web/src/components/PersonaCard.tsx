import { ArrowRight } from 'lucide-react';
import Link from 'next/link';
import type { Persona } from '@/lib/personas';
import { cn } from './ui/cn';

/**
 * One role, presented as an entry point rather than a menu row.
 *
 * The whole card is a single link — one target, one focus stop — so keyboard and screen
 * reader users get the same one-action-per-role model the mouse does.
 */
export function PersonaCard({ persona, className }: { persona: Persona; className?: string }) {
  const Icon = persona.icon;

  return (
    <Link
      href={persona.route}
      className={cn(
        'group flex h-full flex-col rounded-card border border-border bg-surface p-5',
        'shadow-[0_1px_2px_rgba(26,26,26,0.04)]',
        'transition-[border-color,box-shadow,transform] duration-200 ease-out',
        'hover:-translate-y-0.5 hover:border-cerulea/40 hover:shadow-md',
        'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea',
        className,
      )}
    >
      <span className="flex size-10 items-center justify-center rounded-lg bg-cerulea-light text-cerulea transition-colors duration-200 group-hover:bg-cerulea group-hover:text-white">
        <Icon className="size-5" aria-hidden="true" />
      </span>

      <h3 className="mt-4 text-base font-semibold text-ink">{persona.label}</h3>
      <p className="mt-1.5 flex-1 text-sm leading-relaxed text-ink-muted">{persona.summary}</p>

      <span className="mt-4 flex items-center justify-between gap-3 border-t border-border pt-3">
        <span className="min-w-0 truncate text-xs text-ink-subtle">{persona.actingAs}</span>
        <span className="inline-flex shrink-0 items-center gap-1 text-sm font-medium text-cerulea">
          Open
          <ArrowRight
            className="size-4 transition-transform duration-200 ease-out group-hover:translate-x-0.5"
            aria-hidden="true"
          />
        </span>
      </span>
    </Link>
  );
}
