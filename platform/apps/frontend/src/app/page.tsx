// apps/frontend/src/app/page.tsx
import { headers } from 'next/headers';
import dynamic from 'next/dynamic';

// Lazy-load the Studio entry so landing users don't download studio code
const StudioEntry = dynamic(() => import('@/app/_studio/StudioEntry'), { ssr: false });

// Your existing landing component:
import Landing from '@/components/landing/Hero'; // <- adjust if your landing uses a different entry

type SearchParams = { [key: string]: string | string[] | undefined };

export default async function Home({ searchParams }: { searchParams?: SearchParams }) {
  const host = headers().get('host') ?? '';
  const isStudioHost =
    host.startsWith('studio.') ||
    host.includes('studio.localhost') ||
    host.includes('studio.lvh.me');

  // Dev override: http://localhost:3000/?studio=1
  const forceStudio = !!(searchParams && ('studio' in searchParams));

  if (isStudioHost || forceStudio) {
    return <StudioEntry />; // mounts the full Studio Shell
  }

  // Regular marketing / landing page for everything else
  return <Landing />;
}
