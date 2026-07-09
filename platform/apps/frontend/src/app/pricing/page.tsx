import { Suspense } from 'react';
import PricingPage from '@/components/pricing/PricingPage';

export const metadata = {
  title: 'Pricing: Cerulea',
  description: 'Choose the right plan for your blockchain project.',
};

export default function PricingRoute() {
  return (
    <Suspense>
      <PricingPage />
    </Suspense>
  );
}
