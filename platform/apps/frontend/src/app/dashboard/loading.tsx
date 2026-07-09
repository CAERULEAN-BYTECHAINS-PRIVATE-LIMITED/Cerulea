'use client';
import { Box, Card, CardContent, Skeleton, Stack } from '@mui/material';

export default function Loading() {
  return (
    <Box sx={{ p: 4, maxWidth: 1200 }}>
      <Skeleton width={260} height={42} sx={{ mb: 1 }} />
      <Skeleton width={200} height={24} sx={{ mb: 4 }} />

      <Stack direction="row" spacing={2.5} mb={4}>
        {[...Array(4)].map((_, i) => (
          <Card key={i} variant="outlined" sx={{ flex: 1, borderRadius: 3 }}>
            <CardContent>
              <Skeleton width="60%" height={20} />
              <Skeleton height={40} />
            </CardContent>
          </Card>
        ))}
      </Stack>

      <Stack direction="row" spacing={3}>
        <Card variant="outlined" sx={{ flex: 2, borderRadius: 3 }}>
          <CardContent>
            {[...Array(5)].map((_, i) => <Skeleton key={i} height={56} sx={{ mb: 1 }} />)}
          </CardContent>
        </Card>
        <Card variant="outlined" sx={{ flex: 1, borderRadius: 3 }}>
          <CardContent>
            {[...Array(5)].map((_, i) => <Skeleton key={i} height={40} sx={{ mb: 1 }} />)}
          </CardContent>
        </Card>
      </Stack>
    </Box>
  );
}
