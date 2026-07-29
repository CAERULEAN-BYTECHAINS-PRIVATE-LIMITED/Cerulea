/** Small display helpers shared by the four explorer panels. */

export function formatBlockNumber(value: number): string {
  return `#${value.toLocaleString('en-IN')}`;
}

export function formatClock(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString('en-IN', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  });
}

/** "just now" / "4s ago" / "2m 10s ago" — a 250 ms chain needs seconds, not minutes. */
export function formatAge(timestamp: number, now: number): string {
  const seconds = Math.max(0, Math.round((now - timestamp) / 1000));
  if (seconds < 1) return 'just now';
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes}m ${seconds % 60}s ago`;
}

export function shortenHash(value: string, head = 8, tail = 6): string {
  return value.length > head + tail + 3
    ? `${value.slice(0, head)}…${value.slice(-tail)}`
    : value;
}

/**
 * Turn an event's decoded field map into label/value pairs a non-technical reader can
 * scan. `toHuman()` already comma-groups numbers and renders enum variants by name, so
 * this only has to unwrap the field names and flatten anything nested.
 */
export function eventFields(data: Record<string, unknown>): { label: string; value: string }[] {
  return Object.entries(data).map(([key, value]) => ({
    label: key
      .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
      .replace(/_/g, ' ')
      .replace(/^./, (character) => character.toUpperCase()),
    value:
      value === null || value === undefined
        ? '—'
        : typeof value === 'object'
          ? JSON.stringify(value)
          : String(value),
  }));
}

/** Ministry ids and tender ids arrive as ASCII hex on some paths; render them readable. */
export function decodeAscii(value: string): string {
  if (!value.startsWith('0x')) return value;
  const bytes = value.slice(2).match(/.{2}/g) ?? [];
  const text = bytes.map((byte) => String.fromCharCode(parseInt(byte, 16))).join('');
  return /^[\x20-\x7e]+$/.test(text) ? text : value;
}
