'use client';

import { useEffect, useState } from 'react';
import { Container, Typography, TextField, Button, Stack, Alert } from '@mui/material';

export default function ProfilePage() {
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const r = await fetch('/api/auth/me', { credentials: 'include' });
        if (r.ok) {
          const u = await r.json();
          setName(u.name || '');
          setEmail(u.email || '');
        } else {
          setErr('Not signed in');
        }
      } catch {
        setErr('Could not load profile');
      }
    })();
  }, []);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setMsg(null); setErr(null); setBusy(true);
    try {
      const r = await fetch('/api/auth/me', {
        method: 'PUT',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      });
      if (!r.ok) throw new Error('Could not update profile');
      const u = await r.json();
      setName(u.name || '');
      setMsg('Profile updated');

      // ✅ Make the header/avatar refresh immediately
      window.dispatchEvent(new Event('cerulea-auth-changed'));
    } catch (e: any) {
      setErr(e.message || 'Update failed');
    } finally {
      setBusy(false);
    }
  };

  return (
    <Container maxWidth="sm" sx={{ py: 8 }}>
      <Typography variant="h4" fontWeight={800} sx={{ mb: 2 }}>Profile</Typography>
      {msg && <Alert severity="success" sx={{ mb: 2 }}>{msg}</Alert>}
      {err && <Alert severity="error" sx={{ mb: 2 }}>{err}</Alert>}
      <form onSubmit={submit}>
        <Stack spacing={2}>
          <TextField label="Full name" value={name} onChange={(e) => setName(e.target.value)} fullWidth />
          <TextField label="Email" value={email} disabled fullWidth />
          <Button type="submit" variant="contained" disabled={busy}>
            {busy ? 'Saving…' : 'Save changes'}
          </Button>
        </Stack>
      </form>
    </Container>
  );
}
