// apps/frontend/src/components/UserMenu.tsx
'use client';

import { useEffect, useState } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import {
  Avatar, Button, IconButton, Menu, MenuItem, ListItemIcon, Divider, Tooltip, CircularProgress,
} from '@mui/material';
import Logout from '@mui/icons-material/Logout';
import Settings from '@mui/icons-material/Settings';

type Me = { id: string; email: string; name?: string };

export default function UserMenu() {
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const [me, setMe] = useState<Me | null>(null);
  const [loading, setLoading] = useState(true);
  const open = Boolean(anchorEl);
  const pathname = usePathname();

  async function refreshMe() {
    setLoading(true);
    try {
      const r = await fetch('/api/auth/me', { credentials: 'include' });
      setMe(r.ok ? await r.json() : null);
    } catch {
      setMe(null);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { refreshMe(); }, []);               // on mount
  useEffect(() => { refreshMe(); }, [pathname]);       // on route change
  useEffect(() => {                                   // on focus / visibility / custom event
    const onFocus = () => refreshMe();
    const onVis = () => document.visibilityState === 'visible' && refreshMe();
    const onAuth = () => refreshMe();
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onVis);
    window.addEventListener('cerulea-auth-changed', onAuth as any);
    return () => {
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onVis);
      window.removeEventListener('cerulea-auth-changed', onAuth as any);
    };
  }, []);

  const handleOpen = (e: React.MouseEvent<HTMLElement>) => setAnchorEl(e.currentTarget);
  const handleClose = () => setAnchorEl(null);

  const handleLogout = async () => {
    try {
      await fetch('/api/auth/logout', { method: 'POST', credentials: 'include' });
    } catch {}
    // tell the app auth changed; then hard redirect to clear any stale client state
    window.dispatchEvent(new Event('cerulea-auth-changed'));
    window.location.href = '/auth/login';
  };

  if (loading) return <CircularProgress size={20} sx={{ ml: 2 }} />;

  if (!me) {
    return (
      <>
        <Button component={Link} href="/auth/login" variant="text">Sign in</Button>
        <Button component={Link} href="/auth/register" variant="contained">Get started</Button>
      </>
    );
  }

  const initial = (me.name || me.email || 'C').trim().charAt(0).toUpperCase();

  return (
    <>
      <Tooltip title="Account">
        <IconButton onClick={handleOpen} size="small" sx={{ ml: 1 }}>
          <Avatar sx={{ width: 32, height: 32 }}>{initial}</Avatar>
        </IconButton>
      </Tooltip>
      <Menu
        anchorEl={anchorEl} open={open} onClose={handleClose}
        PaperProps={{ elevation: 3, sx: { mt: 1.5, minWidth: 220 } }}
        anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
        transformOrigin={{ horizontal: 'right', vertical: 'top' }}
      >
        <MenuItem disabled>{me.name ? `${me.name} · ${me.email}` : me.email}</MenuItem>
        <Divider />
        <MenuItem component={Link} href="/dashboard" onClick={handleClose}>Dashboard</MenuItem>
        <MenuItem component={Link} href="/settings/profile" onClick={handleClose}>
          <ListItemIcon><Settings fontSize="small" /></ListItemIcon>
          Settings
        </MenuItem>
        <Divider />
        <MenuItem onClick={handleLogout}>
          <ListItemIcon><Logout fontSize="small" /></ListItemIcon>
          Logout
        </MenuItem>
      </Menu>
    </>
  );
}
