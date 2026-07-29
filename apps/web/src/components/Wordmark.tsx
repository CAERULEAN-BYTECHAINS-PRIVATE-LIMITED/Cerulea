import { cn } from './ui/cn';

/**
 * The product mark.
 *
 * Deliberately abstract — three stacked blocks with a verification tick — rather than any
 * national emblem or ministry crest, whose use is restricted. It reads as institutional
 * without claiming an authority this PoC does not have.
 */
export function Mark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 32 32"
      role="img"
      aria-label="CBC-PRAMAAN"
      className={cn('size-8 shrink-0', className)}
    >
      <rect width="32" height="32" rx="7" className="fill-cerulea" />
      <path
        d="M8 11.5h9M8 16h6.5M8 20.5h5"
        stroke="white"
        strokeOpacity="0.55"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="m17.5 19.5 3.2 3.2 6.3-8"
        stroke="white"
        strokeWidth="2.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function Wordmark({
  className,
  showTagline = true,
}: {
  className?: string;
  showTagline?: boolean;
}) {
  return (
    <span className={cn('inline-flex items-center gap-2.5', className)}>
      <Mark />
      <span className="flex flex-col leading-none">
        <span className="text-[0.9375rem] font-semibold tracking-tight text-ink">
          CBC<span className="text-ink-subtle">-</span>PRAMAAN
        </span>
        {showTagline && (
          <span className="mt-1 hidden text-[0.6875rem] text-ink-muted sm:block">
            Make in India compliance verification
          </span>
        )}
      </span>
    </span>
  );
}
