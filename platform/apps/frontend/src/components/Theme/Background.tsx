'use client';

import { Box, useTheme } from '@mui/material';

export default function Background() {
  const theme = useTheme();

  const backgroundGradient =
    theme.palette.mode === 'dark'
      ? `radial-gradient(at 27% 37%, hsla(215, 98%, 62%, 0.15) 0px, transparent 50%),
         radial-gradient(at 97% 21%, hsla(125, 98%, 72%, 0.1) 0px, transparent 50%),
         radial-gradient(at 52% 99%, hsla(355, 98%, 76%, 0.15) 0px, transparent 50%)`
      : `radial-gradient(at 27% 37%, hsla(215, 98%, 62%, 0.1) 0px, transparent 50%),
         radial-gradient(at 97% 21%, hsla(125, 98%, 72%, 0.05) 0px, transparent 50%),
         radial-gradient(at 52% 99%, hsla(355, 98%, 76%, 0.1) 0px, transparent 50%)`;

  return (
    <Box
      sx={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100vw',
        height: '100vh',
        zIndex: -1, // Sits behind all other content
        backgroundColor: theme.palette.background.default,
        backgroundImage: backgroundGradient,
        transition: 'background-color 0.3s ease-in-out',
      }}
    />
  );
}