// apps/frontend/src/components/AuthGate.tsx
'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { CircularProgress, Box } from '@mui/material';

export default function AuthGate({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const [ok, setOk] = useState<boolean | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const r = await fetch('/api/auth/me', { credentials: 'include' });
        if (r.ok) setOk(true);
        else {
          setOk(false);
          router.replace('/auth/login?next=/dashboard');
        }
      } catch {
        setOk(false);
        router.replace('/auth/login?next=/dashboard');
      }
    })();
  }, [router]);

  if (ok === null) {
    return (
      <Box sx={{ display: 'grid', placeItems: 'center', minHeight: '40vh' }}>
        <CircularProgress />
      </Box>
    );
  }

  return <>{children}</>;
}
