'use client';

import { useState } from 'react';
import { signIn } from 'next-auth/react';
import {
  Dialog, DialogContent, DialogTitle, Tabs, Tab, Box,
  TextField, Button, Stack, Typography, Alert, IconButton,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';

type Props = {
  open: boolean;
  title?: string;
  subtitle?: string;
  onSuccess: () => void;
  onClose: () => void;
};

export default function AuthModal({ open, title = 'Sign in', subtitle, onSuccess, onClose }: Props) {
  const [tab, setTab] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [loginForm, setLoginForm] = useState({ email: '', password: '' });
  const [regForm, setRegForm] = useState({ name: '', email: '', password: '' });

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    const res = await signIn('credentials', { redirect: false, ...loginForm });
    setLoading(false);
    if (res?.error) return setError('Invalid email or password.');
    onSuccess();
  };

  const handleRegister = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    const res = await fetch('/api/auth/register', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(regForm),
    });
    if (!res.ok) {
      const j = await res.json().catch(() => ({ error: 'Failed' }));
      setLoading(false);
      return setError(j.error || 'Registration failed');
    }
    const signInRes = await signIn('credentials', {
      redirect: false,
      email: regForm.email,
      password: regForm.password,
    });
    setLoading(false);
    if (signInRes?.error) {
      setTab(0);
      return setError('Account created. Please sign in.');
    }
    // New accounts must select a plan before entering the studio
    const isLocal = window.location.hostname.includes('localhost');
    const returnUrl = encodeURIComponent(window.location.href);
    const base = isLocal ? 'http://localhost:3000' : 'https://cerulea.app';
    window.location.href = `${base}/pricing?return=${returnUrl}`;
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="xs"
      fullWidth
      PaperProps={{ sx: { borderRadius: 3, p: 0.5 } }}
    >
      <DialogTitle sx={{ pb: 0, pr: 6 }}>
        <Typography variant="h6" fontWeight={700}>{title}</Typography>
        {subtitle && (
          <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
            {subtitle}
          </Typography>
        )}
        <IconButton
          size="small"
          onClick={onClose}
          sx={{ position: 'absolute', top: 12, right: 12, color: 'text.secondary' }}
        >
          <CloseIcon fontSize="small" />
        </IconButton>
      </DialogTitle>

      <DialogContent sx={{ pt: 1 }}>
        <Tabs value={tab} onChange={(_, v) => { setTab(v); setError(null); }} sx={{ mb: 2 }}>
          <Tab label="Sign In" />
          <Tab label="Create Account" />
        </Tabs>

        {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}

        {tab === 0 ? (
          <Box component="form" onSubmit={handleLogin}>
            <Stack spacing={1.5}>
              <TextField
                label="Email" type="email" size="small" required autoFocus
                value={loginForm.email}
                onChange={(e) => setLoginForm((f) => ({ ...f, email: e.target.value }))}
              />
              <TextField
                label="Password" type="password" size="small" required
                value={loginForm.password}
                onChange={(e) => setLoginForm((f) => ({ ...f, password: e.target.value }))}
              />
              <Button type="submit" variant="contained" disabled={loading} fullWidth>
                {loading ? 'Signing in...' : 'Sign In'}
              </Button>
            </Stack>
          </Box>
        ) : (
          <Box component="form" onSubmit={handleRegister}>
            <Stack spacing={1.5}>
              <TextField
                label="Full name" size="small" required autoFocus
                value={regForm.name}
                onChange={(e) => setRegForm((f) => ({ ...f, name: e.target.value }))}
              />
              <TextField
                label="Email" type="email" size="small" required
                value={regForm.email}
                onChange={(e) => setRegForm((f) => ({ ...f, email: e.target.value }))}
              />
              <TextField
                label="Password" type="password" size="small" required
                value={regForm.password}
                onChange={(e) => setRegForm((f) => ({ ...f, password: e.target.value }))}
              />
              <Button type="submit" variant="contained" disabled={loading} fullWidth>
                {loading ? 'Creating account...' : 'Create Account'}
              </Button>
            </Stack>
          </Box>
        )}
      </DialogContent>
    </Dialog>
  );
}
