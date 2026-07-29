/**
 * One colour and mark vocabulary for every chart on the dashboard.
 *
 * Three rules govern everything here, in this order:
 *
 * 1. **The three status colours are reserved.** `#1E8E3E` / `#E8A100` / `#D93025` carry a
 *    legal meaning under the PPP-MII Order (docs/PRAMAAN_BUILD_CONTRACT section 5). They
 *    appear on exactly one chart — the GREEN / YELLOW / RED verdict breakdown, where they
 *    *are* the data — and nowhere else. No decorative use, no reuse as "series 4".
 *
 * 2. **Categorical hues are assigned in a fixed order and never cycled.** `CATEGORICAL`
 *    below was validated against a white chart surface with the data-viz skill's checker
 *    (`--pairs all`, light, surface #ffffff): worst pair 9.2 ΔE under deuteranopia, 16.3
 *    ΔE under normal vision, both clear of the floors. Nothing is generated past the
 *    fourth slot — a fifth ministry folds into "Other".
 *
 * 3. **Colour is never the only channel.** Every chart here ships a legend, direct labels
 *    where they fit, and a table view for the values they don't.
 */

/** Reserved. Compliance verdicts only. */
export const STATUS_COLORS = {
  GREEN: '#1E8E3E',
  YELLOW: '#E8A100',
  RED: '#D93025',
} as const;

export const STATUS_LABELS = {
  GREEN: 'Compliant',
  YELLOW: 'Review required',
  RED: 'Blocked',
} as const;

export type TriState = keyof typeof STATUS_COLORS;

export const TRI_STATES: TriState[] = ['GREEN', 'YELLOW', 'RED'];

/**
 * Categorical identity slots, in fixed order. Validated all-pairs on a white surface.
 * Deliberately contains no green, yellow or red — an identity hue must never be mistaken
 * for a verdict on a screen where verdicts are the point.
 */
export const CATEGORICAL = ['#2a78d6', '#eb6834', '#1baf7a', '#4a3aa7'] as const;

/**
 * The "Other" bucket. Achromatic on purpose: it is not an identity, it is the absence of
 * one, and a fifth chromatic hue would fail the separation checks against the four above.
 */
export const OTHER_COLOR = '#5f6368';

/** Single-series accent for the latency chart. Brand blue, not a status colour. */
export const ACCENT = '#004AAD';

/** Chart chrome, matched to the tokens in `globals.css`. */
export const CHROME = {
  surface: '#ffffff',
  grid: '#e3e6ea',
  axis: '#5f6368',
  axisSubtle: '#9aa0a6',
  ink: '#1a1a1a',
} as const;

/** Mark specs, applied identically across the three charts. */
export const MARKS = {
  /** Bars are capped rather than filling their band, so the band's leftover reads as air. */
  maxBarSize: 24,
  lineWidth: 2,
  dotRadius: 4,
  /** A 2 px stroke in the surface colour is how a stacked segment gets its gap in SVG. */
  surfaceGap: 2,
} as const;

export const AXIS_TICK = { fill: CHROME.axis, fontSize: 11 } as const;

/** Colour for the nth ministry slice, folding past the fourth into the neutral bucket. */
export function categoricalColor(index: number): string {
  return index < CATEGORICAL.length ? CATEGORICAL[index] : OTHER_COLOR;
}
