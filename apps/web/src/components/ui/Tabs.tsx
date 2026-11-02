'use client';

import { useId, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { cn } from './cn';

export interface TabItem {
  id: string;
  label: ReactNode;
  /** Optional count shown after the label. */
  meta?: ReactNode;
  content: ReactNode;
  disabled?: boolean;
}

/**
 * A segmented control, built to the WAI-ARIA authoring practice by hand.
 *
 * - `tablist` / `tab` / `tabpanel` roles with `aria-selected`, `aria-controls`
 * - roving tabindex: exactly one tab is in the tab order, arrows move between them
 * - Left/Right wrap, Home/End jump to the ends, disabled tabs are skipped
 * - the panel is focusable so keyboard users can reach its content
 *
 * The selected tab is a filled block rather than an underline: on a projector an
 * underline two pixels tall is the first thing to disappear.
 */
export function Tabs({
  items,
  defaultTabId,
  value,
  onValueChange,
  className,
}: {
  items: TabItem[];
  defaultTabId?: string;
  /** Provide with `onValueChange` to control the selection from outside. */
  value?: string;
  onValueChange?: (id: string) => void;
  className?: string;
}) {
  const baseId = useId();
  const [internal, setInternal] = useState(defaultTabId ?? items[0]?.id);
  const active = value ?? internal;
  const tabRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  function select(id: string) {
    if (value === undefined) setInternal(id);
    onValueChange?.(id);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const enabled = items.filter((item) => !item.disabled);
    if (enabled.length === 0) return;
    const current = enabled.findIndex((item) => item.id === active);
    let next = -1;

    if (event.key === 'ArrowRight') next = (current + 1) % enabled.length;
    else if (event.key === 'ArrowLeft') next = (current - 1 + enabled.length) % enabled.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = enabled.length - 1;
    else return;

    event.preventDefault();
    select(enabled[next].id);
    tabRefs.current[enabled[next].id]?.focus();
  }

  const activeItem = items.find((item) => item.id === active);

  return (
    <div className={className}>
      <div
        role="tablist"
        onKeyDown={onKeyDown}
        className="scroll-x flex w-fit max-w-full rounded-md border border-line bg-paper p-0.5"
      >
        {items.map((item) => {
          const selected = item.id === active;
          return (
            <button
              key={item.id}
              ref={(node) => {
                tabRefs.current[item.id] = node;
              }}
              type="button"
              role="tab"
              id={`${baseId}-tab-${item.id}`}
              aria-selected={selected}
              aria-controls={`${baseId}-panel-${item.id}`}
              tabIndex={selected ? 0 : -1}
              disabled={item.disabled}
              onClick={() => select(item.id)}
              className={cn(
                'flex shrink-0 items-center gap-1.5 rounded-sm px-3 py-1.5 text-xs font-medium',
                'transition-colors duration-150',
                'focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent',
                'disabled:cursor-not-allowed disabled:opacity-40',
                selected ? 'bg-accent text-white' : 'text-ink-muted hover:bg-shell hover:text-ink',
              )}
            >
              {item.label}
              {item.meta && (
                <span
                  className={cn(
                    'font-mono text-2xs',
                    selected ? 'text-white/75' : 'text-ink-subtle',
                  )}
                >
                  {item.meta}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {activeItem && (
        <div
          role="tabpanel"
          id={`${baseId}-panel-${activeItem.id}`}
          aria-labelledby={`${baseId}-tab-${activeItem.id}`}
          tabIndex={0}
          className="mt-4 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
        >
          {activeItem.content}
        </div>
      )}
    </div>
  );
}
