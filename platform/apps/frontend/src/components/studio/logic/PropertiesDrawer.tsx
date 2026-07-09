'use client';

import * as React from 'react';
import { Drawer, Box, Typography, Divider, TextField, Stack, Switch, FormControlLabel } from '@mui/material';
import { Node, Edge } from 'reactflow';

type Props = {
  node: Node | null;
  edge: Edge | null;
  onNodeChange: (patch: any) => void;
  onEdgeChange: (patch: any) => void;
};

function renderField(
  key: string,
  schema: any,
  value: any,
  onChange: (v: any) => void
) {
  const type = schema?.type || typeof value || 'string';
  if (type === 'boolean') {
    return (
      <FormControlLabel
        key={key}
        control={<Switch checked={!!value} onChange={e => onChange(e.target.checked)} />}
        label={schema?.title || key}
      />
    );
  }
  return (
    <TextField
      key={key}
      fullWidth size="small" margin="dense"
      label={schema?.title || key}
      value={value ?? ''}
      onChange={e => onChange(e.target.value)}
      helperText={schema?.description || ''}
    />
  );
}

export default function PropertiesDrawer({ node, edge, onNodeChange, onEdgeChange }: Props) {
  const open = !!node || !!edge;

  // Node schema → from node.data.propertiesSchema
  const nodeSchema = node?.data?.propertiesSchema || null;
  const nodeValues = node?.data?.properties || {};

  // Edge schema (defaults)
  const edgeSchema = {
    label: { type: 'string', title: 'Label', description: 'Shown near the connection arrow' },
    condition: { type: 'string', title: 'Condition', description: 'Optional expression to gate this path' },
    priority: { type: 'number', title: 'Priority', description: 'Lower runs first when multiple edges leave a block' },
    timeoutMs: { type: 'number', title: 'Timeout (ms)', description: 'Failover or switch path after this duration' },
    retryTimes: { type: 'number', title: 'Retry – times', description: 'Number of retries on failure' },
    retryIntervalMs: { type: 'number', title: 'Retry – interval (ms)', description: 'Delay between retries' }
  } as const;

  return (
    <Drawer anchor="right" open={open} variant="persistent" PaperProps={{ sx: { width: 340 } }}>
      <Box sx={{ p: 2 }}>
        {node && (
          <>
            <Typography variant="subtitle2">Block properties</Typography>
            <Typography variant="caption" sx={{ opacity: 0.7 }}>
              {node.data?.label}
            </Typography>
            <Divider sx={{ my: 1 }} />
            {/* Editable title */}
            {renderField('title', { title: 'Title' }, node.data?.label, v => onNodeChange({ label: v }))}
            {/* Schema-driven properties */}
            {nodeSchema && Object.keys(nodeSchema.properties || {}).map(k =>
              renderField(k, nodeSchema.properties[k], nodeValues[k], (v) => {
                const next = { ...(nodeValues || {}), [k]: v };
                onNodeChange({ properties: next });
              })
            )}
          </>
        )}

        {edge && (
          <>
            <Typography variant="subtitle2">Edge properties</Typography>
            <Typography variant="caption" sx={{ opacity: 0.7 }}>
              {edge.id}
            </Typography>
            <Divider sx={{ my: 1 }} />
            {Object.entries(edgeSchema).map(([k, s]) =>
              renderField(k, s, edge.data?.[k], (v) => onEdgeChange({ [k]: v }))
            )}
          </>
        )}
      </Box>
    </Drawer>
  );
}
