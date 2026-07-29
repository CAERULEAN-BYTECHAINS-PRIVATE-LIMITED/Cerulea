import type { Metadata, Viewport } from 'next';
import { Inter } from 'next/font/google';
import './globals.css';

/**
 * Inter is self-hosted by `next/font` rather than linked from a CDN.
 *
 * The demo may be given on a lectern machine with no reliable network, and a font that
 * fails to load would take the typographic hierarchy with it. Binding it to `--font-sans`
 * means the token declared in `globals.css` resolves to the real Inter without that file
 * needing to change.
 */
const inter = Inter({
  subsets: ['latin'],
  variable: '--font-sans',
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
  themeColor: '#004aad',
  width: 'device-width',
  initialScale: 1,
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className={inter.variable}>
      <body>{children}</body>
    </html>
  );
}
