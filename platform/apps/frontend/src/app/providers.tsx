'use client';

import { ThemeProvider } from '@mui/material/styles';
import CssBaseline from '@mui/material/CssBaseline';
import { createContext, useState, useMemo, ReactNode, useContext } from 'react';
import { SessionProvider } from 'next-auth/react';
import { finalLightTheme, finalDarkTheme } from '@/theme/theme';

export const ThemeContext = createContext({
  toggleTheme: () => {},
});

export const useThemeToggle = () => useContext(ThemeContext);

export function Providers({ children }: { children: ReactNode }) {
  const [mode, setMode] = useState<'light' | 'dark'>('light');

  const themeToggle = useMemo(
    () => ({
      toggleTheme: () => {
        setMode((prevMode) => (prevMode === 'light' ? 'dark' : 'light'));
      },
    }),
    []
  );

  const theme = useMemo(() => (mode === 'light' ? finalLightTheme : finalDarkTheme), [mode]);

  return (
    <SessionProvider refetchOnWindowFocus={false} refetchInterval={0}>
      <ThemeContext.Provider value={themeToggle}>
        <ThemeProvider theme={theme}>
          <CssBaseline />
          {children}
        </ThemeProvider>
      </ThemeContext.Provider>
    </SessionProvider>
  );
}

