import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/**
 * Merge Tailwind class lists so a caller's `className` always wins over a component's
 * defaults. Every primitive in this kit takes `className` and funnels it through here,
 * which is what lets the persona pages adjust spacing without forking a component.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
