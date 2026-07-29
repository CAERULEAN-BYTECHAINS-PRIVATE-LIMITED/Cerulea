'use client';

import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { Check, ChevronDown, UserRound } from 'lucide-react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { PERSONAS, personaFromPathname } from '@/lib/personas';
import { cn } from './ui/cn';

/**
 * Switches between the six role consoles.
 *
 * This is a demo-mode role selector, not authentication — it says so on its face, because
 * a government audience should never be left guessing whether a login was implied.
 *
 * Keyboard behaviour follows the ARIA menu-button pattern: Down/Up open the menu and move
 * between items, Home/End jump to the ends, Escape closes and returns focus to the button,
 * and a click outside dismisses it.
 */
export function PersonaSwitcher({ className }: { className?: string }) {
  const pathname = usePathname();
  const menuId = useId();
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const itemRefs = useRef<(HTMLAnchorElement | null)[]>([]);
  const reduceMotion = useReducedMotion();

  const current = personaFromPathname(pathname);

  useEffect(() => {
    if (!open) return;

    function onPointerDown(event: MouseEvent) {
      if (!containerRef.current?.contains(event.target as Node)) setOpen(false);
    }
    document.addEventListener('mousedown', onPointerDown);
    return () => document.removeEventListener('mousedown', onPointerDown);
  }, [open]);

  useEffect(() => {
    if (open) itemRefs.current[activeIndex]?.focus();
  }, [open, activeIndex]);

  function openAt(index: number) {
    setActiveIndex(index);
    setOpen(true);
  }

  function onButtonKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (event.key === 'ArrowDown' || event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openAt(current ? PERSONAS.findIndex((persona) => persona.id === current.id) : 0);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      openAt(PERSONAS.length - 1);
    }
  }

  function onMenuKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.preventDefault();
      setOpen(false);
      buttonRef.current?.focus();
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActiveIndex((index) => (index + 1) % PERSONAS.length);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActiveIndex((index) => (index - 1 + PERSONAS.length) % PERSONAS.length);
    } else if (event.key === 'Home') {
      event.preventDefault();
      setActiveIndex(0);
    } else if (event.key === 'End') {
      event.preventDefault();
      setActiveIndex(PERSONAS.length - 1);
    } else if (event.key === 'Tab') {
      setOpen(false);
    }
  }

  const CurrentIcon = current?.icon ?? UserRound;

  return (
    <div ref={containerRef} className={cn('relative', className)}>
      <button
        ref={buttonRef}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => (open ? setOpen(false) : openAt(0))}
        onKeyDown={onButtonKeyDown}
        className={cn(
          'inline-flex items-center gap-2 rounded-lg border border-border bg-surface py-1.5 pr-2 pl-2.5',
          'text-sm font-medium text-ink shadow-sm',
          'transition-colors duration-150 ease-out hover:bg-surface-sunken',
          'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea',
        )}
      >
        <CurrentIcon className="size-4 text-cerulea" aria-hidden="true" />
        <span className="max-w-40 truncate">
          {current && <span className="sr-only">Viewing as </span>}
          {current?.shortLabel ?? 'Choose a role'}
        </span>
        <ChevronDown
          className={cn('size-4 text-ink-muted transition-transform duration-200', open && 'rotate-180')}
          aria-hidden="true"
        />
      </button>

      <AnimatePresence>
        {open && (
          <motion.div
            id={menuId}
            role="menu"
            aria-label="Switch role"
            onKeyDown={onMenuKeyDown}
            initial={reduceMotion ? { opacity: 0 } : { opacity: 0, y: -4, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, y: -4, scale: 0.98 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            className="absolute right-0 z-40 mt-2 w-80 origin-top-right overflow-hidden rounded-card border border-border bg-surface shadow-lg"
          >
            <p className="border-b border-border bg-surface-sunken px-3 py-2 text-[0.6875rem] font-semibold tracking-wide text-ink-muted uppercase">
              Demo mode — role selection, not sign-in
            </p>

            <ul className="max-h-[70vh] overflow-y-auto py-1">
              {PERSONAS.map((persona, index) => {
                const selected = current?.id === persona.id;
                const Icon = persona.icon;
                return (
                  <li key={persona.id}>
                    <Link
                      ref={(node) => {
                        itemRefs.current[index] = node;
                      }}
                      href={persona.route}
                      role="menuitemradio"
                      aria-checked={selected}
                      tabIndex={-1}
                      onMouseEnter={() => setActiveIndex(index)}
                      onClick={() => setOpen(false)}
                      className={cn(
                        'flex w-full items-start gap-3 px-3 py-2.5 text-left',
                        'transition-colors duration-150 ease-out',
                        'focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-cerulea',
                        index === activeIndex ? 'bg-surface-sunken' : 'bg-surface',
                      )}
                    >
                      <Icon className="mt-0.5 size-4 shrink-0 text-cerulea" aria-hidden="true" />
                      <span className="min-w-0 flex-1">
                        <span className="block text-sm font-medium text-ink">{persona.label}</span>
                        <span className="mt-0.5 block text-xs leading-snug text-ink-muted">
                          {persona.actingAs}
                        </span>
                      </span>
                      {selected && (
                        <Check className="mt-0.5 size-4 shrink-0 text-cerulea" aria-hidden="true" />
                      )}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
