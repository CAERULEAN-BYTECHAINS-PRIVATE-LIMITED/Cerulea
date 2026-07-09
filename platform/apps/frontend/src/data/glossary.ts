// Centralised glossary for Studio step tooltips and help text.
// Used throughout all steps for consistent, user-friendly explanations.

export interface GlossaryEntry {
  term: string;
  definition: string;
  example?: string;
  learnMore?: string;
}

export const GLOSSARY: Record<string, GlossaryEntry> = {
  // ── Project types ─────────────────────────────────────────────────────────
  dapp: {
    term: 'Decentralised Application (dApp)',
    definition: 'An application that runs on a blockchain network rather than a traditional centralised server. Your users own their data and assets.',
    example: 'An NFT marketplace, a DeFi lending platform, or a DAO voting app.',
  },
  blockchain: {
    term: 'Private / Sovereign Blockchain',
    definition: 'A fully owned blockchain network that you control. Used by enterprises and governments for compliance, privacy, and high-throughput operations.',
    example: 'A supply chain ledger for a manufacturer, or a central bank digital currency network.',
  },

  // ── Step 0 Project Foundation ───────────────────────────────────────────
  template: {
    term: 'Template',
    definition: 'A pre-built starting point that comes with a curated set of modules and connections. Choosing a template is the fastest way to get started; you can always add or remove modules later.',
  },
  consensus: {
    term: 'Consensus Mechanism',
    definition: 'The algorithm your blockchain uses to agree on new transactions and blocks.',
    example: 'PoA (Proof of Authority) is fast and controlled, good for private chains. PoS (Proof of Stake) is decentralised, good for public networks.',
  },
  region: {
    term: 'Deployment Region',
    definition: 'The geographic area where your blockchain nodes will be hosted. Choose the region closest to your users for lowest latency.',
  },
  nativeToken: {
    term: 'Native Token',
    definition: 'The built-in currency of your blockchain, used to pay transaction fees (gas). Every Cerulea blockchain has one.',
    example: 'ETH is the native token of Ethereum. You can name yours anything (e.g. "MYTOKEN").',
  },

  // ── Step 1 Blueprint Builder ────────────────────────────────────────────
  module: {
    term: 'Module',
    definition: 'A self-contained building block that adds a specific feature to your application , such as wallets, tokens, governance voting, or payments.',
    example: 'The "ERC-20" module adds a fungible token. The "RBAC" module adds role-based access control.',
  },
  blueprint: {
    term: 'Blueprint',
    definition: 'A visual map of all the modules in your application and how they connect to each other. Think of it as your app\'s architecture diagram.',
  },
  edge: {
    term: 'Connection (Edge)',
    definition: 'A line between two modules showing how they interact. Define what type of interaction it is (e.g. READS, WRITES, TRIGGERS).',
  },

  // ── Step 2 Data & Logic ─────────────────────────────────────────────────
  entity: {
    term: 'Entity',
    definition: 'A data model representing a core object in your application , like a User, a Token, or a Transaction. Think of it as a database table.',
    example: 'A "User" entity might have fields: id, email, wallet_address, role.',
  },
  field: {
    term: 'Field',
    definition: 'A single piece of data stored on an entity. Every field has a name, a type (text, number, etc.), and storage location (database vs. blockchain).',
    example: 'The "email" field on a User entity is of type "string" and stored in the database.',
  },
  storageOnChain: {
    term: 'On-Chain Storage',
    definition: 'Data stored permanently on the blockchain. It is public, immutable (cannot be changed), and costs gas fees to write. Use it only for data that must be auditable.',
    example: 'Token balances, ownership records, governance votes.',
  },
  storageDatabase: {
    term: 'Database Storage',
    definition: 'Data stored in a regular (off-chain) database. It is fast, cheap, and private. Use it for most application data.',
    example: 'User profiles, email addresses, session tokens.',
  },
  storageIPFS: {
    term: 'IPFS Storage',
    definition: 'Data stored on IPFS , a decentralised file system. Good for large files like NFT metadata, images, or documents that should not be on-chain but must be permanently accessible.',
  },
  relationship: {
    term: 'Relationship',
    definition: 'A link between two entities. For example, a "User" has many "Orders". Relationships define the structure of your data.',
  },
  accessControl: {
    term: 'Access Control Rule',
    definition: 'A rule that defines who can read or write a piece of data. For example: "Only admins can delete users".',
  },
  logicFlow: {
    term: 'Logic Flow',
    definition: 'A visual representation of business logic , like "when a user places an order, notify the seller". It shows what happens step by step when an event occurs.',
  },

  // ── Step 3 Economics ────────────────────────────────────────────────────
  gasPolicy: {
    term: 'Gas Policy',
    definition: 'Rules for how transaction fees (gas) are calculated and distributed on your blockchain. A higher base fee prevents spam.',
  },
  staking: {
    term: 'Staking',
    definition: 'Users lock up (stake) tokens to participate in network validation or governance. In return they earn rewards. Unstaking has a time delay (unbonding period).',
  },
  slashing: {
    term: 'Slashing',
    definition: 'A penalty where a validator loses part of their staked tokens if they act maliciously or go offline. This keeps validators honest.',
  },
  quorum: {
    term: 'Quorum',
    definition: 'The minimum percentage of votes required for a governance proposal to pass. For example: 10% quorum means at least 10% of all voters must participate.',
  },
  inflation: {
    term: 'Inflation Rate',
    definition: 'The rate at which new tokens are created and distributed to validators and stakers as rewards. Higher inflation = more rewards but dilutes existing holders.',
  },

  // ── Step 4 Integrations ─────────────────────────────────────────────────
  webhook: {
    term: 'Webhook',
    definition: 'An automated message sent from your app to an external service when something happens. For example, send a Slack message when a new user registers.',
  },
  oracle: {
    term: 'Oracle',
    definition: 'A service that brings real-world data onto your blockchain. For example, a price oracle tells your smart contract the current price of ETH/USD.',
  },

  // ── Step 6 Deploy ───────────────────────────────────────────────────────
  deployment: {
    term: 'Deployment',
    definition: 'The process of publishing your application to the Cerulea network. This compiles your smart contracts, provisions infrastructure, and makes your app live.',
  },
  rpc: {
    term: 'RPC Endpoint',
    definition: 'A URL that external wallets and apps use to interact with your blockchain. Think of it as your blockchain\'s public API.',
  },
};

// Helper: look up a term and return its definition
export function glossaryLookup(key: string): string {
  return GLOSSARY[key]?.definition ?? '';
}

// Helper: get tooltip title (term + definition)
export function glossaryTooltip(key: string): string {
  const entry = GLOSSARY[key];
  if (!entry) return '';
  return `${entry.term}: ${entry.definition}`;
}
