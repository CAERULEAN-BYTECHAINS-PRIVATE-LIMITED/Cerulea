'use client';
import { createTheme, responsiveFontSizes, alpha } from '@mui/material/styles';

const commonSettings = {
  typography: { fontFamily: ['Inter', 'sans-serif'].join(',') },
  shape: { borderRadius: 16 },
};

// --- Light Glass Theme ---
let lightTheme = createTheme({
  palette: {
    mode: 'light',
    primary: { main: '#3d5afe' },
    secondary: { main: '#9c27b0' },
    background: { default: '#f4f6fa', paper: '#ffffff' },
    text: { primary: '#172b4d', secondary: '#5e6c84' },
  },
  ...commonSettings,
});

lightTheme = createTheme(lightTheme, {
  components: {
    MuiAppBar: { styleOverrides: { root: { color: lightTheme.palette.text.primary }}},
    MuiButton: { styleOverrides: { root: { textTransform: 'none', fontWeight: 600, borderRadius: '999px', boxShadow: 'none' }}},
    MuiTooltip: {
      styleOverrides: {
        tooltip: {
          backgroundColor: 'rgba(255,255,255,0.98)',
          color: '#172b4d',
          border: '1px solid rgba(0,0,0,0.12)',
          backdropFilter: 'blur(12px)',
          boxShadow: '0 8px 32px rgba(0,0,0,0.14)',
          borderRadius: 8,
          fontSize: '0.78rem',
          padding: '8px 12px',
        },
        arrow: { color: 'rgba(255,255,255,0.98)' },
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: {
          backgroundColor: '#ffffff',
          backgroundImage: 'none',
        },
      },
    },
    MuiPopover: {
      styleOverrides: {
        paper: {
          backgroundColor: '#ffffff',
          backgroundImage: 'none',
        },
      },
    },
    MuiMenu: {
      styleOverrides: {
        paper: {
          backgroundColor: '#ffffff',
          backgroundImage: 'none',
        },
      },
    },
  },
});

// --- Dark Glass Theme ---
let darkTheme = createTheme({
  palette: {
    mode: 'dark',
    primary: { main: '#448aff' },
    secondary: { main: '#ce93d8' },
    background: { default: '#0d1117', paper: '#1a2035' },
    text: { primary: '#e6edf3', secondary: '#8b949e' },
  },
  ...commonSettings,
});

darkTheme = createTheme(darkTheme, {
  components: {
    MuiButton: { ...lightTheme.components?.MuiButton },
    MuiTooltip: {
      styleOverrides: {
        tooltip: {
          backgroundColor: 'rgba(20,20,24,0.98)',
          color: '#e6edf3',
          border: '1px solid rgba(255,255,255,0.1)',
          backdropFilter: 'blur(12px)',
          boxShadow: '0 8px 32px rgba(0,0,0,0.4)',
          borderRadius: 8,
          fontSize: '0.78rem',
          padding: '8px 12px',
        },
        arrow: { color: 'rgba(20,20,24,0.98)' },
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: {
          backgroundColor: '#1a1f2e',
          backgroundImage: 'none',
        },
      },
    },
    MuiPopover: {
      styleOverrides: {
        paper: {
          backgroundColor: '#1a1f2e',
          backgroundImage: 'none',
        },
      },
    },
    MuiMenu: {
      styleOverrides: {
        paper: {
          backgroundColor: '#1a1f2e',
          backgroundImage: 'none',
        },
      },
    },
  },
});

export const finalLightTheme = responsiveFontSizes(lightTheme);
export const finalDarkTheme = responsiveFontSizes(darkTheme);