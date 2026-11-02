import { cn } from './ui/cn';

/**
 * The product mark.
 *
 * Deliberately abstract — a sealed record with a verification rule through it — rather
 * than any national emblem, ashoka capital or ministry crest, whose use is restricted by
 * the State Emblem of India (Prohibition of Improper Use) Act. It has to read as
 * institutional without claiming an authority this proof of concept does not hold.
 */
export function Mark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      role="img"
      aria-label="CBC-PRAMAAN"
      className={cn('size-6 shrink-0', className)}
    >
      <rect x="0.75" y="0.75" width="22.5" height="22.5" rx="2" className="fill-white/10 stroke-white/35" strokeWidth="1.5" />
      <path d="M5.5 8h8M5.5 12h5.5" stroke="currentColor" strokeOpacity="0.7" strokeWidth="1.6" strokeLinecap="square" />
      <path d="m9.5 15.5 2.6 2.6 6-7" stroke="currentColor" strokeWidth="2.1" strokeLinecap="square" strokeLinejoin="miter" />
    </svg>
  );
}

export function Wordmark({ className }: { className?: string }) {
  return (
    <span className={cn('inline-flex items-center gap-2 text-white', className)}>
      <Mark />
      <span className="text-sm font-semibold tracking-tight">
        CBC<span className="text-white/40">·</span>PRAMAAN
      </span>
    </span>
  );
}
