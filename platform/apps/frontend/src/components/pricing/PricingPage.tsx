'use client';

import {
  Box, Typography, Card, CardContent, Button, Stack, Chip, List,
  ListItem, ListItemIcon, ListItemText, CircularProgress, Alert, Divider
} from '@mui/material';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import RadioButtonUncheckedIcon from '@mui/icons-material/RadioButtonUnchecked';
import CreditCardIcon from '@mui/icons-material/CreditCard';
import CancelIcon from '@mui/icons-material/Cancel';
import AutorenewIcon from '@mui/icons-material/Autorenew';
import { useTheme, alpha, keyframes } from '@mui/material/styles';
import { useState } from 'react';
import { useSession } from 'next-auth/react';
import { useSearchParams } from 'next/navigation';

const shimmer = keyframes`
  0%   { background-position: -400px 0; }
  100% { background-position: 400px 0; }
`;

const pulse = keyframes`
  0%, 100% { opacity: 1; }
  50%       { opacity: 0.6; }
`;

interface PlanFeature {
  text: string;
  included: boolean;
}

interface Plan {
  id: 'developer' | 'pro' | 'enterprise';
  label: string;
  price: string;
  period: string;
  tagline: string;
  cta: string;
  popular: boolean;
  features: PlanFeature[];
}

const PLANS: Plan[] = [
  {
    id: 'developer',
    label: 'DEVELOPER',
    price: '₹14,999',
    period: 'per month',
    tagline: 'For individuals and small teams building public dApps and executing production pilots.',
    cta: 'Start 14-Day Free Trial',
    popular: false,
    features: [
      { text: 'Access to Cerulea Studio', included: true },
      { text: 'Deploy to Cerulea Public L1', included: true },
      { text: '100,000 RPC requests per day', included: true },
      { text: 'Standard community governance', included: true },
      { text: 'Community Discord support', included: true },
    ],
  },
  {
    id: 'pro',
    label: 'PRO',
    price: '₹55,000',
    period: 'per month',
    tagline: 'For scaling applications requiring dedicated indexing and staging environments.',
    cta: 'Start 14-Day Free Trial',
    popular: true,
    features: [
      { text: 'Everything in Developer', included: true },
      { text: 'Unlimited RPC requests', included: true },
      { text: 'Dedicated indexing nodes', included: true },
      { text: 'Staging and testnet environments', included: true },
      { text: 'Priority email support', included: true },
    ],
  },
  {
    id: 'enterprise',
    label: 'ENTERPRISE',
    price: 'Custom',
    period: 'yearly licensing',
    tagline: 'For organizations deploying sovereign Private Chains with strict compliance rules.',
    cta: 'Contact Sales',
    popular: false,
    features: [
      { text: 'Sovereign Private Chain deployment', included: true },
      { text: 'Bring your own cloud (AWS, GCP)', included: true },
      { text: 'Custom compliance and RBAC modules', included: true },
      { text: 'Node architecture review', included: true },
      { text: '24/7 dedicated engineering SLA', included: true },
    ],
  },
];

export default function PricingPage() {
  const theme = useTheme();
  const { data: session } = useSession();
  const searchParams = useSearchParams();
  const returnUrl = searchParams.get('return') || '';
  const [loading, setLoading] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSelect = async (plan: Plan) => {
    if (plan.id === 'enterprise') {
      window.location.href = 'mailto:sales@cerulea.app?subject=Enterprise Plan Inquiry';
      return;
    }

    setLoading(plan.id);
    setError(null);

    try {
      const res = await fetch('/api/stripe/checkout', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ plan: plan.id, returnUrl }),
      });

      const data = await res.json();

      if (!res.ok || !data.url) {
        throw new Error(data.error || 'Failed to start checkout');
      }

      window.location.href = data.url;
    } catch (err: any) {
      setError(err.message || 'Something went wrong. Please try again.');
      setLoading(null);
    }
  };

  const isDark = theme.palette.mode === 'dark';

  return (
    <Box
      sx={{
        minHeight: '100vh',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        px: { xs: 2, md: 4 },
        py: 8,
      }}
    >
      {/* ── Launch Trial Banner ── */}
      <Box
        sx={{
          width: '100%',
          maxWidth: 1100,
          mb: 6,
          borderRadius: 3,
          position: 'relative',
          overflow: 'hidden',
          // layered glow border
          p: '1.5px',
          background: `linear-gradient(135deg,
            ${theme.palette.primary.main} 0%,
            ${alpha('#10b981', 1)} 50%,
            ${theme.palette.primary.main} 100%)`,
          backgroundSize: '200% 200%',
          animation: `${shimmer} 4s linear infinite`,
        }}
      >
        <Box
          sx={{
            borderRadius: 'inherit',
            background: isDark
              ? 'linear-gradient(135deg, rgba(10,14,26,0.97) 0%, rgba(12,18,30,0.97) 100%)'
              : 'linear-gradient(135deg, #f0f4ff 0%, #ecfdf5 100%)',
            px: { xs: 3, md: 5 },
            py: { xs: 3, md: 3.5 },
          }}
        >
          <Stack
            direction={{ xs: 'column', sm: 'row' }}
            alignItems={{ xs: 'flex-start', sm: 'center' }}
            spacing={{ xs: 2, sm: 3 }}
          >
            {/* Big day count */}
            <Box sx={{ flexShrink: 0, textAlign: 'center', minWidth: 88 }}>
              <Typography
                sx={{
                  fontSize: { xs: '3.5rem', md: '4.5rem' },
                  fontWeight: 900,
                  lineHeight: 1,
                  background: `linear-gradient(135deg, ${theme.palette.primary.main}, #10b981)`,
                  WebkitBackgroundClip: 'text',
                  WebkitTextFillColor: 'transparent',
                  backgroundClip: 'text',
                }}
              >
                14
              </Typography>
              <Typography
                sx={{
                  fontSize: '0.65rem',
                  fontWeight: 800,
                  letterSpacing: 2,
                  color: 'text.secondary',
                  textTransform: 'uppercase',
                  mt: -0.5,
                }}
              >
                Day Free
              </Typography>
            </Box>

            {/* Divider line — desktop only */}
            <Box
              sx={{
                display: { xs: 'none', sm: 'block' },
                width: '1px',
                alignSelf: 'stretch',
                background: isDark ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.1)',
                flexShrink: 0,
              }}
            />

            {/* Copy */}
            <Box sx={{ flex: 1 }}>
              <Stack direction="row" alignItems="center" spacing={1} sx={{ mb: 0.75 }}>
                <Chip
                  label="LAUNCH OFFER"
                  size="small"
                  sx={{
                    height: 20,
                    fontSize: '0.6rem',
                    fontWeight: 800,
                    letterSpacing: 1.5,
                    background: `linear-gradient(90deg, ${theme.palette.primary.main}, #10b981)`,
                    color: '#fff',
                    animation: `${pulse} 2.4s ease-in-out infinite`,
                  }}
                />
                <Typography variant="caption" sx={{ opacity: 0.5 }}>
                  Developer &amp; Pro plans
                </Typography>
              </Stack>

              <Typography
                variant="h5"
                sx={{ fontWeight: 800, mb: 0.75, lineHeight: 1.25 }}
              >
                Start free. No risk. On us for 14 days.
              </Typography>

              <Stack
                direction={{ xs: 'column', sm: 'row' }}
                spacing={{ xs: 0.5, sm: 2.5 }}
                sx={{ mb: 1.25 }}
              >
                {[
                  { icon: <CreditCardIcon sx={{ fontSize: 14 }} />, text: 'Card required to start' },
                  { icon: <CancelIcon sx={{ fontSize: 14 }} />, text: 'Cancel anytime during trial' },
                  { icon: <AutorenewIcon sx={{ fontSize: 14 }} />, text: 'Auto-converts at trial end' },
                ].map(({ icon, text }) => (
                  <Stack key={text} direction="row" alignItems="center" spacing={0.6}>
                    <Box sx={{ color: '#10b981', display: 'flex' }}>{icon}</Box>
                    <Typography variant="caption" sx={{ opacity: 0.75, fontWeight: 500 }}>
                      {text}
                    </Typography>
                  </Stack>
                ))}
              </Stack>

              <Typography
                variant="caption"
                sx={{
                  opacity: 0.45,
                  display: 'block',
                  fontSize: '0.7rem',
                  borderTop: isDark ? '1px solid rgba(255,255,255,0.07)' : '1px solid rgba(0,0,0,0.07)',
                  pt: 1,
                }}
              >
                Trial available for new users during the launch window only. After launch, all new plans start at standard full price with no trial. Existing launch users retain their 14-day trial terms.
              </Typography>
            </Box>
          </Stack>
        </Box>
      </Box>

      {/* Header */}
      <Box sx={{ textAlign: 'center', mb: 6, maxWidth: 640 }}>
        <Typography
          variant="overline"
          sx={{ color: 'primary.main', fontWeight: 700, letterSpacing: 3, mb: 1, display: 'block' }}
        >
          CERULEA PRICING
        </Typography>
        <Typography variant="h3" sx={{ fontWeight: 800, mb: 2, lineHeight: 1.2 }}>
          Choose your plan
        </Typography>
        <Typography variant="body1" sx={{ opacity: 0.7, fontSize: '1.05rem' }}>
          Start free in the Studio. Upgrade when you're ready to deploy to production.
          All plans include access to Cerulea AI and the full module library.
        </Typography>

        {session?.user && (
          <Typography variant="body2" sx={{ mt: 2, opacity: 0.6 }}>
            Logged in as <strong>{session.user.email}</strong>
          </Typography>
        )}
      </Box>

      {error && (
        <Alert severity="error" sx={{ mb: 4, maxWidth: 900, width: '100%' }} onClose={() => setError(null)}>
          {error}
        </Alert>
      )}

      {/* Plan cards */}
      <Stack
        direction={{ xs: 'column', md: 'row' }}
        spacing={3}
        sx={{ width: '100%', maxWidth: 1100, alignItems: 'stretch' }}
      >
        {PLANS.map((plan) => {
          const isPopular = plan.popular;
          const isLoading = loading === plan.id;

          return (
            <Card
              key={plan.id}
              elevation={0}
              sx={{
                flex: 1,
                borderRadius: 3,
                border: isPopular
                  ? `2px solid ${theme.palette.primary.main}`
                  : `1px solid ${alpha(theme.palette.divider, 0.3)}`,
                background: isDark
                  ? alpha(theme.palette.background.paper, 0.6)
                  : theme.palette.background.paper,
                backdropFilter: 'blur(12px)',
                position: 'relative',
                overflow: 'visible',
                transition: 'transform 0.2s, box-shadow 0.2s',
                '&:hover': {
                  transform: 'translateY(-4px)',
                  boxShadow: isPopular
                    ? `0 12px 40px ${alpha(theme.palette.primary.main, 0.25)}`
                    : `0 8px 30px ${alpha(theme.palette.common.black, 0.15)}`,
                },
              }}
            >
              {/* Most Popular badge */}
              {isPopular && (
                <Box
                  sx={{
                    position: 'absolute',
                    top: -14,
                    left: '50%',
                    transform: 'translateX(-50%)',
                    zIndex: 1,
                  }}
                >
                  <Chip
                    label="MOST POPULAR"
                    size="small"
                    sx={{
                      backgroundColor: isDark ? 'rgba(30,30,40,0.95)' : 'white',
                      border: `1px solid ${theme.palette.primary.main}`,
                      color: theme.palette.primary.main,
                      fontWeight: 700,
                      fontSize: '0.65rem',
                      letterSpacing: 1.5,
                      px: 1,
                    }}
                  />
                </Box>
              )}

              <CardContent sx={{ p: 4, height: '100%', display: 'flex', flexDirection: 'column' }}>
                {/* Plan name */}
                <Typography
                  variant="overline"
                  sx={{
                    fontWeight: 700,
                    letterSpacing: 2,
                    color: isPopular ? 'primary.main' : 'text.secondary',
                    mb: 1.5,
                  }}
                >
                  {plan.label}
                </Typography>

                {/* Price */}
                <Typography
                  variant="h3"
                  sx={{ fontWeight: 800, lineHeight: 1, mb: 0.5 }}
                >
                  {plan.price}
                </Typography>
                <Typography variant="body2" sx={{ opacity: 0.6, mb: 2.5 }}>
                  {plan.period}
                </Typography>

                {/* Tagline */}
                <Typography
                  variant="body2"
                  sx={{
                    color: isPopular ? 'primary.main' : 'text.secondary',
                    mb: 3,
                    lineHeight: 1.6,
                    minHeight: 60,
                  }}
                >
                  {plan.tagline}
                </Typography>

                {/* CTA button */}
                <Button
                  variant={isPopular ? 'contained' : 'outlined'}
                  fullWidth
                  size="large"
                  disabled={isLoading}
                  onClick={() => handleSelect(plan)}
                  sx={{
                    mb: 3,
                    py: 1.5,
                    borderRadius: 2,
                    fontWeight: 700,
                    fontSize: '0.95rem',
                    ...(isPopular && {
                      background: `linear-gradient(135deg, ${theme.palette.primary.main}, ${theme.palette.primary.dark})`,
                      boxShadow: `0 4px 20px ${alpha(theme.palette.primary.main, 0.4)}`,
                    }),
                  }}
                >
                  {isLoading ? <CircularProgress size={20} color="inherit" /> : plan.cta}
                </Button>

                <Divider sx={{ mb: 3, opacity: 0.3 }} />

                {/* Features */}
                <Typography
                  variant="overline"
                  sx={{ fontWeight: 700, letterSpacing: 1.5, mb: 1.5, opacity: 0.7, display: 'block' }}
                >
                  INCLUDED FEATURES
                </Typography>

                <List dense disablePadding sx={{ flex: 1 }}>
                  {plan.features.map((feature, i) => (
                    <ListItem key={i} disableGutters sx={{ py: 0.5 }}>
                      <ListItemIcon sx={{ minWidth: 32 }}>
                        {feature.included ? (
                          <CheckCircleIcon
                            sx={{
                              fontSize: 18,
                              color: isPopular ? 'primary.main' : alpha(theme.palette.success.main, 0.9),
                            }}
                          />
                        ) : (
                          <RadioButtonUncheckedIcon sx={{ fontSize: 18, opacity: 0.3 }} />
                        )}
                      </ListItemIcon>
                      <ListItemText
                        primary={feature.text}
                        primaryTypographyProps={{ variant: 'body2', sx: { opacity: feature.included ? 1 : 0.4 } }}
                      />
                    </ListItem>
                  ))}
                </List>
              </CardContent>
            </Card>
          );
        })}
      </Stack>

      {/* Footer note */}
      <Typography variant="caption" sx={{ mt: 5, opacity: 0.5, textAlign: 'center', maxWidth: 700 }}>
        14-day free trial requires a valid payment card. No charge during the trial period. If not canceled before
        day 14, the subscription auto-converts to the selected plan at the standard monthly price.
        All prices in INR, exclusive of applicable taxes. Cancel anytime.
      </Typography>
    </Box>
  );
}
