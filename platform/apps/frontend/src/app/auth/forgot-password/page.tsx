'use client';
import { useState } from 'react';
import { Box, Paper, Typography, TextField, Button, Alert, Stack, Link } from '@mui/material';

export default function ForgotPasswordPage(){
  const [email,setEmail]=useState('');
  const [msg,setMsg]=useState<string|null>(null);
  const [err,setErr]=useState<string|null>(null);
  const [loading,setLoading]=useState(false);

  const submit=async(e:React.FormEvent)=>{
    e.preventDefault();
    setMsg(null); setErr(null); setLoading(true);
    const res=await fetch('/api/auth/forgot-password',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email})});
    setLoading(false);
    if(!res.ok) return setErr('Failed to send reset link. Try again.');
    setMsg('If an account exists, a reset link has been emailed.');
  };

  return (
    <Box sx={{display:'grid',placeItems:'center',minHeight:'100vh',
      background:'radial-gradient(1200px 600px at 10% 10%, rgba(99,102,241,.18), transparent), radial-gradient(1000px 500px at 90% 10%, rgba(16,185,129,.12), transparent)'}}>
      <Paper elevation={0} sx={{width: 420, p:3, borderRadius:3, backdropFilter:'blur(16px)', backgroundColor:'rgba(255,255,255,0.08)', border:'1px solid rgba(255,255,255,0.16)'}}>
        <Typography variant="h5" sx={{mb:1.5, fontWeight:700}}>Forgot password</Typography>
        <Typography variant="body2" sx={{mb:3, opacity:.8}}>Enter your email to receive a reset link.</Typography>
        {msg && <Alert severity="success" sx={{mb:2}}>{msg}</Alert>}
        {err && <Alert severity="error" sx={{mb:2}}>{err}</Alert>}
        <Box component="form" onSubmit={submit}>
          <Stack spacing={1.5}>
            <TextField label="Email" type="email" value={email} onChange={e=>setEmail(e.target.value)} required size="small"/>
            <Button type="submit" variant="contained" disabled={loading}>{loading?'Sending…':'Send reset link'}</Button>
          </Stack>
        </Box>
        <Stack direction="row" justifyContent="flex-end" sx={{mt:2}}>
          <Link href="/auth/login" underline="hover">Back to sign in</Link>
        </Stack>
      </Paper>
    </Box>
  );
}
