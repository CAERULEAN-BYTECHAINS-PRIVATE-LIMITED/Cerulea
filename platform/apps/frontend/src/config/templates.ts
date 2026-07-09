import WebhookIcon from '@mui/icons-material/Webhook';
import HowToVoteIcon from '@mui/icons-material/HowToVote';
import GavelIcon from '@mui/icons-material/Gavel';
import VpnKeyIcon from '@mui/icons-material/VpnKey';
import CollectionsBookmarkIcon from '@mui/icons-material/CollectionsBookmark';
import EmojiEventsIcon from '@mui/icons-material/EmojiEvents';
import CorporateFareIcon from '@mui/icons-material/CorporateFare';
import AssuredWorkloadIcon from '@mui/icons-material/AssuredWorkload';

export interface AppTemplate {
  id: string;
  title: string;
  description: string;
  icon: React.ElementType;
  preConfiguredModules: string[];
}

export const dAppTemplates: AppTemplate[] = [
  {
    id: 'nft-marketplace',
    title: 'NFT Marketplace',
    description: 'A ready-to-deploy marketplace for digital collectibles.',
    icon: CollectionsBookmarkIcon,
    preConfiguredModules: ['user_authentication_wallet', 'decentralized_file_storage', 'core_data_storage', 'nft_minting'],
  },
  {
    id: 'dao-governance',
    title: 'DAO Governance',
    description: 'A portal for community proposals and on-chain voting.',
    icon: HowToVoteIcon,
    preConfiguredModules: ['user_authentication_wallet', 'decentralized_voting', 'treasury_management', 'role_based_access_control'],
  },
   {
    id: 'token-gated',
    title: 'Token-Gated Community',
    description: 'Exclusive content and features accessible only to token holders.',
    icon: VpnKeyIcon,
    preConfiguredModules: ['user_authentication_wallet', 'token_gating', 'in_app_notifications'],
  },
  {
    id: 'loyalty-program',
    title: 'Loyalty Program',
    description: 'Reward your users with points or tokens for their engagement.',
    icon: EmojiEventsIcon,
    preConfiguredModules: ['user_authentication_traditional', 'token_rewards_loyalty', 'ai_data_analysis'],
  },
];

export const blockchainTemplates: AppTemplate[] = [
  {
    id: 'enterprise-consortium',
    title: 'Enterprise Consortium',
    description: 'A permissioned network for trusted partners to share data securely.',
    icon: CorporateFareIcon,
    preConfiguredModules: ['proof_of_authority', 'role_based_access_control', 'private_data_storage'],
  },
  {
    id: 'document-verification',
    title: 'Document Verification',
    description: 'An immutable ledger for verifying credentials and documents.',
    icon: AssuredWorkloadIcon,
    preConfiguredModules: ['proof_of_authority', 'decentralized_file_storage', 'verifiable_credentials'],
  },
   {
    id: 'governance-chain',
    title: 'Internal Governance',
    description: 'A dedicated chain for internal corporate voting and record-keeping.',
    icon: GavelIcon,
    preConfiguredModules: ['proof_of_stake', 'on_chain_voting_module', 'treasury_management'],
  },
];

