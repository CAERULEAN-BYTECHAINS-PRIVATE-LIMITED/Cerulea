'use client';

import { useState } from 'react';
import { signIn } from 'next-auth/react';
import { useSearchParams, useRouter } from 'next/navigation';
import {
  Box, Paper, Typography, TextField, Button, Stack, Link, Alert
} from '@mui/material';

export default function LoginPage() {
  const sp = useSearchParams();
  const router = useRouter();
  const next = sp.get('next') || '/dashboard';
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr(null); setLoading(true);
    try {
      const res = await signIn('credentials', { redirect: false, email, password });
      setLoading(false);
      if (res?.error) return setErr('Invalid email or password.');
      // Hard redirect so middleware runs fresh and enforces the pricing gate
      window.location.href = next;
    } catch {
      setLoading(false);
      setErr('Something went wrong. Please try again.');
    }
  };

  return (
    <Box sx={{
      display:'grid', placeItems:'center', minHeight:'100vh',
      background: 'radial-gradient(1200px 600px at 10% 10%, rgba(99,102,241,.18), transparent), radial-gradient(1000px 500px at 90% 10%, rgba(16,185,129,.12), transparent)'
    }}>
      <Paper elevation={0} sx={{
        width: 420, p: 3, borderRadius: 3,
        backdropFilter: 'blur(16px)',
        backgroundColor: 'rgba(255,255,255,0.08)',
        border: '1px solid rgba(255,255,255,0.16)'
      }}>
        <Typography variant="h5" sx={{mb:1.5, fontWeight:700}}>Welcome back</Typography>
        <Typography variant="body2" sx={{mb:3, opacity:.8}}>
          Sign in to access your Cerulea dashboard.
        </Typography>
        {err && <Alert severity="error" sx={{mb:2}}>{err}</Alert>}
        <Box component="form" onSubmit={onSubmit}>
          <Stack spacing={1.5}>
            <TextField
              label="Email" type="email" value={email} onChange={e=>setEmail(e.target.value)} required
              size="small" autoFocus fullWidth
            />
            <TextField
              label="Password" type="password" value={password} onChange={e=>setPassword(e.target.value)} required
              size="small" fullWidth
            />
            <Button type="submit" variant="contained" disabled={loading}>
              {loading ? 'Signing in…' : 'Sign in'}
            </Button>
          </Stack>
        </Box>
        <Stack direction="row" justifyContent="space-between" sx={{mt:2}}>
          <Link href="/auth/register" underline="hover">Create account</Link>
          <Link href="/auth/forgot-password" underline="hover">Forgot password?</Link>
        </Stack>
      </Paper>
    </Box>
  );
}
