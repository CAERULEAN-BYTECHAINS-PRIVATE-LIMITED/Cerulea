// Comprehensive entity catalog for all modules.
// Each module maps to core entities (pre-installed) and extended entities (add via dropdown).

export type FieldType =
  | 'uuid' | 'string' | 'text' | 'boolean' | 'int' | 'float'
  | 'datetime' | 'json' | 'address' | 'uint256' | 'bytes32' | 'ipfs-hash';

export type StorageStrategy = 'database' | 'on-chain' | 'ipfs';

export interface CatalogField {
  name: string;
  type: FieldType;
  storage: StorageStrategy;
  required?: boolean;
  unique?: boolean;
  indexed?: boolean;
  encrypted?: boolean;
  description?: string;
  defaultValue?: string;
}

export interface CatalogEntity {
  id: string;
  name: string;
  description: string;
  category: string;
  moduleIds: string[];
  fields: CatalogField[];
}

// ─────────────────────────────────────────────────────────────────
// CATEGORIES
// ─────────────────────────────────────────────────────────────────

export const ENTITY_CATEGORIES = [
  'Identity & Auth',
  'Tokens & Assets',
  'DeFi & Finance',
  'Governance',
  'Commerce & Marketplace',
  'Infrastructure',
  'Integrations',
  'Compliance',
  'Legacy & Migration',
  'Chain-specific',
] as const;

export type EntityCategory = (typeof ENTITY_CATEGORIES)[number];

// ─────────────────────────────────────────────────────────────────
// FULL ENTITY CATALOG
// ─────────────────────────────────────────────────────────────────

export const ENTITIES_CATALOG: CatalogEntity[] = [

  // ── Identity & Auth ──────────────────────────────────────────────
  {
    id: 'user',
    name: 'User',
    description: 'Core identity record for an application user.',
    category: 'Identity & Auth',
    moduleIds: ['auth', 'rbac', 'wallet', 'kyc', 'social'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true, indexed: true, description: 'Primary key, auto-generated unique identifier.' },
      { name: 'walletAddress', type: 'address', storage: 'database', unique: true, indexed: true, description: 'Ethereum/EVM wallet address for on-chain actions.' },
      { name: 'email', type: 'string', storage: 'database', unique: true, encrypted: true, description: 'User email, stored encrypted at rest.' },
      { name: 'name', type: 'string', storage: 'database', description: 'Display name.' },
      { name: 'role', type: 'string', storage: 'database', defaultValue: "'user'", description: 'User role for RBAC (e.g. user, admin, moderator).' },
      { name: 'isActive', type: 'boolean', storage: 'database', defaultValue: 'true', description: 'Whether the account is active.' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()', description: 'Account creation timestamp.' },
      { name: 'updatedAt', type: 'datetime', storage: 'database', defaultValue: 'now()', description: 'Last update timestamp.' },
    ],
  },
  {
    id: 'session',
    name: 'Session',
    description: 'Active authentication session for a user.',
    category: 'Identity & Auth',
    moduleIds: ['auth'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'userId', type: 'uuid', storage: 'database', required: true, indexed: true, description: 'FK → User.id' },
      { name: 'token', type: 'string', storage: 'database', required: true, unique: true, encrypted: true },
      { name: 'expiresAt', type: 'datetime', storage: 'database', required: true },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'role',
    name: 'Role',
    description: 'Named role with a set of permissions (RBAC).',
    category: 'Identity & Auth',
    moduleIds: ['rbac'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'name', type: 'string', storage: 'database', required: true, unique: true, description: 'e.g. admin, editor, viewer' },
      { name: 'permissions', type: 'json', storage: 'database', description: 'Array of permission strings.' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'user_role',
    name: 'UserRole',
    description: 'Many-to-many join between User and Role.',
    category: 'Identity & Auth',
    moduleIds: ['rbac'],
    fields: [
      { name: 'userId', type: 'uuid', storage: 'database', required: true, indexed: true },
      { name: 'roleId', type: 'uuid', storage: 'database', required: true, indexed: true },
      { name: 'assignedAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'kyc_record',
    name: 'KYCRecord',
    description: 'Identity verification record for a user.',
    category: 'Identity & Auth',
    moduleIds: ['kyc'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'userId', type: 'uuid', storage: 'database', required: true, indexed: true },
      { name: 'provider', type: 'string', storage: 'database', description: 'e.g. Sumsub, Persona' },
      { name: 'status', type: 'string', storage: 'database', defaultValue: "'pending'", description: 'pending | approved | rejected' },
      { name: 'level', type: 'string', storage: 'database', description: 'basic | enhanced | full' },
      { name: 'submittedAt', type: 'datetime', storage: 'database' },
      { name: 'approvedAt', type: 'datetime', storage: 'database' },
    ],
  },
  {
    id: 'device_key',
    name: 'DeviceKey',
    description: 'Registered device or hardware key for a user.',
    category: 'Identity & Auth',
    moduleIds: ['auth', 'mfa'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'userId', type: 'uuid', storage: 'database', required: true, indexed: true },
      { name: 'publicKey', type: 'string', storage: 'database', required: true, unique: true },
      { name: 'deviceName', type: 'string', storage: 'database' },
      { name: 'lastUsedAt', type: 'datetime', storage: 'database' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },

  // ── Tokens & Assets ────────────────────────────────────────────────
  {
    id: 'token_balance',
    name: 'TokenBalance',
    description: 'ERC-20 token balance for a wallet address.',
    category: 'Tokens & Assets',
    moduleIds: ['erc20', 'token'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'owner', type: 'address', storage: 'on-chain', required: true, indexed: true, description: 'Wallet address that owns the tokens.' },
      { name: 'amount', type: 'uint256', storage: 'on-chain', required: true, description: 'Token amount in smallest denomination (wei).' },
      { name: 'updatedAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'token_transfer',
    name: 'TokenTransfer',
    description: 'Record of an ERC-20 token transfer event.',
    category: 'Tokens & Assets',
    moduleIds: ['erc20', 'token'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'from', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'to', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'amount', type: 'uint256', storage: 'on-chain', required: true },
      { name: 'txHash', type: 'bytes32', storage: 'on-chain', required: true, unique: true },
      { name: 'timestamp', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'nft_asset',
    name: 'NFTAsset',
    description: 'A single NFT (ERC-721) token with on-chain ownership.',
    category: 'Tokens & Assets',
    moduleIds: ['erc721', 'nft'],
    fields: [
      { name: 'tokenId', type: 'uint256', storage: 'on-chain', required: true, unique: true, indexed: true, description: 'Unique numeric ID of the token.' },
      { name: 'owner', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'metadataUri', type: 'ipfs-hash', storage: 'on-chain', description: 'IPFS URI pointing to the token metadata JSON.' },
      { name: 'name', type: 'string', storage: 'database' },
      { name: 'description', type: 'text', storage: 'database' },
      { name: 'imageUrl', type: 'string', storage: 'database' },
      { name: 'mintedAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'nft_collection',
    name: 'NFTCollection',
    description: 'An NFT contract / collection.',
    category: 'Tokens & Assets',
    moduleIds: ['erc721', 'nft'],
    fields: [
      { name: 'address', type: 'address', storage: 'on-chain', required: true, unique: true },
      { name: 'name', type: 'string', storage: 'on-chain', required: true },
      { name: 'symbol', type: 'string', storage: 'on-chain', required: true },
      { name: 'maxSupply', type: 'uint256', storage: 'on-chain' },
      { name: 'mintPrice', type: 'uint256', storage: 'on-chain', description: 'In wei.' },
      { name: 'royaltyBps', type: 'int', storage: 'on-chain', description: 'Royalty in basis points (e.g. 500 = 5%).' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'token_allowance',
    name: 'TokenAllowance',
    description: 'ERC-20 allowance granted from one address to another.',
    category: 'Tokens & Assets',
    moduleIds: ['erc20', 'token'],
    fields: [
      { name: 'owner', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'spender', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'amount', type: 'uint256', storage: 'on-chain', required: true },
    ],
  },

  // ── DeFi & Finance ─────────────────────────────────────────────────
  {
    id: 'liquidity_pool',
    name: 'LiquidityPool',
    description: 'A trading pair pool for AMM swaps.',
    category: 'DeFi & Finance',
    moduleIds: ['dex', 'amm', 'defi'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'address', type: 'address', storage: 'on-chain', required: true, unique: true },
      { name: 'token0', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'token1', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'reserve0', type: 'uint256', storage: 'on-chain' },
      { name: 'reserve1', type: 'uint256', storage: 'on-chain' },
      { name: 'feeBps', type: 'int', storage: 'on-chain', description: 'Fee in basis points (30 = 0.3%).' },
    ],
  },
  {
    id: 'swap_event',
    name: 'SwapEvent',
    description: 'A token swap executed through an AMM.',
    category: 'DeFi & Finance',
    moduleIds: ['dex', 'amm'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'poolId', type: 'uuid', storage: 'database', indexed: true },
      { name: 'sender', type: 'address', storage: 'on-chain', indexed: true },
      { name: 'amountIn', type: 'uint256', storage: 'on-chain' },
      { name: 'amountOut', type: 'uint256', storage: 'on-chain' },
      { name: 'txHash', type: 'bytes32', storage: 'on-chain', unique: true },
      { name: 'timestamp', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'vault',
    name: 'Vault',
    description: 'A yield-bearing vault (ERC-4626 compatible).',
    category: 'DeFi & Finance',
    moduleIds: ['vault', 'yield', 'defi'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'address', type: 'address', storage: 'on-chain', required: true, unique: true },
      { name: 'asset', type: 'address', storage: 'on-chain', required: true },
      { name: 'totalAssets', type: 'uint256', storage: 'on-chain' },
      { name: 'totalShares', type: 'uint256', storage: 'on-chain' },
      { name: 'apy', type: 'float', storage: 'database', description: 'Approximate APY in percent.' },
    ],
  },

  // ── Governance ─────────────────────────────────────────────────────
  {
    id: 'proposal',
    name: 'Proposal',
    description: 'A governance proposal that token holders can vote on.',
    category: 'Governance',
    moduleIds: ['governance', 'dao'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'onChainId', type: 'uint256', storage: 'on-chain', unique: true },
      { name: 'title', type: 'string', storage: 'database', required: true },
      { name: 'description', type: 'text', storage: 'database' },
      { name: 'proposer', type: 'address', storage: 'on-chain', indexed: true },
      { name: 'status', type: 'string', storage: 'database', defaultValue: "'pending'", description: 'pending | active | passed | failed | executed' },
      { name: 'votesFor', type: 'uint256', storage: 'on-chain', defaultValue: '0' },
      { name: 'votesAgainst', type: 'uint256', storage: 'on-chain', defaultValue: '0' },
      { name: 'quorumRequired', type: 'uint256', storage: 'on-chain' },
      { name: 'startTime', type: 'datetime', storage: 'database' },
      { name: 'endTime', type: 'datetime', storage: 'database' },
      { name: 'executedAt', type: 'datetime', storage: 'database' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'vote',
    name: 'Vote',
    description: 'A single vote cast on a governance proposal.',
    category: 'Governance',
    moduleIds: ['governance', 'dao'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'proposalId', type: 'uuid', storage: 'database', required: true, indexed: true },
      { name: 'voter', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'support', type: 'boolean', storage: 'on-chain', required: true, description: 'true = for, false = against' },
      { name: 'weight', type: 'uint256', storage: 'on-chain', description: 'Voting power used.' },
      { name: 'reason', type: 'text', storage: 'database' },
      { name: 'votedAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'delegation',
    name: 'Delegation',
    description: 'Voting power delegation from one address to another.',
    category: 'Governance',
    moduleIds: ['governance', 'dao'],
    fields: [
      { name: 'delegator', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'delegatee', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'amount', type: 'uint256', storage: 'on-chain' },
      { name: 'delegatedAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'multisig_tx',
    name: 'MultiSigTx',
    description: 'A multi-signature transaction requiring threshold approvals.',
    category: 'Governance',
    moduleIds: ['multisig', 'dao'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'to', type: 'address', storage: 'on-chain', required: true },
      { name: 'value', type: 'uint256', storage: 'on-chain', defaultValue: '0' },
      { name: 'data', type: 'bytes32', storage: 'on-chain' },
      { name: 'threshold', type: 'int', storage: 'on-chain' },
      { name: 'approvals', type: 'json', storage: 'database', description: 'List of approving signers.' },
      { name: 'executed', type: 'boolean', storage: 'on-chain', defaultValue: 'false' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },

  // ── Commerce & Marketplace ──────────────────────────────────────────
  {
    id: 'listing',
    name: 'Listing',
    description: 'An NFT or token listed for sale on a marketplace.',
    category: 'Commerce & Marketplace',
    moduleIds: ['marketplace', 'nft-marketplace'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'seller', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'tokenContract', type: 'address', storage: 'on-chain', required: true },
      { name: 'tokenId', type: 'uint256', storage: 'on-chain', indexed: true },
      { name: 'price', type: 'uint256', storage: 'on-chain', required: true, description: 'In wei.' },
      { name: 'currency', type: 'address', storage: 'on-chain', description: 'Zero address = native token.' },
      { name: 'status', type: 'string', storage: 'database', defaultValue: "'active'", description: 'active | sold | cancelled' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
      { name: 'expiresAt', type: 'datetime', storage: 'database' },
    ],
  },
  {
    id: 'sale',
    name: 'Sale',
    description: 'A completed NFT/token sale transaction.',
    category: 'Commerce & Marketplace',
    moduleIds: ['marketplace', 'nft-marketplace'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'listingId', type: 'uuid', storage: 'database', indexed: true },
      { name: 'buyer', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'seller', type: 'address', storage: 'on-chain', indexed: true },
      { name: 'price', type: 'uint256', storage: 'on-chain' },
      { name: 'royaltiesPaid', type: 'uint256', storage: 'on-chain' },
      { name: 'txHash', type: 'bytes32', storage: 'on-chain', unique: true },
      { name: 'saleAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'auction',
    name: 'Auction',
    description: 'A timed English auction for an NFT.',
    category: 'Commerce & Marketplace',
    moduleIds: ['auction', 'marketplace'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'tokenContract', type: 'address', storage: 'on-chain', required: true },
      { name: 'tokenId', type: 'uint256', storage: 'on-chain', required: true },
      { name: 'reservePrice', type: 'uint256', storage: 'on-chain' },
      { name: 'highestBid', type: 'uint256', storage: 'on-chain', defaultValue: '0' },
      { name: 'highestBidder', type: 'address', storage: 'on-chain' },
      { name: 'startTime', type: 'datetime', storage: 'database' },
      { name: 'endTime', type: 'datetime', storage: 'database' },
      { name: 'settled', type: 'boolean', storage: 'on-chain', defaultValue: 'false' },
    ],
  },

  // ── Infrastructure ─────────────────────────────────────────────────
  {
    id: 'audit_log',
    name: 'AuditLog',
    description: 'Immutable record of all significant system events.',
    category: 'Infrastructure',
    moduleIds: ['audit', 'rbac', 'compliance'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'actor', type: 'uuid', storage: 'database', indexed: true, description: 'FK → User.id or address.' },
      { name: 'action', type: 'string', storage: 'database', required: true, indexed: true, description: 'e.g. user.login, proposal.create, token.transfer' },
      { name: 'resource', type: 'string', storage: 'database', description: 'Affected entity type.' },
      { name: 'resourceId', type: 'string', storage: 'database' },
      { name: 'details', type: 'json', storage: 'database', description: 'Additional event context.' },
      { name: 'ipAddress', type: 'string', storage: 'database', encrypted: true },
      { name: 'status', type: 'string', storage: 'database', defaultValue: "'success'" },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()', indexed: true },
    ],
  },
  {
    id: 'api_key',
    name: 'APIKey',
    description: 'API key for programmatic access to the platform.',
    category: 'Infrastructure',
    moduleIds: ['api', 'developer'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'userId', type: 'uuid', storage: 'database', required: true, indexed: true },
      { name: 'name', type: 'string', storage: 'database', description: 'Friendly name for the key.' },
      { name: 'keyHash', type: 'string', storage: 'database', required: true, unique: true, encrypted: true, description: 'Hashed key, never stored in plaintext.' },
      { name: 'scopes', type: 'json', storage: 'database', description: 'Array of permission scopes.' },
      { name: 'lastUsedAt', type: 'datetime', storage: 'database' },
      { name: 'expiresAt', type: 'datetime', storage: 'database' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'webhook_event',
    name: 'WebhookEvent',
    description: 'Outbound webhook event sent to an external endpoint.',
    category: 'Integrations',
    moduleIds: ['webhooks', 'integrations'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'endpoint', type: 'string', storage: 'database', required: true },
      { name: 'eventType', type: 'string', storage: 'database', required: true, indexed: true },
      { name: 'payload', type: 'json', storage: 'database' },
      { name: 'status', type: 'string', storage: 'database', defaultValue: "'pending'", description: 'pending | sent | failed' },
      { name: 'attempts', type: 'int', storage: 'database', defaultValue: '0' },
      { name: 'responseCode', type: 'int', storage: 'database' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },

  // ── Compliance ────────────────────────────────────────────────────
  {
    id: 'aml_flag',
    name: 'AMLFlag',
    description: 'Anti-money laundering flag raised against a wallet or user.',
    category: 'Compliance',
    moduleIds: ['kyc', 'compliance', 'aml'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'userId', type: 'uuid', storage: 'database', indexed: true },
      { name: 'walletAddress', type: 'address', storage: 'database', indexed: true },
      { name: 'riskScore', type: 'float', storage: 'database', description: '0–100 risk score.' },
      { name: 'reason', type: 'text', storage: 'database' },
      { name: 'status', type: 'string', storage: 'database', defaultValue: "'open'", description: 'open | reviewed | cleared' },
      { name: 'flaggedAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
  {
    id: 'geo_block',
    name: 'GeoBlock',
    description: 'Geographic restriction entry for a country or region.',
    category: 'Compliance',
    moduleIds: ['compliance', 'kyc'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'countryCode', type: 'string', storage: 'database', required: true, indexed: true, description: 'ISO-3166 country code.' },
      { name: 'reason', type: 'string', storage: 'database' },
      { name: 'createdAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },

  // ── Legacy & Migration ────────────────────────────────────────────
  {
    id: 'legacy_record',
    name: 'LegacyRecord',
    description: 'A record imported from a legacy system during migration.',
    category: 'Legacy & Migration',
    moduleIds: ['legacy-connect', 'legacy-port'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'sourceSystem', type: 'string', storage: 'database', indexed: true, description: 'Name of the originating system (e.g. SAP, Oracle).' },
      { name: 'sourceId', type: 'string', storage: 'database', indexed: true, description: 'Original ID in the source system.' },
      { name: 'entityType', type: 'string', storage: 'database', indexed: true },
      { name: 'data', type: 'json', storage: 'database', description: 'Full record data from the source system.' },
      { name: 'migrationStatus', type: 'string', storage: 'database', defaultValue: "'pending'", description: 'pending | migrated | failed | skipped' },
      { name: 'migratedAt', type: 'datetime', storage: 'database' },
      { name: 'error', type: 'text', storage: 'database' },
    ],
  },
  {
    id: 'migration_batch',
    name: 'MigrationBatch',
    description: 'A batch run of the data migration process.',
    category: 'Legacy & Migration',
    moduleIds: ['legacy-connect', 'legacy-port'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'name', type: 'string', storage: 'database', required: true },
      { name: 'totalRecords', type: 'int', storage: 'database' },
      { name: 'migratedRecords', type: 'int', storage: 'database', defaultValue: '0' },
      { name: 'failedRecords', type: 'int', storage: 'database', defaultValue: '0' },
      { name: 'status', type: 'string', storage: 'database', defaultValue: "'pending'", description: 'pending | running | completed | failed' },
      { name: 'startedAt', type: 'datetime', storage: 'database' },
      { name: 'completedAt', type: 'datetime', storage: 'database' },
    ],
  },

  // ── Chain-specific ────────────────────────────────────────────────
  {
    id: 'validator',
    name: 'Validator',
    description: 'A network validator node in a private blockchain.',
    category: 'Chain-specific',
    moduleIds: ['staking', 'consensus', 'blockchain'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'address', type: 'address', storage: 'on-chain', required: true, unique: true },
      { name: 'stakedAmount', type: 'uint256', storage: 'on-chain' },
      { name: 'commission', type: 'float', storage: 'on-chain', description: 'Commission rate in percent.' },
      { name: 'status', type: 'string', storage: 'database', defaultValue: "'active'", description: 'active | jailed | inactive' },
      { name: 'uptime', type: 'float', storage: 'database', description: 'Uptime percentage.' },
      { name: 'joinedAt', type: 'datetime', storage: 'database' },
    ],
  },
  {
    id: 'staking_position',
    name: 'StakingPosition',
    description: 'A delegator\'s staking position in a validator.',
    category: 'Chain-specific',
    moduleIds: ['staking'],
    fields: [
      { name: 'id', type: 'uuid', storage: 'database', required: true, unique: true },
      { name: 'delegator', type: 'address', storage: 'on-chain', required: true, indexed: true },
      { name: 'validatorId', type: 'uuid', storage: 'database', indexed: true },
      { name: 'amount', type: 'uint256', storage: 'on-chain', required: true },
      { name: 'rewards', type: 'uint256', storage: 'on-chain', defaultValue: '0' },
      { name: 'unbondingStart', type: 'datetime', storage: 'database' },
      { name: 'stakedAt', type: 'datetime', storage: 'database', defaultValue: 'now()' },
    ],
  },
];

// ─────────────────────────────────────────────────────────────────
// HELPERS
// ─────────────────────────────────────────────────────────────────

/** Return entities whose moduleIds overlap with the given module IDs */
export function getEntitiesForModules(moduleIds: string[]): CatalogEntity[] {
  const lower = moduleIds.map((m) => m.toLowerCase());
  return ENTITIES_CATALOG.filter((e) =>
    e.moduleIds.some((mid) => lower.some((m) => m.includes(mid) || mid.includes(m)))
  );
}

/** Return the first 5 core entities for a module */
export function getCoreEntitiesForModule(moduleId: string): CatalogEntity[] {
  const lower = moduleId.toLowerCase();
  return ENTITIES_CATALOG.filter((e) =>
    e.moduleIds.some((mid) => lower.includes(mid) || mid.includes(lower))
  ).slice(0, 5);
}

/** Group full catalog by category */
export function getEntitiesByCategory(): Record<EntityCategory, CatalogEntity[]> {
  const result: Record<string, CatalogEntity[]> = {};
  for (const cat of ENTITY_CATEGORIES) {
    result[cat] = ENTITIES_CATALOG.filter((e) => e.category === cat);
  }
  return result as Record<EntityCategory, CatalogEntity[]>;
}
