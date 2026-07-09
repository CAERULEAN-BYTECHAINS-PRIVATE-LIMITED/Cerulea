'use client';

import React, { useState } from 'react';
import {
  Box, List, ListItem, ListItemButton, ListItemIcon, ListItemText,
  Typography, IconButton, Tooltip, Paper,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import { usePathname } from 'next/navigation';
import Link from 'next/link';

// Icons
import DashboardIcon from '@mui/icons-material/Dashboard';
import FolderOpenIcon from '@mui/icons-material/FolderOpen';
import NetworkCheckIcon from '@mui/icons-material/NetworkCheck';
import DnsIcon from '@mui/icons-material/Dns';
import KeyIcon from '@mui/icons-material/Key';
import AccountBalanceIcon from '@mui/icons-material/AccountBalance';
import HexagonOutlinedIcon from '@mui/icons-material/HexagonOutlined';
import DescriptionIcon from '@mui/icons-material/Description';
import SaveIcon from '@mui/icons-material/Save';
import HubIcon from '@mui/icons-material/Hub';
import SettingsIcon from '@mui/icons-material/Settings';
import AddCircleOutlineIcon from '@mui/icons-material/AddCircleOutline';
import ChevronLeftIcon from '@mui/icons-material/ChevronLeft';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import CeruleaLogoIcon from '@mui/icons-material/Hexagon';

const DRAWER_WIDTH = 240;
const DRAWER_COLLAPSED = 64;

const NAV_ITEMS = [
  { label: 'Overview', href: '/dashboard', icon: <DashboardIcon fontSize="small" /> },
  { label: 'Projects', href: '/dashboard/projects', icon: <FolderOpenIcon fontSize="small" /> },
  { label: 'Networks', href: '/dashboard/networks', icon: <NetworkCheckIcon fontSize="small" /> },
  { label: 'Nodes', href: '/dashboard/nodes', icon: <DnsIcon fontSize="small" /> },
  { label: 'Keys & Access', href: '/dashboard/keys', icon: <KeyIcon fontSize="small" /> },
  { label: 'Governance', href: '/dashboard/governance', icon: <AccountBalanceIcon fontSize="small" /> },
  { label: 'Smart Contracts', href: '/dashboard/contracts', icon: <HexagonOutlinedIcon fontSize="small" /> },
  { label: 'Audit Logs', href: '/dashboard/audit', icon: <DescriptionIcon fontSize="small" /> },
  { label: 'State Snapshots', href: '/dashboard/state', icon: <SaveIcon fontSize="small" /> },
  { label: 'Integrations', href: '/dashboard/integrations', icon: <HubIcon fontSize="small" /> },
  { label: 'Settings', href: '/dashboard/settings', icon: <SettingsIcon fontSize="small" /> },
];

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  const theme = useTheme();
  const pathname = usePathname();
  const [collapsed, setCollapsed] = useState(false);

  const drawerWidth = collapsed ? DRAWER_COLLAPSED : DRAWER_WIDTH;

  return (
    <Box sx={{ display: 'flex', minHeight: 'calc(100vh - 64px)', bgcolor: 'background.default' }}>
      {/* Sidebar */}
      <Box
        component="nav"
        sx={{
          width: drawerWidth,
          flexShrink: 0,
          transition: 'width 0.2s ease',
          position: 'fixed',
          top: 64,
          left: 0,
          height: 'calc(100vh - 64px)',
          zIndex: 1200,
          bgcolor: theme.palette.mode === 'dark' ? 'rgba(10,10,14,0.97)' : 'rgba(255,255,255,0.97)',
          backdropFilter: 'blur(20px)',
          borderRight: `1px solid ${theme.palette.divider}`,
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
        }}
      >
        {/* Logo */}
        <Box
          sx={{
            height: 64,
            display: 'flex',
            alignItems: 'center',
            px: collapsed ? 1.5 : 2.5,
            gap: 1.5,
            borderBottom: `1px solid ${theme.palette.divider}`,
            flexShrink: 0,
          }}
        >
          <CeruleaLogoIcon sx={{ color: 'primary.main', fontSize: 28 }} />
          {!collapsed && (
            <Typography variant="h6" fontWeight={900} sx={{ letterSpacing: -0.5 }}>
              Cerulea
            </Typography>
          )}
          <Box sx={{ flex: 1 }} />
          <IconButton size="small" onClick={() => setCollapsed((c) => !c)}>
            {collapsed ? <ChevronRightIcon fontSize="small" /> : <ChevronLeftIcon fontSize="small" />}
          </IconButton>
        </Box>

        {/* New Project Button */}
        {!collapsed && (
          <Box sx={{ px: 2, py: 1.5 }}>
            <Paper
              onClick={() => {
                const isLocal = window.location.hostname.includes('localhost');
                window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
              }}
              sx={{
                display: 'flex',
                alignItems: 'center',
                gap: 1,
                px: 2,
                py: 1,
                borderRadius: 2,
                cursor: 'pointer',
                bgcolor: alpha(theme.palette.primary.main, 0.1),
                border: `1px solid ${alpha(theme.palette.primary.main, 0.2)}`,
                color: 'primary.main',
                fontWeight: 700,
                fontSize: '0.8rem',
                transition: 'all 0.15s',
                '&:hover': { bgcolor: alpha(theme.palette.primary.main, 0.18) },
              }}
              elevation={0}
            >
              <AddCircleOutlineIcon fontSize="small" />
              New Project
            </Paper>
          </Box>
        )}

        {/* Nav Items */}
        <List sx={{ flex: 1, overflowY: 'auto', py: 0.5 }}>
          {NAV_ITEMS.map((item) => {
            const isActive =
              item.href === '/dashboard'
                ? pathname === '/dashboard'
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
                      borderRadius: 2,
                      minHeight: 40,
                      px: collapsed ? 1.5 : 2,
                      color: isActive ? 'primary.main' : 'text.secondary',
                      fontWeight: isActive ? 700 : 500,
                      '&.Mui-selected': {
                        bgcolor: alpha(theme.palette.primary.main, 0.1),
                        '&:hover': { bgcolor: alpha(theme.palette.primary.main, 0.14) },
                      },
                      '&:hover': {
                        bgcolor: alpha(theme.palette.action.hover, 0.5),
                        color: 'text.primary',
                      },
                    }}
                  >
                    <ListItemIcon
                      sx={{
                        minWidth: collapsed ? 0 : 36,
                        color: isActive ? 'primary.main' : 'inherit',
                      }}
                    >
                      {item.icon}
                    </ListItemIcon>
                    {!collapsed && (
                      <ListItemText
                        primary={item.label}
                        primaryTypographyProps={{ fontSize: '0.825rem', fontWeight: isActive ? 700 : 500 }}
                      />
                    )}
                  </ListItemButton>
                </ListItem>
              </Tooltip>
            );
          })}
        </List>

        {/* Footer */}
        {!collapsed && (
          <Box sx={{ p: 2, borderTop: `1px solid ${theme.palette.divider}` }}>
            <Typography variant="caption" color="text.disabled">
              Cerulea Studio v0.1
            </Typography>
          </Box>
        )}
      </Box>

      {/* Main Content */}
      <Box
        component="main"
        sx={{
          flex: 1,
          ml: `${drawerWidth}px`,
          transition: 'margin-left 0.2s ease',
          minHeight: '100%',
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        {children}
      </Box>
    </Box>
  );
}
