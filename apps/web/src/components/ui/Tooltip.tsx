'use client';

import {
  Children,
  cloneElement,
  isValidElement,
  useId,
  useState,
  type FocusEvent,
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
 * rather than a wrapper, and it opens on keyboard focus as well as hover. Tooltips carry
 * supporting detail only — never the only copy of a fact, because a projector audience
 * cannot hover.
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
    <span className="relative inline-flex">
      {trigger}
      {open && (
        <span
          role="tooltip"
          id={id}
          className={cn(
            'pointer-events-none absolute left-1/2 z-80 w-64 -translate-x-1/2',
            'rounded-md border border-masthead-2 bg-masthead px-2.5 py-1.5',
            'text-2xs leading-relaxed text-white shadow-pop',
            side === 'top' ? 'bottom-full mb-1.5' : 'top-full mt-1.5',
            className,
          )}
        >
          {content}
        </span>
      )}
    </span>
  );
}
