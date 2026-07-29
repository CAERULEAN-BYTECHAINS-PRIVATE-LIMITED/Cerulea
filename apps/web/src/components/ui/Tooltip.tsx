'use client';

import {
  Children,
  cloneElement,
  isValidElement,
  useId,
  useState,
  type FocusEvent,
  type KeyboardEvent,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
} from 'react';
import { cn } from './cn';

type TriggerProps = {
  onMouseEnter?: (event: MouseEvent<HTMLElement>) => void;
  onMouseLeave?: (event: MouseEvent<HTMLElement>) => void;
  onFocus?: (event: FocusEvent<HTMLElement>) => void;
  onBlur?: (event: FocusEvent<HTMLElement>) => void;
  'aria-describedby'?: string;
};

/**
 * A description tooltip.
 *
 * It clones its single child so `aria-describedby` lands on the real interactive element
 * rather than a wrapper — a screen reader reading the trigger also reads the explanation.
 * It opens on hover *and* on keyboard focus, and Escape dismisses it, so the content is
 * never mouse-only. Tooltips carry supporting detail here, never the only copy of a fact.
 */
export function Tooltip({
  content,
  children,
  side = 'top',
  className,
}: {
  content: ReactNode;
  children: ReactNode;
  side?: 'top' | 'bottom';
  className?: string;
}) {
  const id = useId();
  const [open, setOpen] = useState(false);

  const child = Children.only(children);
  if (!isValidElement<TriggerProps>(child)) return <>{children}</>;

  const element = child as ReactElement<TriggerProps>;
  const trigger = cloneElement(element, {
    'aria-describedby': open ? id : undefined,
    onMouseEnter: (event: MouseEvent<HTMLElement>) => {
      element.props.onMouseEnter?.(event);
      setOpen(true);
    },
    onMouseLeave: (event: MouseEvent<HTMLElement>) => {
      element.props.onMouseLeave?.(event);
      setOpen(false);
    },
    onFocus: (event: FocusEvent<HTMLElement>) => {
      element.props.onFocus?.(event);
      setOpen(true);
    },
    onBlur: (event: FocusEvent<HTMLElement>) => {
      element.props.onBlur?.(event);
      setOpen(false);
    },
  });

  return (
    <span
      className="relative inline-flex"
      onKeyDown={(event: KeyboardEvent<HTMLSpanElement>) => {
        if (event.key === 'Escape') setOpen(false);
      }}
    >
      {trigger}
      <span
        id={id}
        role="tooltip"
        className={cn(
          'pointer-events-none absolute left-1/2 z-40 w-max max-w-xs -translate-x-1/2',
          'rounded-md bg-ink px-2.5 py-1.5 text-xs leading-snug font-normal text-white shadow-lg',
          'transition-opacity duration-150 ease-out',
          // `invisible` (visibility:hidden) keeps the node out of the accessibility tree
          // while it is closed, which the `hidden` attribute would do too — but visibility
          // is animatable, so the fade is real rather than decorative.
          open ? 'visible opacity-100' : 'invisible opacity-0',
          side === 'top' ? 'bottom-[calc(100%+0.5rem)]' : 'top-[calc(100%+0.5rem)]',
          className,
        )}
      >
        {content}
      </span>
    </span>
  );
}
