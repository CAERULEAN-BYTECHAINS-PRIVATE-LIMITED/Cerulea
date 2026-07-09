'use client';

import { Box, Button } from '@mui/material';
import { useRouter } from 'next/navigation';
import { useStudio } from '@/context/StudioContext';
import { saveToBackend } from '@/lib/saveToBackend';

export default function StepperNav({
  next,
  back,
  onNext,
  onBack,
}: {
  next?: string;
  back?: string;
  onNext?: () => void;
  onBack?: () => void;
}) {
  const router = useRouter();
  const studio = useStudio();

  const handleBack = async () => {
    if (onBack) await onBack();
    if (back) router.push(back);
  };

  const handleNext = async () => {
    await saveToBackend(studio, { step: 'nav' });
    if (onNext) await onNext();
    if (next) router.push(next);
  };

  return (
    <Box display="flex" justifyContent="space-between" mt={4}>
      <Button
        variant="outlined"
        disabled={!back}
        onClick={handleBack}
      >
        Back
      </Button>
      <Button
        variant="contained"
        disabled={!next}
        onClick={handleNext}
      >
        Continue
      </Button>
    </Box>
  );
}
