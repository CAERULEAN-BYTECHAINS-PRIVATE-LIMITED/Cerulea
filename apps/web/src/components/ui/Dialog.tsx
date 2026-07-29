'use client';

import { X } from 'lucide-react';
import { useEffect, useRef, type ReactNode } from 'react';
import { cn } from './cn';

/**
 * A modal built on the native `<dialog>` element.
 *
 * `showModal()` puts the dialog in the browser's top layer, which means it escapes every
 * `overflow: hidden` and stacking context on the page for free, traps focus without a
 * hand-rolled trap, and closes on Escape without a key handler. The alternative — a fixed
 * div with a manual focus trap — is the version that goes wrong in front of an audience.
 *
 * Modals are used sparingly here: exactly one, for issuing a certificate, where the action
 * is a discrete legal act with its own inputs and its own record.
 */
export function Dialog({
  open,
  onClose,
  title,
  subtitle,
  footer,
  size = 'md',
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: ReactNode;
  footer?: ReactNode;
  size?: 'md' | 'lg';
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    if (open && !node.open) node.showModal();
    if (!open && node.open) node.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClose={onClose}
      // Clicking the backdrop — the dialog element's own box outside its content — closes.
      onClick={(event) => {
        if (event.target === ref.current) onClose();
      }}
      aria-labelledby="dialog-title"
      className={cn(
        'm-auto w-[calc(100vw-2rem)] rounded-md border border-line-strong bg-paper p-0 text-ink shadow-modal',
        'backdrop:bg-masthead/55',
        size === 'lg' ? 'max-w-3xl' : 'max-w-xl',
      )}
    >
      <div className="flex items-start justify-between gap-4 border-b border-line bg-shell px-4 py-2.5">
        <div className="min-w-0">
          <h2 id="dialog-title" className="text-sm font-semibold text-ink">
            {title}
          </h2>
          {subtitle && <p className="mt-0.5 text-2xs text-ink-muted">{subtitle}</p>}
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close"
          className="-mr-1 shrink-0 rounded-sm p-1 text-ink-muted transition-colors duration-150 hover:bg-shell-2 hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
        >
          <X className="size-4" aria-hidden="true" />
        </button>
      </div>

      <div className="max-h-[70vh] overflow-y-auto px-4 py-4">{children}</div>

      {footer && (
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-line bg-shell px-4 py-2.5">
          {footer}
        </div>
      )}
    </dialog>
  );
}
