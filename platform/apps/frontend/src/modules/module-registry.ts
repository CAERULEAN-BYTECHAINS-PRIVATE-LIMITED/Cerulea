import { SvgIconComponent } from '@mui/icons-material';
// Import icons for a richer library
import FingerprintIcon from '@mui/icons-material/Fingerprint';
import PublicIcon from '@mui/icons-material/Public';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import GroupIcon from '@mui/icons-material/Group';
import ShieldOutlinedIcon from '@mui/icons-material/ShieldOutlined';
import StorageIcon from '@mui/icons-material/Storage';
import CloudUploadIcon from '@mui/icons-material/CloudUpload';
import HubIcon from '@mui/icons-material/Hub';
import LinkIcon from '@mui/icons-material/Link';
import HowToVoteIcon from '@mui/icons-material/HowToVote';
import AccountBalanceIcon from '@mui/icons-material/AccountBalance';
import ReceiptLongIcon from '@mui/icons-material/ReceiptLong';
import ImageIcon from '@mui/icons-material/Image';
import MonetizationOnIcon from '@mui/icons-material/MonetizationOn';
import CreditCardIcon from '@mui/icons-material/CreditCard';
import SubscriptionsIcon from '@mui/icons-material/Subscriptions';
import SavingsIcon from '@mui/icons-material/Savings';
import SwapHorizIcon from '@mui/icons-material/SwapHoriz';
import NotificationsIcon from '@mui/icons-material/Notifications';
import EmailIcon from '@mui/icons-material/Email';
import DynamicFeedIcon from '@mui/icons-material/DynamicFeed';
import TokenIcon from '@mui/icons-material/Token';
import ViewInArIcon from '@mui/icons-material/ViewInAr';
import Looks3Icon from '@mui/icons-material/Looks3';
import AccountTreeIcon from '@mui/icons-material/AccountTree';

// Define the structure for a module
export interface Module {
  id: string;
  name: string;
  category: string;
  description: string;
  icon: SvgIconComponent;
  dependencies?: string[]; // List of module IDs it depends on
}

// The full, detailed list of 24 MVP modules
export const MODULE_LIBRARY: Module[] = [
  // Core Token Standards
  { id: 'erc20', name: 'Fungible Token (ERC-20)', category: 'Tokens', description: 'Create an interchangeable utility or governance token.', icon: TokenIcon },
  { id: 'erc721', name: 'NFT (ERC-721)', category: 'Tokens', description: 'Mint unique, non-fungible digital assets and collectibles.', icon: ImageIcon, dependencies: ['file-storage'] },
  { id: 'erc1155', name: 'Multi-Token (ERC-1155)', category: 'Tokens', description: 'A single contract for multiple fungible and non-fungible token types.', icon: Looks3Icon, dependencies: ['file-storage'] },
  { id: 'erc4626', name: 'Tokenized Vault (ERC-4626)', category: 'Tokens', description: 'Standard for yield-bearing vaults, common in DeFi.', icon: SavingsIcon, dependencies: ['erc20'] },
  // User Management
  { id: 'wallet-auth', name: 'Wallet Authentication', category: 'Users', description: 'Allow users to sign in with their crypto wallet (e.g., MetaMask).', icon: AccountBalanceWalletIcon },
  { id: 'social-logins', name: 'Web2 Social Logins', category: 'Users', description: 'Onboard users with familiar Google, Twitter, or email accounts.', icon: PublicIcon },
  { id: 'user-profiles', name: 'User Profiles', category: 'Users', description: 'Manage user data, both on-chain and off-chain.', icon: GroupIcon, dependencies: ['wallet-auth'] },
  { id: 'rbac', name: 'Role-Based Access Control', category: 'Users', description: 'Define roles (e.g., Admin, Moderator) with specific permissions.', icon: ShieldOutlinedIcon, dependencies: ['user-profiles'] },
  // Data & Logic
  { id: 'on-chain-storage', name: 'On-Chain Data Structures', category: 'Data', description: 'Define and manage your core data models directly on the blockchain.', icon: StorageIcon },
  { id: 'file-storage', name: 'Decentralized File Storage', category: 'Data', description: 'Store large files and metadata on IPFS or Arweave.', icon: CloudUploadIcon },
  { id: 'logic-editor', name: 'Logic & State Machine Editor', category: 'Data', description: 'Visually define the core logic and state transitions of your application.', icon: AccountTreeIcon },
  { id: 'oracle', name: 'External API Oracle', category: 'Data', description: 'Connect your application to real-world data via Chainlink.', icon: LinkIcon },
  // Governance & DAOs
  { id: 'voting', name: 'On-Chain Voting', category: 'Governance', description: 'Enable token holders to create and vote on proposals.', icon: HowToVoteIcon, dependencies: ['erc20'] },
  { id: 'treasury', name: 'Treasury Management', category: 'Governance', description: 'A secure, multi-signature vault for managing project funds.', icon: AccountBalanceIcon },
  { id: 'proposal-system', name: 'Proposal System', category: 'Governance', description: 'A formal system for community members to submit and discuss proposals.', icon: ReceiptLongIcon, dependencies: ['voting'] },
  // Monetization & DeFi
  { id: 'minting-engine', name: 'NFT Minting Engine', category: 'Monetization', description: 'Create customizable minting pages and drop mechanics for your NFTs.', icon: ViewInArIcon, dependencies: ['erc721'] },
  { id: 'royalty-standard', name: 'Royalty Standard (EIP-2981)', category: 'Monetization', description: 'Set and enforce creator royalties for secondary NFT sales.', icon: MonetizationOnIcon, dependencies: ['erc721'] },
  { id: 'fiat-on-ramp', name: 'Fiat On-Ramp', category: 'Monetization', description: 'Allow users to purchase crypto or NFTs with a credit card via Stripe.', icon: CreditCardIcon },
  { id: 'on-chain-subscriptions', name: 'On-Chain Subscriptions', category: 'Monetization', description: 'Create recurring payment and subscription models for your services.', icon: SubscriptionsIcon },
  { id: 'token-staking', name: 'Token Staking', category: 'Monetization', description: 'Allow users to lock up their tokens to earn rewards.', icon: SavingsIcon, dependencies: ['erc20'] },
  { id: 'amm-swap', name: 'Simple AMM / Swap', category: 'Monetization', description: 'Create a simple decentralized exchange for your project tokens.', icon: SwapHorizIcon, dependencies: ['erc20'] },
  // User Interaction
  { id: 'in-app-notifications', name: 'In-App Notifications', category: 'Interaction', description: 'A real-time notification center within your application.', icon: NotificationsIcon },
  { id: 'email-notifications', name: 'Transactional Email', category: 'Interaction', description: 'Send automated emails for key events (e.g., Welcome, Purchase).', icon: EmailIcon },
  { id: 'social-feed', name: 'Social Feed / Activity Log', category: 'Interaction', description: 'Display a feed of recent on-chain or off-chain user activity.', icon: DynamicFeedIcon },
];

// --- Dependency Checking Engine ---
export function checkDependencies(selectedModules: Module[]): { missing: { forModule: Module, needs: Module }[], satisfied: boolean } {
  const selectedIds = new Set(selectedModules.map(m => m.id));
  const missing = [];

  for (const module of selectedModules) {
    if (module.dependencies) {
      for (const depId of module.dependencies) {
        if (!selectedIds.has(depId)) {
          missing.push({
            forModule: module,
            needs: MODULE_LIBRARY.find(m => m.id === depId)!,
          });
        }
      }
    }
  }

  return { missing, satisfied: missing.length === 0 };
}