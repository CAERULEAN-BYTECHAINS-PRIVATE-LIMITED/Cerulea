import type { Metadata } from 'next';
import { ReactNode } from 'react';
import { Providers } from './providers';
import { StudioProvider } from '@/context/StudioContext';
import NavBar from '../components/NavBar';
import Assistant from '@/components/AI/Assistant';
import Background from '@/components/Theme/Background'; // Import the new Background component
import './globals.css';

export const metadata: Metadata = {
  title: 'Cerulea Studio',
  description: 'The AI Layer 1 for the next internet',
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body suppressHydrationWarning>
        <Providers>
          <StudioProvider>
            <Background /> {/* This renders the background behind everything */}
            <NavBar />
            {children}
            <Assistant />
          </StudioProvider>
        </Providers>
      </body>
    </html>
  );
}