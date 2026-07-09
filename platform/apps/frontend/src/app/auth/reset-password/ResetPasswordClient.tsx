'use client';
import { useState } from 'react';
import { useSearchParams, useRouter } from 'next/navigation';
import { Box, Paper, Typography, TextField, Button, Alert, Stack, Link } from '@mui/material';

export default function ResetPasswordPage(){
  const sp=useSearchParams();
  const token=sp.get('token')||'';
  const router=useRouter();
  const [password,setPassword]=useState('');
  const [ok,setOk]=useState(false);
  const [err,setErr]=useState<string|null>(null);
  const [loading,setLoading]=useState(false);

  const submit=async(e:React.FormEvent)=>{
    e.preventDefault();
    setErr(null); setOk(false); setLoading(true);
    const res=await fetch('/api/auth/reset-password',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({token,password})});
    setLoading(false);
    if(!res.ok){ setErr('Reset failed or token expired.'); return; }
    setOk(true);
    setTimeout(()=>router.push('/auth/login'), 1000);
  };

  return (
    <Box sx={{display:'grid',placeItems:'center',minHeight:'100vh',
      background:'radial-gradient(1200px 600px at 10% 10%, rgba(99,102,241,.18), transparent), radial-gradient(1000px 500px at 90% 10%, rgba(16,185,129,.12), transparent)'}}>
      <Paper elevation={0} sx={{width: 420, p:3, borderRadius:3, backdropFilter:'blur(16px)', backgroundColor:'rgba(255,255,255,0.08)', border:'1px solid rgba(255,255,255,0.16)'}}>
        <Typography variant="h5" sx={{mb:1.5, fontWeight:700}}>Reset password</Typography>
        <Typography variant="body2" sx={{mb:3, opacity:.8}}>Choose a new password for your account.</Typography>
        {ok && <Alert severity="success" sx={{mb:2}}>Password updated. Redirecting…</Alert>}
        {err && <Alert severity="error" sx={{mb:2}}>{err}</Alert>}
        <Box component="form" onSubmit={submit}>
          <Stack spacing={1.5}>
            <TextField label="New password" type="password" value={password} onChange={e=>setPassword(e.target.value)} required size="small"/>
            <Button type="submit" variant="contained" disabled={loading || !token}>{loading?'Updating…':'Update password'}</Button>
          </Stack>
        </Box>
        <Stack direction="row" justifyContent="space-between" sx={{mt:2}}>
          <Link href="/auth/login" underline="hover">Back to sign in</Link>
          <Typography variant="caption" sx={{opacity:.7}}>Token: {token ? 'OK' : 'missing'}</Typography>
        </Stack>
      </Paper>
    </Box>
  );
}
