import type { Metadata, Viewport } from 'next';
import { IBM_Plex_Mono, Inter } from 'next/font/google';
import './globals.css';

/**
 * Both faces are self-hosted by `next/font` rather than linked from a CDN. The demo may
 * be given on a lectern machine with no reliable network, and a font that fails to load
 * would take the typographic hierarchy with it.
 *
 * Two families on a contrast axis, not two lookalike sans faces: Inter carries every
 * label, heading and control, and IBM Plex Mono carries the machine-readable half of the
 * record — hashes, block numbers, ministry ids, HSN codes, figures in a column. A judge
 * should be able to tell at a glance which values are transcribed identifiers and which
 * are prose, without reading either.
 */
const inter = Inter({
  subsets: ['latin'],
  variable: '--font-inter',
  display: 'swap',
});

const plexMono = IBM_Plex_Mono({
  subsets: ['latin'],
  weight: ['400', '500', '600'],
  variable: '--font-plex-mono',
  display: 'swap',
});

export const metadata: Metadata = {
  title: {
    default: 'CBC-PRAMAAN — Make in India compliance verification',
    template: '%s · CBC-PRAMAAN',
  },
  description:
    'Compliance verification infrastructure for the Public Procurement (Preference to Make in India) Order. Every classification, preference calculation and certification is decided on chain and returned only after finality.',
  applicationName: 'CBC-PRAMAAN',
  authors: [{ name: 'Caerulean Bytechians' }],
  robots: { index: false, follow: false },
};

export const viewport: Viewport = {
  themeColor: '#0d1521',
  width: 'device-width',
  initialScale: 1,
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className={`${inter.variable} ${plexMono.variable}`}>
      <body>{children}</body>
    </html>
  );
}
