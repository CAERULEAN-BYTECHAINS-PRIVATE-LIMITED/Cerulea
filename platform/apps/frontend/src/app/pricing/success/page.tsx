'use client';

import { useEffect, useState } from 'react';
import { Box, Typography, Button, Paper, CircularProgress } from '@mui/material';
import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import { useSession } from 'next-auth/react';

export default function PricingSuccessPage() {
  const { update } = useSession();
  const [returnUrl, setReturnUrl] = useState('');
  const [refreshed, setRefreshed] = useState(false);

  useEffect(() => {
    // Read return URL client-side to avoid useSearchParams() Suspense requirement at build time
    const params = new URLSearchParams(window.location.search);
    setReturnUrl(params.get('return') || '');
    // Force JWT to re-fetch plan from DB so middleware sees the new subscription
    update().then(() => setRefreshed(true)).catch(() => setRefreshed(true));
  }, []);

  const handleContinue = () => {
    if (returnUrl) {
      window.location.href = returnUrl;
    } else {
      const isLocal = window.location.hostname.includes('localhost');
      window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
    }
  };

  return (
    <Box sx={{ display: 'grid', placeItems: 'center', minHeight: '100vh', px: 2 }}>
      <Paper
        elevation={0}
        sx={{
          p: 5,
          borderRadius: 4,
          textAlign: 'center',
          maxWidth: 480,
          backdropFilter: 'blur(16px)',
          backgroundColor: 'rgba(255,255,255,0.06)',
          border: '1px solid rgba(255,255,255,0.12)',
        }}
      >
        <CheckCircleOutlineIcon sx={{ fontSize: 64, color: 'success.main', mb: 2 }} />
        <Typography variant="h4" sx={{ fontWeight: 800, mb: 1.5 }}>
          You're all set!
        </Typography>
        <Typography variant="body1" sx={{ opacity: 0.75, mb: 4 }}>
          Your subscription is now active. You have full access to Cerulea Studio
          and all the features included in your plan.
        </Typography>
        <Button
          variant="contained"
          size="large"
          onClick={handleContinue}
          disabled={!refreshed}
          startIcon={!refreshed ? <CircularProgress size={16} color="inherit" /> : undefined}
          sx={{ px: 4, py: 1.5, fontWeight: 700, borderRadius: 2 }}
        >
          {refreshed ? 'Open Studio' : 'Activating…'}
        </Button>
      </Paper>
    </Box>
  );
}
