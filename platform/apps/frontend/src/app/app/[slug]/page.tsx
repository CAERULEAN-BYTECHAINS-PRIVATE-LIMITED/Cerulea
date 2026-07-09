'use client';

import { useEffect, useState } from 'react';
import { useParams } from 'next/navigation';
import {
  Box,
  Typography,
  Paper,
  CircularProgress,
  TextField,
  Button,
  MenuItem,
  Select,
  InputLabel,
  FormControl,
  Container,
} from '@mui/material';

type RuntimeConfig = {
  appMetadata: any;
  tokenomics: any;
  accessControls: any;
  logicFlow: any;
  aiConfig: any;
  uiConfig: any;
};

export default function DeployedAppPage() {
  const { slug } = useParams();
  const [config, setConfig] = useState<RuntimeConfig | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function loadConfig() {
      try {
        const base = `http://localhost:4000/deployed-apps/${slug}`;
        const [appMetadata, tokenomics, accessControls, logicFlow, aiConfig, uiConfig] = await Promise.all([
          fetch(`${base}/appMetadata.json`).then(res => res.json()),
          fetch(`${base}/tokenomics.json`).then(res => res.json()),
          fetch(`${base}/accessControls.json`).then(res => res.json()),
          fetch(`${base}/logicFlow.json`).then(res => res.json()),
          fetch(`${base}/aiConfig.json`).then(res => res.json()),
          fetch(`${base}/uiConfig.json`).then(res => res.json()),
        ]);

        setConfig({
          appMetadata,
          tokenomics,
          accessControls,
          logicFlow,
          aiConfig,
          uiConfig,
        });
      } catch (err) {
        console.error('Failed to load config', err);
      } finally {
        setLoading(false);
      }
    }

    loadConfig();
  }, [slug]);

  if (loading) return <CircularProgress sx={{ m: 4 }} />;
  if (!config) return <Typography sx={{ m: 4 }}>Failed to load app config.</Typography>;

  const { appMetadata, uiConfig } = config;

  return (
    <Container maxWidth="md" sx={{ py: 6 }}>
      <Typography variant="h3" gutterBottom>
        {appMetadata?.name || 'Untitled App'}
      </Typography>
      <Typography variant="subtitle1" gutterBottom>
        {appMetadata?.description || 'No description provided.'}
      </Typography>

      {Array.isArray(uiConfig) && uiConfig.length > 0 ? (
        <Box mt={4} display="flex" flexDirection="column" gap={3}>
          {uiConfig.map((component: any, index: number) => {
            const { type, props = {} } = component;

            switch (type) {
              case 'title':
                return <Typography key={index} variant="h4">{props.label || 'Title'}</Typography>;

              case 'paragraph':
                return <Typography key={index} variant="body1">{props.label || 'Paragraph'}</Typography>;

              case 'input':
                return (
                  <TextField
                    key={index}
                    label={props.label || 'Input'}
                    placeholder={props.placeholder || ''}
                    fullWidth
                  />
                );

              case 'select':
                return (
                  <FormControl key={index} fullWidth>
                    <InputLabel>{props.label || 'Select'}</InputLabel>
                    <Select label={props.label || 'Select'} defaultValue="">
                      {(props.options || ['Option 1', 'Option 2']).map((option: string, idx: number) => (
                        <MenuItem key={idx} value={option}>
                          {option}
                        </MenuItem>
                      ))}
                    </Select>
                  </FormControl>
                );

              case 'button':
                return (
                  <Button
                    key={index}
                    variant="contained"
                    color="primary"
                    onClick={() => alert(`${props.label || 'Button'} clicked`)}
                  >
                    {props.label || 'Click Me'}
                  </Button>
                );

              default:
                return (
                  <Typography key={index} variant="body2" color="text.secondary">
                    Unknown component: {type}
                  </Typography>
                );
            }
          })}
        </Box>
      ) : (
        <Typography variant="body2" color="text.secondary" mt={4}>
          No UI components defined for this app.
        </Typography>
      )}
    </Container>
  );
}
