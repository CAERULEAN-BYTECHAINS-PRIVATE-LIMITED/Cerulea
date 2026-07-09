import React from 'react';
import { Box, Typography, Tooltip } from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import { glossaryTooltip, GLOSSARY } from '@/data/glossary';

/* ------------------------------------------------------------------ */
/* StepDescription "What you're doing here" header banner            */
/* ------------------------------------------------------------------ */

interface StepDescriptionProps {
  step: number;
  total?: number;
  stepName: string;
  title: string;
  description: string;
  trackLabel?: string;
}

export function StepDescription({
  step, total = 6, stepName, title, description, trackLabel,
}: StepDescriptionProps) {
  return (
    <Box sx={{ mb: 3 }}>
      <Typography variant="overline" fontWeight={800} color="primary" sx={{ letterSpacing: 1 }}>
        STEP {step} OF {total} : {stepName.toUpperCase()}{trackLabel ? ` · ${trackLabel.toUpperCase()}` : ''}
      </Typography>
      <Typography variant="h4" fontWeight={900}>{title}</Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, maxWidth: 700 }}>
        {description}
      </Typography>
    </Box>
  );
}

/* ------------------------------------------------------------------ */
/* SectionLabel subtitle for a tab or section within a step          */
/* ------------------------------------------------------------------ */

interface SectionLabelProps {
  title: string;
  subtitle?: string;
}

export function SectionLabel({ title, subtitle }: SectionLabelProps) {
  return (
    <Box sx={{ mb: 3 }}>
      <Typography variant="h5" fontWeight={800}>{title}</Typography>
      {subtitle && (
        <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
          {subtitle}
        </Typography>
      )}
    </Box>
  );
}

/* ------------------------------------------------------------------ */
/* FieldHelp (?) icon that shows a tooltip when hovered              */
/* ------------------------------------------------------------------ */

interface FieldHelpProps {
  /** Either a glossary key or a custom description string */
  glossaryKey?: string;
  description?: string;
  example?: string;
  size?: 'small' | 'medium';
}

export function FieldHelp({ glossaryKey, description, example, size = 'small' }: FieldHelpProps) {
  const theme = useTheme();

  const entry = glossaryKey ? GLOSSARY[glossaryKey] : null;
  const label = entry ? `${entry.term}: ${entry.definition}${entry.example ? `\n\nExample: ${entry.example}` : ''}` : (description || '');
  const fullLabel = example ? `${label}\n\nExample: ${example}` : label;

  if (!fullLabel) return null;

  return (
    <Tooltip
      title={<Box sx={{ whiteSpace: 'pre-line', fontSize: '0.78rem', lineHeight: 1.5 }}>{fullLabel}</Box>}
      placement="right"
      enterDelay={200}
      arrow
    >
      <InfoOutlinedIcon
        sx={{
          fontSize: size === 'small' ? 14 : 18,
          color: 'text.disabled',
          cursor: 'help',
          ml: 0.5,
          verticalAlign: 'middle',
          '&:hover': { color: theme.palette.primary.main },
          flexShrink: 0,
        }}
      />
    </Tooltip>
  );
}

/* ------------------------------------------------------------------ */
/* EmptyStateGuide banner shown when a section has no content yet    */
/* ------------------------------------------------------------------ */

interface EmptyStateGuideProps {
  icon?: React.ReactNode;
  primary: string;
  secondary?: string;
}

export function EmptyStateGuide({ icon, primary, secondary }: EmptyStateGuideProps) {
  const theme = useTheme();
  return (
    <Box
      sx={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 1,
        py: 6,
        color: 'text.secondary',
        opacity: 0.7,
      }}
    >
      {icon && <Box sx={{ fontSize: '2.5rem', mb: 1 }}>{icon}</Box>}
      <Typography variant="body1" fontWeight={600} align="center">{primary}</Typography>
      {secondary && <Typography variant="body2" align="center" sx={{ maxWidth: 340 }}>{secondary}</Typography>}
    </Box>
  );
}
