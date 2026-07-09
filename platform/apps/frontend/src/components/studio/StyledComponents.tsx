// FILE: apps/frontend/src/components/studio/StyledComponents.tsx
import { Paper } from '@mui/material';
import { styled } from '@mui/material/styles';

export const GlassPaper = styled(Paper)(({ theme }) => ({
  padding: theme.spacing(3),
  borderRadius: Number(theme.shape.borderRadius) * 2.5,
  // Glassmorphism styles from our theme
  backdropFilter: 'var(--glass-blur-effect)',
  backgroundColor: 'var(--glass-bg-color)',
  border: '1px solid var(--glass-border-color)',
  // A subtle shadow for depth
  boxShadow: '0 8px 32px 0 rgba(0, 0, 0, 0.15)',
  // Ensure text inside is readable
  color: 'var(--primary-text-color)',
}));