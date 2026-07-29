'use client';

import { useId, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { cn } from './cn';

export interface TabItem {
  id: string;
  label: ReactNode;
  /** Optional count or badge shown after the label. */
  meta?: ReactNode;
  content: ReactNode;
  disabled?: boolean;
}

/**
 * Tabs built to the WAI-ARIA authoring practice, by hand — Radix is not a dependency here.
 *
 * - `tablist` / `tab` / `tabpanel` roles with `aria-selected`, `aria-controls`, `aria-labelledby`
 * - roving tabindex: exactly one tab is in the tab order, arrows move between them
 * - Left/Right wrap, Home/End jump to the ends, disabled tabs are skipped
 * - the panel itself is focusable (`tabIndex={0}`) so keyboard users can reach its content
 */
export function Tabs({
  items,
  defaultTabId,
  value,
  onValueChange,
  className,
  listClassName,
  panelClassName,
}: {
  items: TabItem[];
  defaultTabId?: string;
  /** Provide with `onValueChange` to control the selection from outside. */
  value?: string;
  onValueChange?: (id: string) => void;
  className?: string;
  listClassName?: string;
  panelClassName?: string;
}) {
  const baseId = useId();
  const [internal, setInternal] = useState(defaultTabId ?? items[0]?.id);
  const active = value ?? internal;
  const tabRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  function select(id: string) {
    if (value === undefined) setInternal(id);
    onValueChange?.(id);
  }

  function focusTab(id: string) {
    select(id);
    tabRefs.current[id]?.focus();
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
    focusTab(enabled[next].id);
  }

  const activeItem = items.find((item) => item.id === active);

  return (
    <div className={className}>
      <div
        role="tablist"
        onKeyDown={onKeyDown}
        className={cn('flex gap-1 overflow-x-auto border-b border-border', listClassName)}
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
                'relative -mb-px flex shrink-0 items-center gap-2 border-b-2 px-4 py-2.5 text-sm font-medium',
                'transition-colors duration-150 ease-out',
                'focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-cerulea',
                'disabled:cursor-not-allowed disabled:opacity-40',
                selected
                  ? 'border-cerulea text-cerulea-dark'
                  : 'border-transparent text-ink-muted hover:border-border hover:text-ink',
              )}
            >
              {item.label}
              {item.meta}
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
          className={cn(
            'pt-5 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea',
            panelClassName,
          )}
        >
          {activeItem.content}
        </div>
      )}
    </div>
  );
}
