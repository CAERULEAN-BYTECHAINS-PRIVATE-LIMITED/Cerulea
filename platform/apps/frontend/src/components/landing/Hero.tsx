'use client';

import Link from 'next/link';
import { Box, Button, Chip, Container, Stack, Typography } from '@mui/material';
import { motion, useScroll, useTransform } from 'framer-motion';

export default function Hero() {
  const { scrollY } = useScroll();
  const y = useTransform(scrollY, [0, 300], [0, -80]); // subtle parallax on copy

  return (
    <Box sx={{ position: 'relative', overflow: 'hidden', borderBottom: (t) => `1px solid ${t.palette.grey[200]}` }}>
      {/* Background video */}
      <Box sx={{ position: 'absolute', inset: 0, zIndex: 0, opacity: 0.35 }}>
        <video
          src="/media/hero.mp4"
          autoPlay
          muted
          loop
          playsInline
          style={{ width: '100%', height: '100%', objectFit: 'cover' }}
        />
      </Box>

      {/* Gradient overlays */}
      <Box
        sx={{
          position: 'absolute',
          inset: 0,
          zIndex: 1,
          background:
            'radial-gradient(1200px 600px at 10% -10%, rgba(22,98,240,0.30), transparent), radial-gradient(800px 400px at 100% 0%, rgba(0,194,255,0.25), transparent)',
        }}
      />

      <Container sx={{ position: 'relative', zIndex: 2, py: { xs: 10, md: 16 } }}>
        <motion.div style={{ y }}>
          <Stack spacing={3} sx={{ maxWidth: 920 }}>
            <Chip label="AI-First • No-Code • Dual Chains" color="primary" variant="outlined" />
            <Typography variant="h1" sx={{ fontSize: { xs: 36, md: 60 }, lineHeight: 1.02 }}>
              Build blockchain apps&nbsp;
              <Box component="span" sx={{ color: 'primary.main' }}>without running a chain</Box>.
            </Typography>
            <Typography variant="h6" color="text.secondary" sx={{ maxWidth: 760 }}>
              Design models, flows, and policies visually. Simulate chain behavior instantly.
              Deploy a real runtime in minutes, no infra grind.
            </Typography>

            <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} sx={{ pt: 1 }}>
              <Button component={Link} href="/auth/register" variant="contained" size="large">
                Get started free
              </Button>
              <Button component={Link} href="/dashboard" variant="outlined" size="large">
                View dashboard
              </Button>
              <Button
                variant="text"
                size="large"
                onClick={() => {
                  const isLocal = window.location.hostname.includes('localhost');
                  window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
                }}
              >
                Open Studio
              </Button>
            </Stack>
          </Stack>
        </motion.div>
      </Container>
    </Box>
  );
}
