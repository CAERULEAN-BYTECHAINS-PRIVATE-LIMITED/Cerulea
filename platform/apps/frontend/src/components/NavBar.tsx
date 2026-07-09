'use client';

import {
  AppBar, Toolbar, Typography, Button, IconButton, Avatar, Box,
  Menu, MenuItem, Divider, ListItemIcon, Chip, Tooltip,
} from '@mui/material';
import { useTheme, alpha } from '@mui/material/styles';
import { useState, useEffect } from 'react';
import { useSession, signOut, getSession } from 'next-auth/react';
import Link from 'next/link';
import Brightness4Icon from '@mui/icons-material/Brightness4';
import Brightness7Icon from '@mui/icons-material/Brightness7';
import DashboardIcon from '@mui/icons-material/Dashboard';
import PersonIcon from '@mui/icons-material/Person';
import LoginIcon from '@mui/icons-material/Login';
import PersonAddIcon from '@mui/icons-material/PersonAdd';
import LogoutIcon from '@mui/icons-material/Logout';
import { useThemeToggle } from '@/app/providers';

export default function NavBar() {
  const theme = useTheme();
  const { toggleTheme } = useThemeToggle();
  const { data: session, status, update } = useSession();
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);

  // Refresh session on mount so NavBar always reflects current auth state
  // after a hard navigation back from the login page
  useEffect(() => { update(); }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const user = session?.user as any;
  const initials = user?.name
    ? user.name.split(' ').map((n: string) => n[0]).join('').toUpperCase().slice(0, 2)
    : user?.email?.[0]?.toUpperCase() ?? '?';

  return (
    <AppBar
      position="sticky"
      elevation={0}
      sx={{
        backgroundColor: alpha(theme.palette.background.paper, 0.85),
        backdropFilter: 'blur(16px) saturate(180%)',
        WebkitBackdropFilter: 'blur(16px) saturate(180%)',
        borderBottom: `1px solid ${theme.palette.divider}`,
        color: theme.palette.text.primary,
      }}
    >
      <Toolbar sx={{ gap: 0.5 }}>
        {/* Logo */}
        <Typography
          component={Link}
          href="/"
          variant="h6"
          sx={{
            flexGrow: 1,
            fontWeight: 800,
            textDecoration: 'none',
            color: 'inherit',
            letterSpacing: -0.5,
          }}
        >
          Cerulea Studio
        </Typography>

        {/* Theme toggle */}
        <Tooltip title={theme.palette.mode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}>
          <IconButton onClick={toggleTheme} color="inherit" size="small" sx={{ ml: 0.5 }}>
            {theme.palette.mode === 'dark' ? <Brightness7Icon fontSize="small" /> : <Brightness4Icon fontSize="small" />}
          </IconButton>
        </Tooltip>

        {/* Auth state */}
        {status === 'loading' ? null : session ? (
          <>
            <Button
              component={Link}
              href="/dashboard"
              size="small"
              startIcon={<DashboardIcon />}
              sx={{ ml: 0.5 }}
              color="inherit"
            >
              Dashboard
            </Button>

            <Tooltip title={user?.name || user?.email || 'Profile'}>
              <IconButton
                size="small"
                onClick={(e) => setAnchorEl(e.currentTarget)}
                sx={{ ml: 0.5 }}
              >
                <Avatar
                  sx={{
                    width: 32,
                    height: 32,
                    fontSize: '0.75rem',
                    fontWeight: 700,
                    bgcolor: theme.palette.primary.main,
                    color: theme.palette.primary.contrastText,
                  }}
                >
                  {initials}
                </Avatar>
              </IconButton>
            </Tooltip>

            <Menu
              anchorEl={anchorEl}
              open={Boolean(anchorEl)}
              onClose={() => setAnchorEl(null)}
              transformOrigin={{ horizontal: 'right', vertical: 'top' }}
              anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
              PaperProps={{
                elevation: 3,
                sx: { minWidth: 220, borderRadius: 2, mt: 0.5 },
              }}
            >
              <Box sx={{ px: 2, py: 1.5 }}>
                <Typography variant="subtitle2" fontWeight={700} noWrap>
                  {user?.name || 'User'}
                </Typography>
                <Typography variant="caption" color="text.secondary" noWrap>
                  {user?.email}
                </Typography>
                {user?.plan && (
                  <Chip
                    label={user.plan.toUpperCase()}
                    size="small"
                    color={user.plan === 'pro' || user.plan === 'enterprise' ? 'primary' : 'default'}
                    sx={{ mt: 0.5, height: 18, fontSize: '0.6rem' }}
                  />
                )}
              </Box>
              <Divider />
              <MenuItem
                component={Link}
                href="/settings/profile"
                onClick={() => setAnchorEl(null)}
                dense
              >
                <ListItemIcon><PersonIcon fontSize="small" /></ListItemIcon>
                My Profile
              </MenuItem>
              <MenuItem
                component={Link}
                href="/dashboard"
                onClick={() => setAnchorEl(null)}
                dense
              >
                <ListItemIcon><DashboardIcon fontSize="small" /></ListItemIcon>
                Dashboard
              </MenuItem>
              <Divider />
              <MenuItem
                onClick={() => {
                  setAnchorEl(null);
                  // Use the force-signout route which expires the httpOnly session
                  // cookie directly via Set-Cookie response headers — the only
                  // reliable way to clear httpOnly cookies from the browser.
                  const returnTo = encodeURIComponent(window.location.origin + '/');
                  window.location.href = `/api/auth/force-signout?next=${returnTo}`;
                }}
                dense
              >
                <ListItemIcon><LogoutIcon fontSize="small" /></ListItemIcon>
                Sign Out
              </MenuItem>
            </Menu>
          </>
        ) : (
          <>
            <Button
              component={Link}
              href="/auth/login"
              size="small"
              startIcon={<LoginIcon />}
              color="inherit"
              sx={{ ml: 0.5 }}
            >
              Sign In
            </Button>
            <Button
              component={Link}
              href="/auth/register"
              size="small"
              startIcon={<PersonAddIcon />}
              variant="contained"
              color="primary"
              sx={{ ml: 0.5 }}
            >
              Sign Up
            </Button>
          </>
        )}
      </Toolbar>
    </AppBar>
  );
}
