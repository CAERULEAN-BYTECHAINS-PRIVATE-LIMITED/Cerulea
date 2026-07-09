'use client';

import React, { useEffect, useMemo, useState } from "react";
import StepGuidance from '@/components/studio/StepGuidance';
import {
  Box, Button, Divider, IconButton, Stack, Typography, Paper, TextField, 
  Switch, FormControlLabel, Chip, Fade, InputAdornment, 
  CircularProgress, Tooltip, Link
} from "@mui/material";
import { useTheme, styled, alpha } from "@mui/material/styles";
import { useRouter, useSearchParams } from "next/navigation";

// Icons
import ArrowBackIcon from "@mui/icons-material/KeyboardArrowLeft";
import ArrowForwardIcon from "@mui/icons-material/KeyboardArrowRight";
import CheckCircleIcon from "@mui/icons-material/CheckCircle";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import PaymentsIcon from '@mui/icons-material/Payments';
import EmailIcon from '@mui/icons-material/Email';
import StorageIcon from '@mui/icons-material/Storage';
import WebhookIcon from '@mui/icons-material/Webhook';
import AnalyticsIcon from '@mui/icons-material/Analytics';
import SecurityIcon from '@mui/icons-material/Security';
import HubIcon from '@mui/icons-material/Hub';
import Visibility from '@mui/icons-material/Visibility';
import VisibilityOff from '@mui/icons-material/VisibilityOff';
import OpenInNewIcon from '@mui/icons-material/OpenInNew';
import QuestionMarkIcon from '@mui/icons-material/QuestionMark';
import CloseIcon from '@mui/icons-material/Close';
import Dialog from '@mui/material/Dialog';
import DialogTitle from '@mui/material/DialogTitle';
import DialogContent from '@mui/material/DialogContent';

/* ------------------ Types ------------------ */
type IntegrationCategory = "Payments" | "Auth" | "Communication" | "Storage" | "Data" | "Analytics" | "Webhooks";

type IntegrationDef = {
  id: string;
  name: string;
  category: IntegrationCategory;
  description: string;
  docsUrl?: string;
  fields: Array<{ 
    key: string; 
    label: string; 
    type: "text" | "password"; 
    placeholder?: string;
    hint?: string;
    validate?: (val: string) => string | null;
  }>;
};

type IntegrationConfig = {
  enabled: boolean;
  environment: "dev" | "prod";
  credentials: Record<string, string>;
  settings?: Record<string, any>;
};

const DEFAULT_CONFIG: IntegrationConfig = {
  enabled: false,
  environment: 'dev',
  credentials: {}
};

/* ------------------ 1. EXTENDED CATALOG (5+ Per Category) ------------------ */
const CATALOG: IntegrationDef[] = [
  // --- PAYMENTS ---
  {
    id: "stripe",
    name: "Stripe",
    category: "Payments",
    description: "The standard for online payments and subscriptions.",
    docsUrl: "https://stripe.com/docs",
    fields: [
      { key: "publishableKey", label: "Publishable Key", type: "text", placeholder: "pk_test_...", validate: v => !v.startsWith('pk_') ? 'Must start with pk_' : null },
      { key: "secretKey", label: "Secret Key", type: "password", placeholder: "sk_test_...", validate: v => !v.startsWith('sk_') ? 'Must start with sk_' : null },
      { key: "webhookSecret", label: "Webhook Secret", type: "password", placeholder: "whsec_..." },
    ]
  },
  {
    id: "paypal",
    name: "PayPal",
    category: "Payments",
    description: "Accept PayPal, Venmo, and Pay Later options.",
    fields: [
      { key: "clientId", label: "Client ID", type: "text" },
      { key: "clientSecret", label: "Client Secret", type: "password" },
      { key: "webhookId", label: "Webhook ID", type: "text" },
    ]
  },
  {
    id: "coinbase",
    name: "Coinbase Commerce",
    category: "Payments",
    description: "Accept crypto payments directly to your wallet.",
    fields: [
      { key: "apiKey", label: "API Key", type: "password" },
      { key: "webhookSecret", label: "Webhook Shared Secret", type: "password" },
    ]
  },
  {
    id: "lemonsqueezy",
    name: "Lemon Squeezy",
    category: "Payments",
    description: "Merchant of record for SaaS tax compliance.",
    fields: [
      { key: "apiKey", label: "API Key", type: "password" },
      { key: "storeId", label: "Store ID", type: "text" },
      { key: "webhookSecret", label: "Signing Secret", type: "password" },
    ]
  },
  {
    id: "razorpay",
    name: "Razorpay",
    category: "Payments",
    description: "Payment gateway solution for India.",
    fields: [
      { key: "keyId", label: "Key ID", type: "text", placeholder: "rzp_test_..." },
      { key: "keySecret", label: "Key Secret", type: "password" },
    ]
  },

  // --- AUTHENTICATION ---
  {
    id: "clerk",
    name: "Clerk",
    category: "Auth",
    description: "Complete user management UIs and APIs.",
    fields: [
      { key: "publishableKey", label: "Publishable Key", type: "text", placeholder: "pk_..." },
      { key: "secretKey", label: "Secret Key", type: "password", placeholder: "sk_..." },
    ]
  },
  {
    id: "privy",
    name: "Privy",
    category: "Auth",
    description: "Onboard users to Web3 with embedded wallets.",
    docsUrl: "https://docs.privy.io",
    fields: [
      { key: "appId", label: "App ID", type: "text" },
      { key: "appSecret", label: "App Secret", type: "password" },
      { key: "verificationKey", label: "Verification Key", type: "text", hint: "Public key for JWT verification" },
    ]
  },
  {
    id: "dynamic",
    name: "Dynamic",
    category: "Auth",
    description: "Multi-chain wallet authentication adapter.",
    fields: [
      { key: "environmentId", label: "Environment ID", type: "text" },
      { key: "apiSecret", label: "API Secret", type: "password" },
    ]
  },
  {
    id: "auth0",
    name: "Auth0",
    category: "Auth",
    description: "Enterprise grade identity management.",
    fields: [
      { key: "domain", label: "Domain", type: "text", placeholder: "tenant.auth0.com" },
      { key: "clientId", label: "Client ID", type: "text" },
      { key: "clientSecret", label: "Client Secret", type: "password" },
    ]
  },
  {
    id: "firebase",
    name: "Firebase Auth",
    category: "Auth",
    description: "Google's identity platform for apps.",
    fields: [
      { key: "apiKey", label: "API Key", type: "text" },
      { key: "authDomain", label: "Auth Domain", type: "text" },
      { key: "projectId", label: "Project ID", type: "text" },
    ]
  },

  // --- COMMUNICATION ---
  {
    id: "sendgrid",
    name: "SendGrid",
    category: "Communication",
    description: "Transactional email delivery at scale.",
    fields: [
      { key: "apiKey", label: "API Key", type: "password", placeholder: "SG...", validate: v => !v.startsWith('SG.') ? 'Must start with SG.' : null },
      { key: "fromEmail", label: "Sender Identity", type: "text" },
    ]
  },
  {
    id: "resend",
    name: "Resend",
    category: "Communication",
    description: "Modern email API for developers.",
    fields: [
      { key: "apiKey", label: "API Key", type: "password", placeholder: "re_..." },
      { key: "domain", label: "Sending Domain", type: "text" },
    ]
  },
  {
    id: "twilio",
    name: "Twilio",
    category: "Communication",
    description: "SMS, WhatsApp, and Voice APIs.",
    fields: [
      { key: "accountSid", label: "Account SID", type: "text", placeholder: "AC..." },
      { key: "authToken", label: "Auth Token", type: "password" },
      { key: "messagingServiceSid", label: "Messaging Service SID", type: "text", placeholder: "MG..." },
    ]
  },
  {
    id: "xmtp",
    name: "XMTP",
    category: "Communication",
    description: "Secure web3 messaging protocol.",
    fields: [
      { key: "privateKey", label: "Client Private Key", type: "password", hint: "Used to sign system messages" },
      { key: "env", label: "Environment", type: "text", placeholder: "production" },
    ]
  },
  {
    id: "push",
    name: "Push Protocol",
    category: "Communication",
    description: "Cross-chain notifications and messaging.",
    fields: [
      { key: "channelAddress", label: "Channel Address", type: "text", placeholder: "0x..." },
      { key: "privateKey", label: "Channel Private Key", type: "password" },
    ]
  },

  // --- STORAGE ---
  {
    id: "s3",
    name: "AWS S3",
    category: "Storage",
    description: "Standard object storage.",
    fields: [
      { key: "bucket", label: "Bucket Name", type: "text" },
      { key: "region", label: "Region", type: "text", placeholder: "us-east-1" },
      { key: "accessKey", label: "Access Key ID", type: "text" },
      { key: "secretKey", label: "Secret Access Key", type: "password" },
    ]
  },
  {
    id: "pinata",
    name: "Pinata",
    category: "Storage",
    description: "IPFS pinning service for NFTs.",
    fields: [
      { key: "jwt", label: "JWT Token", type: "password" },
      { key: "gateway", label: "Gateway URL", type: "text", placeholder: "https://gateway.pinata.cloud" },
    ]
  },
  {
    id: "arweave",
    name: "Irys (Arweave)",
    category: "Storage",
    description: "Permanent decentralized data storage.",
    fields: [
      { key: "rpcUrl", label: "RPC URL", type: "text" },
      { key: "privateKey", label: "Wallet Private Key", type: "password", hint: "Used to pay for storage uploads" },
    ]
  },
  {
    id: "filecoin",
    name: "Lighthouse (Filecoin)",
    category: "Storage",
    description: "Perpetual file storage on Filecoin.",
    fields: [
      { key: "apiKey", label: "API Key", type: "password" },
    ]
  },
  {
    id: "supabase_storage",
    name: "Supabase Storage",
    category: "Storage",
    description: "Storage integrated with Postgres RLS.",
    fields: [
      { key: "projectUrl", label: "Project URL", type: "text" },
      { key: "serviceKey", label: "Service Role Key", type: "password" },
      { key: "bucket", label: "Bucket", type: "text" },
    ]
  },

  // --- DATA / ORACLE ---
  {
    id: "chainlink",
    name: "Chainlink",
    category: "Data",
    description: "Decentralized oracle networks.",
    fields: [
      { key: "subId", label: "VRF Subscription ID", type: "text" },
      { key: "rpcUrl", label: "Chain RPC", type: "text" },
    ]
  },
  {
    id: "graph",
    name: "The Graph",
    category: "Data",
    description: "Index blockchain data with Subgraphs.",
    fields: [
      { key: "apiKey", label: "Query API Key", type: "password" },
      { key: "subgraphId", label: "Subgraph ID", type: "text" },
    ]
  },
  {
    id: "alchemy",
    name: "Alchemy",
    category: "Data",
    description: "Web3 development platform and nodes.",
    fields: [
      { key: "apiKey", label: "API Key", type: "password" },
      { key: "appId", label: "App ID", type: "text" },
    ]
  },
  {
    id: "moralis",
    name: "Moralis",
    category: "Data",
    description: "Enterprise-grade Web3 APIs.",
    fields: [
      { key: "apiKey", label: "Web3 API Key", type: "password" },
    ]
  },
  {
    id: "pyth",
    name: "Pyth Network",
    category: "Data",
    description: "Real-time market data oracle.",
    fields: [
      { key: "hermesUrl", label: "Hermes Endpoint", type: "text", placeholder: "https://hermes.pyth.network" },
    ]
  },

  // --- ANALYTICS ---
  {
    id: "posthog",
    name: "PostHog",
    category: "Analytics",
    description: "Product analytics and feature flags.",
    fields: [
      { key: "apiKey", label: "Project API Key", type: "text", placeholder: "phc_..." },
      { key: "host", label: "Instance Host", type: "text", placeholder: "https://app.posthog.com" },
    ]
  },
  {
    id: "segment",
    name: "Segment",
    category: "Analytics",
    description: "Customer data platform (CDP).",
    fields: [
      { key: "writeKey", label: "Write Key", type: "password" },
    ]
  },
  {
    id: "dune",
    name: "Dune API",
    category: "Analytics",
    description: "Blockchain data analytics.",
    fields: [
      { key: "apiKey", label: "API Key", type: "password" },
    ]
  },
  {
    id: "ga4",
    name: "Google Analytics 4",
    category: "Analytics",
    description: "Web traffic measurement.",
    fields: [
      { key: "measurementId", label: "Measurement ID", type: "text", placeholder: "G-..." },
      { key: "apiSecret", label: "API Secret", type: "password" },
    ]
  },
  {
    id: "mixpanel",
    name: "Mixpanel",
    category: "Analytics",
    description: "Event analytics for mobile & web.",
    fields: [
      { key: "projectToken", label: "Project Token", type: "text" },
    ]
  },

  // --- WEBHOOKS (Newly populated) ---
  {
    id: "slack_webhook",
    name: "Slack",
    category: "Webhooks",
    description: "Post messages to a Slack channel.",
    fields: [
      { key: "webhookUrl", label: "Webhook URL", type: "password", placeholder: "https://hooks.slack.com/services/...", validate: v => !v.startsWith('https://hooks.slack.com') ? 'Invalid Slack URL' : null },
    ]
  },
  {
    id: "discord_webhook",
    name: "Discord",
    category: "Webhooks",
    description: "Send alerts to a Discord channel.",
    fields: [
      { key: "webhookUrl", label: "Webhook URL", type: "password", placeholder: "https://discord.com/api/webhooks/...", validate: v => !v.includes('discord.com/api') ? 'Invalid Discord URL' : null },
    ]
  },
  {
    id: "telegram_bot",
    name: "Telegram Bot",
    category: "Webhooks",
    description: "Send messages via a Telegram Bot.",
    fields: [
      { key: "botToken", label: "Bot Token", type: "password" },
      { key: "chatId", label: "Chat ID", type: "text" },
    ]
  },
  {
    id: "zapier",
    name: "Zapier",
    category: "Webhooks",
    description: "Trigger multi-step Zapier workflows.",
    fields: [
      { key: "webhookUrl", label: "Catch Hook URL", type: "password", placeholder: "https://hooks.zapier.com/..." },
    ]
  },
  {
    id: "custom_webhook",
    name: "Custom Endpoint",
    category: "Webhooks",
    description: "Send JSON payloads to your own server.",
    fields: [
      { key: "url", label: "Endpoint URL", type: "text", placeholder: "https://api.myapp.com/events" },
      { key: "secret", label: "Signing Secret", type: "password", hint: "Used to sign X-Hub-Signature header" },
      { key: "headers", label: "Custom Headers", type: "text", placeholder: '{"Authorization": "Bearer..."}' },
    ]
  },
];

/* ------------------ Styled Components ------------------ */

const FloatingIsland = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? 'rgba(255, 255, 255, 0.95)' : 'rgba(20, 20, 23, 0.95)',
  backdropFilter: 'blur(16px)',
  border: `1px solid ${theme.palette.divider}`,
  boxShadow: '0 20px 40px -8px rgba(0, 0, 0, 0.3)',
  borderRadius: 100,
  padding: '8px 24px',
  display: 'flex',
  alignItems: 'center',
  gap: 16,
  zIndex: 1000,
  pointerEvents: 'auto',
}));

const StepPill = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? 'rgba(255, 255, 255, 0.9)' : 'rgba(20, 20, 23, 0.9)',
  backdropFilter: 'blur(10px)',
  border: `1px solid ${theme.palette.divider}`,
  borderRadius: 100,
  padding: '8px 20px',
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  boxShadow: '0 4px 12px rgba(0,0,0,0.1)',
  pointerEvents: 'auto',
}));

const CategoryRail = styled(Box)(({ theme }) => ({
  width: 260,
  height: '100%',
  borderRight: `1px solid ${theme.palette.divider}`,
  display: 'flex',
  flexDirection: 'column',
  background: theme.palette.mode === 'light' ? 'rgba(255,255,255,0.5)' : 'rgba(10,10,12,0.5)',
  backdropFilter: 'blur(20px)',
  paddingTop: 80, 
}));

const CategoryItem = styled(Box, { shouldForwardProp: (p) => p !== 'active' })<{ active?: boolean }>(({ theme, active }) => ({
  padding: '12px 24px',
  cursor: 'pointer',
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  borderLeft: `3px solid ${active ? theme.palette.primary.main : 'transparent'}`,
  background: active ? alpha(theme.palette.primary.main, 0.05) : 'transparent',
  color: active ? theme.palette.primary.main : theme.palette.text.secondary,
  transition: 'all 0.2s ease',
  fontWeight: active ? 700 : 500,
  '&:hover': {
    background: active ? alpha(theme.palette.primary.main, 0.08) : alpha(theme.palette.action.hover, 0.5),
    color: active ? theme.palette.primary.main : theme.palette.text.primary,
  }
}));

const ProviderList = styled(Box)(({ theme }) => ({
  width: 320,
  height: '100%',
  borderRight: `1px solid ${theme.palette.divider}`,
  display: 'flex',
  flexDirection: 'column',
  background: theme.palette.mode === 'light' ? '#fff' : '#0a0a0c',
  paddingTop: 80,
  overflowY: 'auto',
}));

const ProviderItem = styled(Box, { shouldForwardProp: (p) => p !== 'active' })<{ active?: boolean }>(({ theme, active }) => ({
  padding: '20px 24px',
  cursor: 'pointer',
  borderBottom: `1px solid ${theme.palette.divider}`,
  background: active ? alpha(theme.palette.primary.main, 0.04) : 'transparent',
  transition: 'all 0.2s',
  '&:hover': {
    background: active ? alpha(theme.palette.primary.main, 0.06) : alpha(theme.palette.action.hover, 0.5),
  }
}));

const ConfigPanel = styled(Box)(({ theme }) => ({
  flex: 1,
  height: '100%',
  paddingTop: 80,
  paddingBottom: 100,
  overflowY: 'auto',
  background: theme.palette.mode === 'light' ? '#f8fafc' : '#121214',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
}));

/* ------------------ Components ------------------ */

function getIcon(cat: IntegrationCategory) {
  switch (cat) {
    case 'Payments': return <PaymentsIcon fontSize="small" />;
    case 'Auth': return <SecurityIcon fontSize="small" />;
    case 'Communication': return <EmailIcon fontSize="small" />;
    case 'Storage': return <StorageIcon fontSize="small" />;
    case 'Data': return <HubIcon fontSize="small" />;
    case 'Analytics': return <AnalyticsIcon fontSize="small" />;
    case 'Webhooks': return <WebhookIcon fontSize="small" />;
    default: return <HubIcon fontSize="small" />;
  }
}

async function apiGetProject(projectId: string) {
  const res = await fetch(`/api/projects/${encodeURIComponent(projectId)}`, { method: "GET" });
  if (!res.ok) throw new Error("Load failed");
  return await res.json();
}

async function apiPatchIntegrations(projectId: string, configs: any) {
  await fetch(`/api/projects/${encodeURIComponent(projectId)}/integrations`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ configs }),
  });
}

// Extracted to safe hook usage
function IntegrationFieldRow({ 
  field, 
  value, 
  onChange, 
  error 
}: { 
  field: IntegrationDef['fields'][0];
  value: string;
  onChange: (val: string) => void;
  error: string | null;
}) {
  const [show, setShow] = useState(false);

  return (
    <Box>
       <Stack direction="row" justifyContent="space-between" mb={0.5}>
          <Typography variant="subtitle2" fontWeight={700}>{field.label}</Typography>
          <Link component="button" variant="caption" underline="hover">Get {field.label}</Link>
       </Stack>
       <TextField 
          fullWidth size="small" 
          placeholder={field.placeholder}
          type={field.type === 'text' || show ? 'text' : 'password'}
          value={value}
          onChange={e => onChange(e.target.value)}
          error={!!error}
          helperText={error || field.hint}
          InputProps={{
             endAdornment: field.type === 'password' && (
                <InputAdornment position="end">
                   <IconButton size="small" onClick={() => setShow(!show)}>
                      {show ? <VisibilityOff fontSize="small" /> : <Visibility fontSize="small" />}
                   </IconButton>
                </InputAdornment>
             )
          }}
          sx={{ "& .MuiInputBase-root": { borderRadius: 2 } }}
       />
    </Box>
  );
}

/* ====================================================================== */
export default function Step4({ goPrev, goNext, projectId }: { goPrev?: () => void; goNext?: () => void; projectId: string | null }) {
  const theme = useTheme();
  const router = useRouter();
  const sp = useSearchParams();

  const [resolvedId, setResolvedId] = useState<string | null>(projectId);
  useEffect(() => {
    if (!resolvedId) {
       const ls = typeof window !== 'undefined' ? localStorage.getItem('cerulea.activeProjectId') : null;
       setResolvedId(sp?.get('projectId') || ls);
    }
  }, [projectId, sp, resolvedId]);

  const [activeCat, setActiveCat] = useState<IntegrationCategory>("Payments");
  const [selectedId, setSelectedId] = useState<string>("stripe");
  const [configs, setConfigs] = useState<Record<string, IntegrationConfig>>({});
  const [isHelpOpen, setIsHelpOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<'success'|'error'|null>(null);

  const providers = useMemo(() => CATALOG.filter(c => c.category === activeCat), [activeCat]);
  
  // Safe Default Initialization
  const activeConfig = useMemo(() => {
    return configs[selectedId] || { ...DEFAULT_CONFIG };
  }, [configs, selectedId]);
  
  const activeDef = useMemo(() => CATALOG.find(c => c.id === selectedId), [selectedId]);

  useEffect(() => {
    (async () => {
      if (!resolvedId) { setLoading(false); return; }
      try {
        const p = await apiGetProject(resolvedId);
        if (p.integrations?.configs) setConfigs(p.integrations.configs);
      } catch (e) { console.warn(e); }
      finally { setLoading(false); }
    })();
  }, [resolvedId]);

  const updateConfig = (patch: Partial<IntegrationConfig>) => {
    setConfigs(prev => {
      const existing = prev[selectedId] || DEFAULT_CONFIG;
      return {
        ...prev,
        [selectedId]: { ...existing, ...patch }
      };
    });
  };

  const updateCred = (key: string, val: string) => {
    setConfigs(prev => {
      const existing = prev[selectedId] || DEFAULT_CONFIG;
      const env = existing.environment || 'dev';
      const envKey = `${env}_${key}`;
      return {
        ...prev,
        [selectedId]: {
          ...existing,
          credentials: { ...(existing.credentials || {}), [envKey]: val }
        }
      };
    });
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    setTimeout(() => {
      const isValid = activeDef?.fields.every(f => {
        const envKey = `${activeConfig.environment || 'dev'}_${f.key}`;
        const val = activeConfig.credentials?.[envKey];
        return val && (!f.validate || !f.validate(val));
      });
      setTestResult(isValid ? 'success' : 'error');
      setTesting(false);
    }, 1200);
  };

  const handleSave = async () => {
    if (resolvedId) await apiPatchIntegrations(resolvedId, configs);
    if (goNext) goNext();
  };

  return (
    <Box sx={{ width: '100%', position: 'fixed', inset: 0, top: 64, bgcolor: 'background.default', display: 'flex', flexDirection: 'column' }}>
      <StepGuidance
        stepKey="step4"
        title="Integrations"
        subtitle="STEP 5 OF 7"
        description="Connect your application to external services: payment processors, KYC providers, storage, oracles, and more. Each integration adds a pre-built connector to your deployment."
        steps={[
          { first: 'Review available integrations', next: 'Browse the list of supported services like Stripe, Sumsub KYC, Chainlink Oracle, and IPFS.' },
          { first: 'Enable integrations you need', next: 'Toggle on each service and paste the required API keys or credentials.' },
          { first: 'Configure webhooks', next: 'Set a webhook endpoint URL if you need to receive real-time events from the integration.' },
        ]}
        tip="Only enable integrations your app actually uses. Each enabled integration adds to your deployment footprint and monthly costs."
      />

       <Box sx={{ position: 'absolute', inset: 0, opacity: 0.3, zIndex: -1,
          backgroundImage: theme.palette.mode === 'light' ? 'radial-gradient(#ccc 1px, transparent 1px)' : 'radial-gradient(#333 1px, transparent 1px)',
          backgroundSize: '24px 24px'
       }} />

       <Box sx={{ position: 'absolute', top: 24, left: '50%', transform: 'translateX(-50%)', zIndex: 10, pointerEvents: 'none' }}>
          <StepPill>
             <Typography variant="overline" fontWeight={800} color="primary" sx={{ letterSpacing: 1, lineHeight: 1 }}>STEP 5 OF 6</Typography>
             <Divider orientation="vertical" flexItem sx={{ height: 14, my: 'auto', opacity: 0.5 }} />
             <Typography variant="subtitle2" fontWeight={700}>Integrations</Typography>
          </StepPill>
       </Box>

       <Box sx={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
          
          {/* 1. Category Rail */}
          <CategoryRail>
             <Box sx={{ px: 3, pb: 2 }}>
               <Typography variant="overline" fontWeight={800} color="text.disabled" fontSize="0.6rem">STEP 5 OF 6</Typography>
               <Typography variant="subtitle1" fontWeight={800}>Integrations</Typography>
               <Typography variant="caption" color="text.secondary">
                 Connect your app to third-party services. Toggle each integration on and add your API keys. Credentials are stored encrypted and never shared.
               </Typography>
             </Box>
             <Stack spacing={0.5} sx={{ px: 2 }}>
                {(["Payments", "Auth", "Communication", "Storage", "Data", "Analytics", "Webhooks"] as IntegrationCategory[]).map(cat => (
                   <CategoryItem key={cat} active={activeCat === cat} onClick={() => { setActiveCat(cat); const first = CATALOG.find(c => c.category === cat); if (first) setSelectedId(first.id); }}>
                      {getIcon(cat)}
                      <Typography variant="subtitle2" fontWeight={700}>{cat}</Typography>
                   </CategoryItem>
                ))}
             </Stack>
          </CategoryRail>

          {/* 2. Provider List */}
          <ProviderList>
             <Box sx={{ p: 3, pb: 1 }}><Typography variant="overline" fontWeight={800} color="text.secondary">PROVIDERS</Typography></Box>
             {providers.map(p => {
               const isEnabled = configs[p.id]?.enabled;
               return (
                 <ProviderItem key={p.id} active={selectedId === p.id} onClick={() => setSelectedId(p.id)}>
                    <Stack direction="row" justifyContent="space-between" alignItems="center">
                       <Typography variant="body2" fontWeight={700}>{p.name}</Typography>
                       {isEnabled && <CheckCircleIcon fontSize="small" color="primary" sx={{ width: 16 }} />}
                    </Stack>
                    <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 0.5, lineHeight: 1.4 }}>
                       {p.description}
                    </Typography>
                 </ProviderItem>
               );
             })}
          </ProviderList>

          {/* 3. Config Panel */}
          <ConfigPanel>
             {activeDef && (
               <Fade in key={activeDef.id}>
                 <Box sx={{ width: '100%', maxWidth: 700, p: 4 }}>
                    <Stack direction="row" alignItems="center" justifyContent="space-between" mb={4}>
                       <Stack direction="row" alignItems="center" spacing={2}>
                          <Typography variant="h4" fontWeight={900}>{activeDef.name}</Typography>
                          {activeConfig.enabled && <Chip label="Active" color="success" size="small" />}
                       </Stack>
                       <FormControlLabel 
                          control={<Switch checked={activeConfig.enabled} onChange={e => updateConfig({ enabled: e.target.checked })} />} 
                          label={<Typography fontWeight={700}>Enable</Typography>} 
                       />
                    </Stack>

                    <Paper variant="outlined" sx={{ p: 4, borderRadius: 4, opacity: activeConfig.enabled ? 1 : 0.6, pointerEvents: activeConfig.enabled ? 'auto' : 'none', transition: 'opacity 0.2s' }}>
                       {/* Header Controls */}
                       <Stack direction="row" justifyContent="space-between" alignItems="center" mb={4}>
                          <Stack direction="row" spacing={1} bgcolor={alpha(theme.palette.action.hover, 0.5)} p={0.5} borderRadius={2}>
                             <Button 
                                size="small" variant={activeConfig.environment === 'dev' ? 'contained' : 'text'} 
                                onClick={() => updateConfig({ environment: 'dev' })}
                                sx={{ borderRadius: 1.5, px: 2 }}
                             >
                                Test Mode
                             </Button>
                             <Button 
                                size="small" variant={activeConfig.environment === 'prod' ? 'contained' : 'text'} 
                                onClick={() => updateConfig({ environment: 'prod' })}
                                color="secondary"
                                sx={{ borderRadius: 1.5, px: 2 }}
                             >
                                Live Mode
                             </Button>
                          </Stack>
                          {activeDef.docsUrl && (
                             <Link href={activeDef.docsUrl} target="_blank" underline="hover" sx={{ display: 'flex', alignItems: 'center', gap: 0.5, fontSize: 13, fontWeight: 600 }}>
                                Documentation <OpenInNewIcon sx={{ fontSize: 14 }} />
                             </Link>
                          )}
                       </Stack>

                       <Divider sx={{ mb: 4 }} />

                       {/* Fields */}
                       <Stack spacing={3}>
                          {activeDef.fields.map(field => {
                             // Fix: Access safely with Optional Chaining/Nullish Coalescing
                             const envKey = `${activeConfig.environment || 'dev'}_${field.key}`;
                             const val = activeConfig.credentials?.[envKey] ?? '';
                             const error = val && field.validate ? field.validate(val) : null;
                             
                             return (
                                <IntegrationFieldRow 
                                  key={field.key}
                                  field={field}
                                  value={val}
                                  onChange={(v) => updateCred(field.key, v)}
                                  error={error}
                                />
                             );
                          })}
                       </Stack>

                       {/* Test Action */}
                       <Box sx={{ mt: 4, pt: 3, borderTop: `1px solid ${theme.palette.divider}`, display: 'flex', alignItems: 'center', gap: 2 }}>
                          <Button 
                             variant="outlined" startIcon={testing ? <CircularProgress size={16} /> : <PlayArrowIcon />}
                             onClick={handleTest}
                             disabled={testing}
                             sx={{ borderRadius: 999, px: 3, fontWeight: 700 }}
                          >
                             Test Connection
                          </Button>
                          
                          {testResult === 'success' && (
                             <Fade in><Chip icon={<CheckCircleIcon />} label="Connected Successfully" color="success" variant="outlined" /></Fade>
                          )}
                          {testResult === 'error' && (
                             <Fade in><Chip icon={<CloseIcon />} label="Connection Failed" color="error" variant="outlined" /></Fade>
                          )}
                       </Box>
                    </Paper>
                 </Box>
               </Fade>
             )}
          </ConfigPanel>

       </Box>

       {/* Dock */}
       <Box sx={{ position: 'absolute', bottom: 32, left: '50%', transform: 'translateX(-50%)', zIndex: 100 }}>
          <FloatingIsland elevation={6}>
             <Tooltip title="Back">
               <IconButton onClick={goPrev ? goPrev : () => router.back()} size="small" sx={{border: '1px solid', borderColor:'divider'}}>
                  <ArrowBackIcon />
               </IconButton>
             </Tooltip>
             <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />
             <Tooltip title="Help">
                <IconButton size="small" color="primary" onClick={() => setIsHelpOpen(true)}><QuestionMarkIcon fontSize="small" /></IconButton>
             </Tooltip>
             <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />
             <Button variant="contained" onClick={handleSave} endIcon={<ArrowForwardIcon />} sx={{ borderRadius: 100, px: 3, fontWeight: 700 }}>
                Complete
             </Button>
          </FloatingIsland>
       </Box>

       {/* Help Dialog */}
       <Dialog open={isHelpOpen} onClose={() => setIsHelpOpen(false)} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: 4, bgcolor: 'background.paper', color: 'text.primary' } }}>
          <DialogTitle sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', p: 3 }}>
             <Typography variant="h6" fontWeight={800}>Integrations Guide</Typography>
             <IconButton onClick={() => setIsHelpOpen(false)}><CloseIcon /></IconButton>
          </DialogTitle>
          <DialogContent dividers sx={{ p: 4 }}>
             <Typography variant="body1" paragraph>
                Connect your application to external services.
             </Typography>
             <Typography variant="body2" color="text.secondary">
                • <b>Dev vs Prod:</b> Use 'Test Mode' keys for local development. Switch to 'Live Mode' before deploying.<br/>
                • <b>Webhooks:</b> Ensure your webhook secret matches what is in your provider dashboard to verify signatures.<br/>
                • <b>Validation:</b> Use the 'Test Connection' button to verify your API keys are active and have correct permissions.
             </Typography>
          </DialogContent>
       </Dialog>

    </Box>
  );
}