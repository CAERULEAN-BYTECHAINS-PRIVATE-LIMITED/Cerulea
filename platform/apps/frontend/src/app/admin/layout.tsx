'use client';

import React, { useState } from 'react';
import {
  Box, List, ListItem, ListItemButton, ListItemIcon, ListItemText,
  Typography, IconButton, Tooltip, Badge, Stack, Avatar, Chip,
  TextField, InputAdornment,
} from '@mui/material';
import { alpha, useTheme, createTheme, ThemeProvider } from '@mui/material/styles';
import { usePathname, useRouter } from 'next/navigation';
import { useSession } from 'next-auth/react';
import Link from 'next/link';

// Icons
import DashboardIcon from '@mui/icons-material/Dashboard';
import PeopleIcon from '@mui/icons-material/People';
import FolderIcon from '@mui/icons-material/Folder';
import RocketLaunchIcon from '@mui/icons-material/RocketLaunch';
import HexagonOutlinedIcon from '@mui/icons-material/HexagonOutlined';
import SmartToyIcon from '@mui/icons-material/SmartToy';
import ShieldIcon from '@mui/icons-material/Shield';
import PaymentIcon from '@mui/icons-material/Payment';
import SupportAgentIcon from '@mui/icons-material/SupportAgent';
import AssignmentIcon from '@mui/icons-material/Assignment';
import TuneIcon from '@mui/icons-material/Tune';
import ChevronLeftIcon from '@mui/icons-material/ChevronLeft';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import NotificationsIcon from '@mui/icons-material/Notifications';
import SearchIcon from '@mui/icons-material/Search';
import AdminPanelSettingsIcon from '@mui/icons-material/AdminPanelSettings';
import HexagonIcon from '@mui/icons-material/Hexagon';
import LogoutIcon from '@mui/icons-material/Logout';

const DRAWER_WIDTH = 252;
const DRAWER_COLLAPSED = 64;

type NavItem = {
  label: string;
  href: string;
  icon: React.ReactNode;
  badge?: number;
  section?: string;
};

const NAV_ITEMS: NavItem[] = [
  { label: 'Overview', href: '/admin', icon: <DashboardIcon fontSize="small" />, section: 'Platform' },
  { label: 'Users', href: '/admin/users', icon: <PeopleIcon fontSize="small" />, section: 'Platform' },
  { label: 'Projects', href: '/admin/projects', icon: <FolderIcon fontSize="small" />, section: 'Platform' },
  { label: 'Deployments', href: '/admin/deployments', icon: <RocketLaunchIcon fontSize="small" />, section: 'Platform' },
  { label: 'Blockchain Ops', href: '/admin/blockchain', icon: <HexagonOutlinedIcon fontSize="small" />, section: 'Infrastructure' },
  { label: 'AI Monitor', href: '/admin/ai', icon: <SmartToyIcon fontSize="small" />, section: 'Infrastructure' },
  { label: 'Security', href: '/admin/security', icon: <ShieldIcon fontSize="small" />, badge: 3, section: 'Operations' },
  { label: 'Billing', href: '/admin/billing', icon: <PaymentIcon fontSize="small" />, section: 'Operations' },
  { label: 'Support', href: '/admin/support', icon: <SupportAgentIcon fontSize="small" />, badge: 7, section: 'Operations' },
  { label: 'Audit Logs', href: '/admin/audit', icon: <AssignmentIcon fontSize="small" />, section: 'Operations' },
  { label: 'Config', href: '/admin/config', icon: <TuneIcon fontSize="small" />, section: 'System' },
];

const SECTIONS = ['Platform', 'Infrastructure', 'Operations', 'System'];

// Force dark theme for admin
const adminTheme = createTheme({
  palette: {
    mode: 'dark',
    primary: { main: '#448aff' },
    secondary: { main: '#ce93d8' },
    error: { main: '#f44336' },
    warning: { main: '#ff9800' },
    success: { main: '#4caf50' },
    background: { default: '#080b12', paper: 'rgba(14,20,32,0.95)' },
    text: { primary: '#e6edf3', secondary: '#8b949e' },
    divider: 'rgba(255,255,255,0.07)',
  },
  typography: { fontFamily: ['Inter', 'sans-serif'].join(',') },
  shape: { borderRadius: 12 },
  components: {
    MuiButton: {
      styleOverrides: {
        root: { textTransform: 'none', fontWeight: 600, borderRadius: '999px', boxShadow: 'none' },
      },
    },
    MuiTableCell: {
      styleOverrides: {
        root: { borderColor: 'rgba(255,255,255,0.06)' },
      },
    },
    MuiChip: {
      styleOverrides: {
        root: { borderRadius: 6 },
      },
    },
  },
});

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const { data: session, status } = useSession();
  const [collapsed, setCollapsed] = useState(false);

  const drawerWidth = collapsed ? DRAWER_COLLAPSED : DRAWER_WIDTH;

  // Admin guard: redirect non-admin users (only after session is fully loaded)
  const userEmail = session?.user?.email;
  const isAdmin = userEmail === 'test@cerulea.app' || (session?.user as { isTestAccount?: boolean })?.isTestAccount;

  if (status === 'loading') {
    return (
      <ThemeProvider theme={adminTheme}>
        <Box sx={{ position: 'fixed', inset: 0, zIndex: 1500, display: 'flex', alignItems: 'center', justifyContent: 'center', bgcolor: '#080b12' }}>
          <Typography color="text.secondary">Loading...</Typography>
        </Box>
      </ThemeProvider>
    );
  }

  if (status === 'unauthenticated') {
    if (typeof window !== 'undefined') {
      const isLocal = window.location.hostname.includes('localhost');
      const mainHost = isLocal ? 'http://localhost:3000' : 'https://cerulea.app';
      window.location.href = `${mainHost}/auth/login?next=${encodeURIComponent(window.location.href)}`;
    }
    return null;
  }

  if (status === 'authenticated' && !isAdmin) {
    if (typeof window !== 'undefined') {
      const isLocal = window.location.hostname.includes('localhost');
      window.location.href = isLocal ? 'http://localhost:3000/dashboard' : 'https://cerulea.app/dashboard';
    }
    return null;
  }

  return (
    <ThemeProvider theme={adminTheme}>
      {/* position:fixed inset:0 covers the root layout's NavBar completely */}
      <Box sx={{ position: 'fixed', inset: 0, zIndex: 1500, display: 'flex', bgcolor: 'background.default', color: 'text.primary', overflow: 'hidden' }}>
        {/* Sidebar */}
        <Box
          component="nav"
          sx={{
            width: drawerWidth,
            flexShrink: 0,
            transition: 'width 0.2s ease',
            position: 'fixed',
            top: 0,
            left: 0,
            height: '100vh',
            zIndex: 1200,
            bgcolor: 'rgba(8,11,18,0.98)',
            backdropFilter: 'blur(24px)',
            borderRight: '1px solid rgba(255,255,255,0.07)',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
          }}
        >
          {/* Logo + Admin Badge */}
          <Box
            sx={{
              height: 64,
              display: 'flex',
              alignItems: 'center',
              px: collapsed ? 1.5 : 2.5,
              gap: 1.5,
              borderBottom: '1px solid rgba(255,255,255,0.07)',
              flexShrink: 0,
            }}
          >
            <HexagonIcon sx={{ color: '#448aff', fontSize: 26 }} />
            {!collapsed && (
              <Box>
                <Typography variant="subtitle2" fontWeight={900} sx={{ letterSpacing: -0.3, lineHeight: 1.1 }}>
                  Cerulea
                </Typography>
                <Chip
                  label="SUPER ADMIN"
                  size="small"
                  icon={<AdminPanelSettingsIcon sx={{ fontSize: '0.6rem !important' }} />}
                  sx={{
                    height: 16,
                    fontSize: '0.58rem',
                    fontWeight: 800,
                    bgcolor: alpha('#f44336', 0.15),
                    color: '#f44336',
                    border: `1px solid ${alpha('#f44336', 0.25)}`,
                    letterSpacing: 0.5,
                    '& .MuiChip-icon': { color: '#f44336 !important', mr: '-2px' },
                  }}
                />
              </Box>
            )}
            <Box sx={{ flex: 1 }} />
            <IconButton size="small" onClick={() => setCollapsed((c) => !c)} sx={{ color: 'text.secondary' }}>
              {collapsed ? <ChevronRightIcon fontSize="small" /> : <ChevronLeftIcon fontSize="small" />}
            </IconButton>
          </Box>

          {/* Nav Items */}
          <Box sx={{ flex: 1, overflowY: 'auto', py: 1 }}>
            {SECTIONS.map((section) => {
              const items = NAV_ITEMS.filter((i) => i.section === section);
              return (
                <Box key={section}>
                  {!collapsed && (
                    <Typography
                      variant="caption"
                      sx={{
                        px: 3,
                        pt: 2,
                        pb: 0.5,
                        display: 'block',
                        color: 'rgba(255,255,255,0.3)',
                        fontWeight: 700,
                        letterSpacing: 1,
                        fontSize: '0.62rem',
                        textTransform: 'uppercase',
                      }}
                    >
                      {section}
                    </Typography>
                  )}
                  <List disablePadding>
                    {items.map((item) => {
                      const isActive =
                        item.href === '/admin'
                          ? pathname === '/admin'
                          : pathname?.startsWith(item.href);
                      return (
                        <Tooltip key={item.href} title={collapsed ? item.label : ''} placement="right">
                          <ListItem disablePadding>
                            <ListItemButton
                              component={Link}
                              href={item.href}
                              selected={isActive}
                              sx={{
                                mx: 1,
                                borderRadius: 1.5,
                                minHeight: 38,
                                px: collapsed ? 1.5 : 1.75,
                                mb: 0.25,
                                color: isActive ? '#448aff' : 'rgba(255,255,255,0.55)',
                                '&.Mui-selected': {
                                  bgcolor: alpha('#448aff', 0.12),
                                  '&:hover': { bgcolor: alpha('#448aff', 0.16) },
                                },
                                '&:hover': {
                                  bgcolor: 'rgba(255,255,255,0.05)',
                                  color: 'rgba(255,255,255,0.85)',
                                },
                              }}
                            >
                              <ListItemIcon
                                sx={{
                                  minWidth: collapsed ? 0 : 34,
                                  color: isActive ? '#448aff' : 'inherit',
                                }}
                              >
                                {item.badge ? (
                                  <Badge badgeContent={item.badge} color="error" sx={{ '& .MuiBadge-badge': { fontSize: '0.55rem', height: 14, minWidth: 14 } }}>
                                    {item.icon}
                                  </Badge>
                                ) : item.icon}
                              </ListItemIcon>
                              {!collapsed && (
                                <ListItemText
                                  primary={item.label}
                                  primaryTypographyProps={{ fontSize: '0.82rem', fontWeight: isActive ? 700 : 500 }}
                                />
                              )}
                              {!collapsed && item.badge && (
                                <Chip
                                  label={item.badge}
                                  size="small"
                                  sx={{
                                    height: 18,
                                    fontSize: '0.6rem',
                                    fontWeight: 800,
                                    bgcolor: alpha('#f44336', 0.18),
                                    color: '#f44336',
                                    border: 'none',
                                  }}
                                />
                              )}
                            </ListItemButton>
                          </ListItem>
                        </Tooltip>
                      );
                    })}
                  </List>
                </Box>
              );
            })}
          </Box>

          {/* Footer */}
          {!collapsed && (
            <Box sx={{ p: 2, borderTop: '1px solid rgba(255,255,255,0.07)' }}>
              <Stack direction="row" alignItems="center" spacing={1.5}>
                <Avatar sx={{ width: 28, height: 28, bgcolor: alpha('#448aff', 0.2), fontSize: '0.7rem', fontWeight: 900, color: '#448aff' }}>
                  {(session?.user?.name || 'A')[0].toUpperCase()}
                </Avatar>
                <Box sx={{ flex: 1, minWidth: 0 }}>
                  <Typography variant="caption" fontWeight={700} noWrap sx={{ display: 'block', color: 'text.primary' }}>
                    {session?.user?.name || 'Admin'}
                  </Typography>
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.35)', fontSize: '0.65rem' }} noWrap>
                    {session?.user?.email}
                  </Typography>
                </Box>
                <Tooltip title="Sign out">
                  <IconButton size="small" onClick={async () => {
                    const isLocal = window.location.hostname.includes('localhost');
                    const home = isLocal ? 'http://localhost:3000/' : 'https://cerulea.app/';
                    window.location.href = `/api/auth/force-signout?next=${encodeURIComponent(home)}`;
                  }} sx={{ color: 'rgba(255,255,255,0.3)', '&:hover': { color: '#f44336' } }}>
                    <LogoutIcon sx={{ fontSize: 14 }} />
                  </IconButton>
                </Tooltip>
              </Stack>
            </Box>
          )}
        </Box>

        {/* Main area */}
        <Box
          sx={{
            flex: 1,
            ml: `${drawerWidth}px`,
            transition: 'margin-left 0.2s ease',
            height: '100vh',
            display: 'flex',
            flexDirection: 'column',
            bgcolor: 'background.default',
            overflow: 'hidden',
          }}
        >
          {/* Top bar */}
          <Box
            sx={{
              height: 64,
              display: 'flex',
              alignItems: 'center',
              px: 3,
              gap: 2,
              borderBottom: '1px solid rgba(255,255,255,0.07)',
              bgcolor: 'rgba(8,11,18,0.96)',
              backdropFilter: 'blur(20px)',
              position: 'sticky',
              top: 0,
              zIndex: 1100,
              flexShrink: 0,
            }}
          >
            {/* Global search */}
            <TextField
              size="small"
              placeholder="Search users, projects, events..."
              InputProps={{
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchIcon sx={{ fontSize: 16, color: 'rgba(255,255,255,0.3)' }} />
                  </InputAdornment>
                ),
                sx: {
                  fontSize: '0.82rem',
                  bgcolor: 'rgba(255,255,255,0.04)',
                  borderRadius: 2,
                  '& fieldset': { borderColor: 'rgba(255,255,255,0.08) !important' },
                  '&:hover fieldset': { borderColor: 'rgba(255,255,255,0.15) !important' },
                },
              }}
              sx={{ width: 340 }}
            />

            <Box sx={{ flex: 1 }} />

            {/* Environment badge */}
            <Chip
              label="PRODUCTION"
              size="small"
              sx={{
                bgcolor: alpha('#4caf50', 0.12),
                color: '#4caf50',
                border: `1px solid ${alpha('#4caf50', 0.25)}`,
                fontWeight: 800,
                fontSize: '0.62rem',
                letterSpacing: 1,
                height: 22,
              }}
            />

            {/* Alerts bell */}
            <Tooltip title="3 critical alerts">
              <IconButton size="small" sx={{ color: 'rgba(255,255,255,0.5)' }}>
                <Badge badgeContent={3} color="error" sx={{ '& .MuiBadge-badge': { fontSize: '0.55rem', height: 14, minWidth: 14 } }}>
                  <NotificationsIcon fontSize="small" />
                </Badge>
              </IconButton>
            </Tooltip>

            {/* Admin profile */}
            <Tooltip title={session?.user?.email || 'Admin'}>
              <Avatar
                sx={{
                  width: 32,
                  height: 32,
                  bgcolor: alpha('#448aff', 0.2),
                  fontSize: '0.75rem',
                  fontWeight: 900,
                  color: '#448aff',
                  cursor: 'pointer',
                  border: '1px solid rgba(68,138,255,0.3)',
                }}
              >
                {(session?.user?.name || 'A')[0].toUpperCase()}
              </Avatar>
            </Tooltip>
          </Box>

          {/* Page content */}
          <Box component="main" sx={{ flex: 1, p: { xs: 3, md: 4 }, overflowY: 'auto' }}>
            {children}
          </Box>
        </Box>
      </Box>
    </ThemeProvider>
  );
}
