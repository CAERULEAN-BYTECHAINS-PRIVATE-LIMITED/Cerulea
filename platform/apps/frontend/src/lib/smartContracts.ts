// Smart contract derivation logic.
// Reads the current project configuration and returns a list of SmartContract objects.

export type ContractType =
  | 'ERC20'
  | 'ERC721'
  | 'ERC1155'
  | 'Governance'
  | 'Staking'
  | 'Vault'
  | 'Bridge'
  | 'Oracle'
  | 'AccessControl'
  | 'MultiSig'
  | 'Marketplace'
  | 'Custom';

export type ContractSource = 'module' | 'economics' | 'entity' | 'template';

export interface SmartContract {
  id: string;
  name: string;
  contractType: ContractType;
  description: string;
  whyItExists: string;
  source: ContractSource;
  /** Which modules trigger this contract */
  dependentModules: string[];
  /** Which entities use on-chain storage from this contract */
  dependentEntities: string[];
  /** Impact if this contract is disabled */
  ifDisabled: string;
  enabled: boolean;
  /** Simplified ABI stub */
  abi?: object[];
  /** Solidity stub source */
  sourceSolidity?: string;
  /** Constructor params */
  constructorParams?: Array<{ name: string; type: string; description: string; defaultValue?: string }>;
}

// ─────────────────────────────────────────────────────────────────
// Module → Contract mapping
// ─────────────────────────────────────────────────────────────────

const MODULE_CONTRACT_MAP: Record<string, Omit<SmartContract, 'id' | 'enabled' | 'dependentEntities'>> = {
  erc20: {
    name: 'ERC20Token',
    contractType: 'ERC20',
    description: 'A standard fungible token (ERC-20). Used for governance tokens, utility tokens, or payment currencies within your app.',
    whyItExists: 'The ERC-20 module in your Blueprint requires an on-chain token contract to manage balances, transfers, and approvals.',
    source: 'module',
    dependentModules: ['erc20'],
    ifDisabled: 'All token balances, transfers, and any feature depending on fungible tokens will stop working. This will break Staking, Governance, and Revenue modules if they depend on this token.',
    constructorParams: [
      { name: 'name', type: 'string', description: 'Full token name', defaultValue: 'My Token' },
      { name: 'symbol', type: 'string', description: 'Ticker symbol', defaultValue: 'TOKEN' },
      { name: 'initialSupply', type: 'uint256', description: 'Initial minted supply (in wei)', defaultValue: '1000000000000000000000000' },
    ],
  },
  token: {
    name: 'ERC20Token',
    contractType: 'ERC20',
    description: 'A standard fungible token contract.',
    whyItExists: 'The Token module requires an ERC-20 contract for balance management.',
    source: 'module',
    dependentModules: ['token'],
    ifDisabled: 'Token functionality disabled.',
    constructorParams: [
      { name: 'name', type: 'string', description: 'Token name', defaultValue: 'My Token' },
      { name: 'symbol', type: 'string', description: 'Token symbol', defaultValue: 'MTK' },
    ],
  },
  erc721: {
    name: 'ERC721NFT',
    contractType: 'ERC721',
    description: 'A non-fungible token (NFT) contract. Each token has a unique ID and can represent ownership of digital or physical assets.',
    whyItExists: 'The NFT module in your Blueprint requires an ERC-721 contract to mint, transfer, and verify ownership of unique tokens.',
    source: 'module',
    dependentModules: ['erc721', 'nft'],
    ifDisabled: 'All NFT minting, transfers, and ownership checks will stop working. The Marketplace module will become non-functional.',
    constructorParams: [
      { name: 'name', type: 'string', description: 'Collection name', defaultValue: 'My NFT Collection' },
      { name: 'symbol', type: 'string', description: 'Collection symbol', defaultValue: 'MNFT' },
      { name: 'baseURI', type: 'string', description: 'Base metadata URI', defaultValue: 'ipfs://...' },
    ],
  },
  erc1155: {
    name: 'ERC1155MultiToken',
    contractType: 'ERC1155',
    description: 'A multi-token standard supporting both fungible and non-fungible tokens in a single contract.',
    whyItExists: 'The ERC-1155 module enables batch minting and transfers of mixed token types, reducing gas costs significantly.',
    source: 'module',
    dependentModules: ['erc1155'],
    ifDisabled: 'Multi-token functionality disabled. Batch operations and gaming item contracts will stop working.',
    constructorParams: [
      { name: 'uri', type: 'string', description: 'Base metadata URI template', defaultValue: 'ipfs://{id}.json' },
    ],
  },
  governance: {
    name: 'GovernorContract',
    contractType: 'Governance',
    description: 'A governance contract that manages proposals, voting, and execution of changes to the protocol.',
    whyItExists: 'The Governance module requires an on-chain Governor to ensure proposals and votes are transparently recorded and tamper-proof.',
    source: 'module',
    dependentModules: ['governance', 'dao'],
    ifDisabled: 'All governance proposals, voting, and on-chain execution will be disabled. The DAO module will become non-functional.',
    constructorParams: [
      { name: 'token', type: 'address', description: 'Address of the governance token (ERC-20 with votes)' },
      { name: 'quorumNumerator', type: 'uint256', description: 'Quorum % (e.g. 4 = 4%)', defaultValue: '4' },
      { name: 'votingDelay', type: 'uint256', description: 'Blocks before voting starts', defaultValue: '1' },
      { name: 'votingPeriod', type: 'uint256', description: 'Blocks voting is open', defaultValue: '45818' },
    ],
  },
  staking: {
    name: 'StakingRewards',
    contractType: 'Staking',
    description: 'A staking contract that lets users lock tokens to earn rewards and participate in network validation.',
    whyItExists: 'The Staking module requires an on-chain contract to track locked positions, distribute rewards, and enforce unbonding periods.',
    source: 'module',
    dependentModules: ['staking'],
    ifDisabled: 'Staking will be completely disabled. Validators cannot stake and users cannot earn staking rewards.',
    constructorParams: [
      { name: 'stakingToken', type: 'address', description: 'Token to stake' },
      { name: 'rewardsToken', type: 'address', description: 'Token distributed as reward' },
      { name: 'minStake', type: 'uint256', description: 'Minimum stake amount (in wei)' },
    ],
  },
  marketplace: {
    name: 'NFTMarketplace',
    contractType: 'Marketplace',
    description: 'A peer-to-peer marketplace for listing, buying, and selling NFTs.',
    whyItExists: 'The Marketplace module requires an on-chain escrow contract to hold tokens during sale and enforce royalty payments.',
    source: 'module',
    dependentModules: ['marketplace', 'nft-marketplace'],
    ifDisabled: 'All marketplace listings, bids, and sales will be disabled.',
    constructorParams: [
      { name: 'platformFee', type: 'uint256', description: 'Platform fee in basis points (e.g. 250 = 2.5%)' },
      { name: 'treasury', type: 'address', description: 'Address to collect platform fees' },
    ],
  },
  dex: {
    name: 'DEXRouter',
    contractType: 'Custom',
    description: 'A decentralized exchange router contract enabling token swaps through automated market maker (AMM) liquidity pools.',
    whyItExists: 'The DEX module needs an on-chain router to execute atomic swaps, calculate prices via the constant product formula, and route trades through optimal paths.',
    source: 'module',
    dependentModules: ['dex'],
    ifDisabled: 'All token swap functionality disabled. Users cannot exchange tokens directly in the app.',
    constructorParams: [
      { name: 'factory', type: 'address', description: 'Address of the DEX factory contract' },
      { name: 'WETH', type: 'address', description: 'Wrapped native token address for ETH ↔ token pairs' },
    ],
  },
  liquidity: {
    name: 'LiquidityPool',
    contractType: 'Vault',
    description: 'A constant-product AMM liquidity pool contract (Uniswap v2-style). Holds token reserves and issues LP tokens to liquidity providers.',
    whyItExists: 'The DEX and Liquidity modules require on-chain pool contracts to store reserves and enable permissionless market making.',
    source: 'module',
    dependentModules: ['liquidity', 'dex'],
    ifDisabled: 'Liquidity provision and removal disabled. DEX pricing will break without functioning pools.',
    constructorParams: [
      { name: 'tokenA', type: 'address', description: 'First token in the pair' },
      { name: 'tokenB', type: 'address', description: 'Second token in the pair' },
    ],
  },
  lending: {
    name: 'LendingPool',
    contractType: 'Vault',
    description: 'A DeFi lending pool where users can deposit collateral and borrow assets, with automated liquidation on under-collateralized positions.',
    whyItExists: 'The Lending module requires an on-chain pool to track deposits, debt positions, interest accrual, and health factors for each user.',
    source: 'module',
    dependentModules: ['lending', 'defi-lending'],
    ifDisabled: 'All lending and borrowing functionality disabled. Users cannot deposit collateral or take loans.',
    constructorParams: [
      { name: 'collateralToken', type: 'address', description: 'Accepted collateral asset address' },
      { name: 'borrowToken', type: 'address', description: 'Borrowable asset address' },
      { name: 'liquidationThreshold', type: 'uint256', description: 'Collateral ratio before liquidation (e.g. 150 = 150%)', defaultValue: '150' },
      { name: 'interestRatePerBlock', type: 'uint256', description: 'Interest accrued per block in basis points', defaultValue: '1' },
    ],
  },
  vault: {
    name: 'YieldVault',
    contractType: 'Vault',
    description: 'An ERC-4626 compliant yield vault that auto-compounds deposited tokens into a configured yield strategy.',
    whyItExists: 'The Vault module requires an on-chain strategy contract to track shares, auto-compound yields, and handle withdrawal queues.',
    source: 'module',
    dependentModules: ['vault', 'yield'],
    ifDisabled: 'Yield vault deposits and strategy execution disabled. Deposited funds will not earn yield.',
    constructorParams: [
      { name: 'asset', type: 'address', description: 'Underlying ERC-20 token to deposit' },
      { name: 'name', type: 'string', description: 'Vault share token name', defaultValue: 'Vault Share' },
      { name: 'symbol', type: 'string', description: 'Vault share token symbol', defaultValue: 'vTKN' },
    ],
  },
  bridge: {
    name: 'TokenBridge',
    contractType: 'Bridge',
    description: 'A cross-chain bridge contract that locks tokens on the source chain and emits events for minting on the destination chain.',
    whyItExists: 'The Bridge module requires lock-and-mint contracts on both chains to enable secure cross-chain token transfers.',
    source: 'module',
    dependentModules: ['bridge'],
    ifDisabled: 'Cross-chain token transfers disabled. Users cannot move tokens between this chain and connected chains.',
    constructorParams: [
      { name: 'token', type: 'address', description: 'Token to bridge' },
      { name: 'relayer', type: 'address', description: 'Authorized relayer address for cross-chain messages' },
      { name: 'destinationChainId', type: 'uint256', description: 'Chain ID of the destination network' },
    ],
  },
  oracle: {
    name: 'PriceOracle',
    contractType: 'Oracle',
    description: 'A Chainlink-compatible price oracle aggregator contract that provides tamper-resistant asset price feeds on-chain.',
    whyItExists: 'The Oracle module requires an on-chain price feed contract to safely consume external asset prices for DeFi calculations (collateral ratios, swap pricing).',
    source: 'module',
    dependentModules: ['oracle', 'chainlink'],
    ifDisabled: 'On-chain price feeds disabled. Lending, DEX, and any price-dependent contract logic will fail.',
    constructorParams: [
      { name: 'aggregator', type: 'address', description: 'Chainlink aggregator contract address' },
      { name: 'decimals', type: 'uint8', description: 'Price feed decimal precision', defaultValue: '8' },
    ],
  },
  multisig: {
    name: 'MultiSigWallet',
    contractType: 'MultiSig',
    description: 'A multi-signature wallet requiring M-of-N owners to approve transactions before execution.',
    whyItExists: 'The MultiSig module requires an on-chain wallet contract for secure, distributed control of funds.',
    source: 'module',
    dependentModules: ['multisig'],
    ifDisabled: 'Multi-signature functionality disabled. Treasury operations will require manual coordination.',
    constructorParams: [
      { name: 'owners', type: 'address[]', description: 'List of owner addresses' },
      { name: 'required', type: 'uint256', description: 'Number of required approvals', defaultValue: '2' },
    ],
  },
  rbac: {
    name: 'AccessControl',
    contractType: 'AccessControl',
    description: 'An on-chain access control contract (OpenZeppelin AccessControl) for managing roles and permissions.',
    whyItExists: 'The RBAC module requires an on-chain access control contract to enforce permission rules for contract functions.',
    source: 'module',
    dependentModules: ['rbac', 'access-control'],
    ifDisabled: 'On-chain access control disabled. Any contract functions gated by roles will become unrestricted.',
  },
  airdrop: {
    name: 'AirdropDistributor',
    contractType: 'Custom',
    description: 'A Merkle-proof based airdrop contract that allows eligible addresses to claim token allocations in a gas-efficient manner.',
    whyItExists: 'The Airdrop module uses a Merkle tree distribution to verify claims without storing every recipient address on-chain.',
    source: 'module',
    dependentModules: ['airdrop'],
    ifDisabled: 'Airdrop claim functionality disabled. Eligible addresses cannot claim their token allocations.',
    constructorParams: [
      { name: 'token', type: 'address', description: 'Token to distribute' },
      { name: 'merkleRoot', type: 'bytes32', description: 'Merkle root of the distribution list' },
    ],
  },
  vesting: {
    name: 'VestingSchedule',
    contractType: 'Custom',
    description: 'A token vesting contract with configurable cliff and linear release schedule. Used for team, investor, and advisor token unlocks.',
    whyItExists: 'The Vesting module enforces on-chain lock-up periods so token allocations are released gradually rather than immediately.',
    source: 'module',
    dependentModules: ['vesting'],
    ifDisabled: 'Vesting schedules disabled. Locked tokens will not be released on schedule.',
    constructorParams: [
      { name: 'token', type: 'address', description: 'Token being vested' },
      { name: 'cliffDuration', type: 'uint256', description: 'Cliff period in seconds', defaultValue: '2592000' },
      { name: 'vestingDuration', type: 'uint256', description: 'Total vesting period in seconds', defaultValue: '31536000' },
    ],
  },
  auction: {
    name: 'DutchAuction',
    contractType: 'Marketplace',
    description: 'A Dutch auction contract where price decreases over time until a buyer accepts, commonly used for token launches and NFT sales.',
    whyItExists: 'The Auction module uses a Dutch auction mechanism to achieve fair price discovery without order book infrastructure.',
    source: 'module',
    dependentModules: ['auction'],
    ifDisabled: 'Auction functionality disabled. Dutch auction sales and token launches via auction will be unavailable.',
    constructorParams: [
      { name: 'startPrice', type: 'uint256', description: 'Starting price in wei', defaultValue: '1000000000000000000' },
      { name: 'endPrice', type: 'uint256', description: 'Floor price in wei', defaultValue: '100000000000000000' },
      { name: 'duration', type: 'uint256', description: 'Auction duration in seconds', defaultValue: '86400' },
    ],
  },
  kyc: {
    name: 'KYCRegistry',
    contractType: 'AccessControl',
    description: 'An on-chain KYC registry that stores verified address attestations from your KYC provider (Sumsub). Gated operations check this registry.',
    whyItExists: 'The KYC module needs an on-chain whitelist to enforce verified-user-only access for compliant DeFi and regulated token operations.',
    source: 'module',
    dependentModules: ['kyc', 'sumsub'],
    ifDisabled: 'KYC verification checks disabled. Regulated features will have no compliance gate and may violate legal requirements.',
    constructorParams: [
      { name: 'kycOperator', type: 'address', description: 'Authorized KYC operator address (backend wallet)' },
    ],
  },
  payments: {
    name: 'PaymentSplitter',
    contractType: 'Custom',
    description: 'A payment splitter contract that receives funds and distributes them proportionally to a list of payees based on configured shares.',
    whyItExists: 'The Payments module uses an on-chain splitter for trustless, automatic revenue distribution to stakeholders.',
    source: 'module',
    dependentModules: ['payments', 'revenue'],
    ifDisabled: 'On-chain payment splitting disabled. Revenue distribution must be done manually off-chain.',
    constructorParams: [
      { name: 'payees', type: 'address[]', description: 'List of payee addresses' },
      { name: 'shares', type: 'uint256[]', description: 'Relative shares for each payee' },
    ],
  },
};

// ─────────────────────────────────────────────────────────────────
// Economics-derived contracts
// ─────────────────────────────────────────────────────────────────

function getEconomicsContracts(economics: any, projectType: string): SmartContract[] {
  const contracts: SmartContract[] = [];

  if (projectType === 'blockchain') {
    if (economics?.chainToken) {
      contracts.push({
        id: 'chain-native-token',
        name: 'NativeToken',
        contractType: 'ERC20',
        description: 'The native token of your private blockchain. Used for gas fees and staking.',
        whyItExists: 'Configured in the Tokenomics section of the Economics step.',
        source: 'economics',
        dependentModules: [],
        dependentEntities: [],
        ifDisabled: 'The blockchain\'s native token cannot be issued. Validators will have nothing to stake.',
        enabled: true,
        constructorParams: [
          { name: 'name', type: 'string', description: 'Token name', defaultValue: economics.chainToken.name || 'Network Token' },
          { name: 'symbol', type: 'string', description: 'Token symbol', defaultValue: economics.chainToken.symbol || 'NET' },
          { name: 'totalSupply', type: 'uint256', description: 'Initial supply', defaultValue: String(economics.chainToken.supply || '100000000') },
        ],
      });
    }
  }

  return contracts;
}

// ─────────────────────────────────────────────────────────────────
// Demo contracts for test account showcase
// ─────────────────────────────────────────────────────────────────

export function getDemoContracts(): SmartContract[] {
  return [
    {
      id: 'demo-erc20',
      name: 'CeruleaToken',
      contractType: 'ERC20',
      description: 'The primary governance and utility token for this dApp. Used for staking, voting, and fee payments across the platform.',
      whyItExists: 'Generated from the ERC-20 Token module in Blueprint. Every Cerulea dApp with tokenomics needs a fungible token contract.',
      source: 'module',
      dependentModules: ['ERC-20 Token', 'Staking', 'Governance DAO'],
      dependentEntities: ['Token', 'Balance', 'Transfer', 'Allowance'],
      ifDisabled: 'Staking, governance voting, and all fee payments will break. This is the foundational contract disabling it cascades to 7 other contracts.',
      enabled: true,
      constructorParams: [
        { name: 'name', type: 'string', description: 'Full token name', defaultValue: 'Cerulea Token' },
        { name: 'symbol', type: 'string', description: 'Ticker symbol', defaultValue: 'CERU' },
        { name: 'initialSupply', type: 'uint256', description: 'Initial minted supply (in wei)', defaultValue: '100000000000000000000000000' },
      ],
    },
    {
      id: 'demo-erc721',
      name: 'CeruleaNFT',
      contractType: 'ERC721',
      description: 'NFT collection contract with on-chain metadata, royalty enforcement (EIP-2981), and access-gated minting.',
      whyItExists: 'Generated from the NFT Minting module. Handles unique token ownership, metadata URIs, and royalty distributions on secondary sales.',
      source: 'module',
      dependentModules: ['NFT Minting', 'NFT Marketplace', 'IPFS Storage'],
      dependentEntities: ['NFTAsset', 'NFTMetadata', 'Collection', 'Royalty'],
      ifDisabled: 'NFT minting, transfers, and royalties disabled. The Marketplace will have no NFTs to list or trade.',
      enabled: true,
      constructorParams: [
        { name: 'name', type: 'string', description: 'Collection name', defaultValue: 'Cerulea Originals' },
        { name: 'symbol', type: 'string', description: 'Collection symbol', defaultValue: 'CORG' },
        { name: 'baseURI', type: 'string', description: 'Base metadata URI', defaultValue: 'ipfs://QmCerulea/' },
        { name: 'royaltyBps', type: 'uint96', description: 'Royalty basis points (500 = 5%)', defaultValue: '500' },
      ],
    },
    {
      id: 'demo-erc1155',
      name: 'GameItems',
      contractType: 'ERC1155',
      description: 'Multi-token contract for in-app game items, badges, and semi-fungible assets. Supports batch minting and transfers.',
      whyItExists: 'Generated from the ERC-1155 Multi-Token module for efficient management of many item types in a single contract.',
      source: 'module',
      dependentModules: ['ERC-1155 Multi-Token', 'Inventory'],
      dependentEntities: ['GameItem', 'ItemBalance', 'BatchTransfer'],
      ifDisabled: 'Game item minting and inventory management disabled. Batch operations will become unavailable.',
      enabled: true,
      constructorParams: [
        { name: 'uri', type: 'string', description: 'Base metadata URI', defaultValue: 'ipfs://QmGameItems/{id}.json' },
      ],
    },
    {
      id: 'demo-governance',
      name: 'CeruleaGovernor',
      contractType: 'Governance',
      description: 'On-chain governance contract based on OpenZeppelin Governor. Manages proposals, on-chain voting, timelock, and execution of protocol changes.',
      whyItExists: 'Generated from the Governance DAO module. Enables token-weighted voting so the community controls protocol parameters.',
      source: 'module',
      dependentModules: ['Governance DAO', 'ERC-20 Token', 'Treasury'],
      dependentEntities: ['Proposal', 'Vote', 'Delegation', 'TreasuryTx'],
      ifDisabled: 'All governance proposals and voting disabled. Protocol upgrades must go through centralized admin control.',
      enabled: true,
      constructorParams: [
        { name: 'token', type: 'address', description: 'Governance token address (CeruleaToken)' },
        { name: 'quorumNumerator', type: 'uint256', description: 'Quorum % required', defaultValue: '4' },
        { name: 'votingDelay', type: 'uint256', description: 'Delay before voting opens (blocks)', defaultValue: '7200' },
        { name: 'votingPeriod', type: 'uint256', description: 'Duration of voting window (blocks)', defaultValue: '50400' },
        { name: 'proposalThreshold', type: 'uint256', description: 'Minimum tokens to create a proposal', defaultValue: '100000000000000000000' },
      ],
    },
    {
      id: 'demo-staking',
      name: 'StakingRewards',
      contractType: 'Staking',
      description: 'Token staking contract with configurable APY, cliff periods, slashing conditions for misbehavior, and compound interest.',
      whyItExists: 'Generated from the Staking module. Incentivizes long-term token holding and validator participation with yield rewards.',
      source: 'module',
      dependentModules: ['Staking', 'ERC-20 Token', 'Validator Set'],
      dependentEntities: ['StakePosition', 'Reward', 'SlashEvent', 'Validator'],
      ifDisabled: 'All staking disabled. APY earnings stop, validators cannot stake to participate in consensus.',
      enabled: true,
      constructorParams: [
        { name: 'stakingToken', type: 'address', description: 'Token users stake (CeruleaToken)' },
        { name: 'rewardRate', type: 'uint256', description: 'Reward tokens per block', defaultValue: '1000000000000000' },
        { name: 'minStakePeriod', type: 'uint256', description: 'Minimum lock duration in seconds', defaultValue: '2592000' },
        { name: 'slashingPct', type: 'uint256', description: 'Slash percentage on violation (bps)', defaultValue: '1000' },
      ],
    },
    {
      id: 'demo-marketplace',
      name: 'NFTMarketplace',
      contractType: 'Marketplace',
      description: 'Full-featured NFT marketplace with fixed-price listings, English auctions, offer/counter-offer, and on-chain royalty enforcement.',
      whyItExists: 'Generated from the NFT Marketplace module. Provides the escrow and settlement logic needed for trustless peer-to-peer NFT trading.',
      source: 'module',
      dependentModules: ['NFT Marketplace', 'NFT Minting', 'ERC-20 Token'],
      dependentEntities: ['Listing', 'Offer', 'Sale', 'Auction', 'Bid'],
      ifDisabled: 'All marketplace listings, bids, and sales disabled. NFTs can be minted but not traded through the platform.',
      enabled: true,
      constructorParams: [
        { name: 'platformFee', type: 'uint256', description: 'Platform fee in basis points', defaultValue: '250' },
        { name: 'treasury', type: 'address', description: 'Platform fee recipient address' },
        { name: 'allowedPaymentTokens', type: 'address[]', description: 'ERC-20 tokens accepted as payment' },
      ],
    },
    {
      id: 'demo-dex',
      name: 'DEXRouter',
      contractType: 'Custom',
      description: 'AMM-based DEX router enabling multi-hop token swaps, deadline-protected transactions, and slippage controls.',
      whyItExists: 'Generated from the DEX module. Routes user swaps through the most efficient liquidity pool path using the constant product formula.',
      source: 'module',
      dependentModules: ['DEX', 'Liquidity Pool', 'Price Oracle'],
      dependentEntities: ['SwapEvent', 'Pool', 'LPPosition'],
      ifDisabled: 'All in-app token swapping disabled. Users must use external DEXes to exchange tokens.',
      enabled: true,
      constructorParams: [
        { name: 'factory', type: 'address', description: 'DEX factory contract address' },
        { name: 'WETH', type: 'address', description: 'Wrapped native token for ETH pairs' },
      ],
    },
    {
      id: 'demo-lending',
      name: 'LendingPool',
      contractType: 'Vault',
      description: 'Over-collateralized DeFi lending pool with variable interest rates, automated liquidations, and flash loan support.',
      whyItExists: 'Generated from the DeFi Lending module. Tracks collateral positions, calculates health factors, and executes liquidations when positions become unsafe.',
      source: 'module',
      dependentModules: ['DeFi Lending', 'Price Oracle', 'ERC-20 Token'],
      dependentEntities: ['Loan', 'Collateral', 'LiquidationEvent', 'InterestIndex'],
      ifDisabled: 'Lending and borrowing disabled. Collateral cannot be deposited and no loans can be issued.',
      enabled: true,
      constructorParams: [
        { name: 'collateralFactor', type: 'uint256', description: 'Max borrow as % of collateral (e.g. 75)', defaultValue: '75' },
        { name: 'liquidationThreshold', type: 'uint256', description: 'Health factor threshold for liquidation', defaultValue: '80' },
        { name: 'liquidationBonus', type: 'uint256', description: 'Bonus % given to liquidators', defaultValue: '5' },
      ],
    },
    {
      id: 'demo-bridge',
      name: 'TokenBridge',
      contractType: 'Bridge',
      description: 'Lock-and-mint cross-chain bridge for moving tokens between this network and external EVM chains, with relayer-based message passing.',
      whyItExists: 'Generated from the Bridge module. Locks tokens on this chain and relays cross-chain messages to trigger minting on the destination chain.',
      source: 'module',
      dependentModules: ['Bridge', 'ERC-20 Token'],
      dependentEntities: ['BridgeDeposit', 'BridgeWithdrawal', 'RelayerEvent'],
      ifDisabled: 'Cross-chain token transfers disabled. Funds already in the bridge will be locked until re-enabled.',
      enabled: true,
      constructorParams: [
        { name: 'relayer', type: 'address', description: 'Trusted relayer address for cross-chain messages' },
        { name: 'destinationChainId', type: 'uint256', description: 'Target chain ID (e.g. 1 for Ethereum mainnet)', defaultValue: '1' },
        { name: 'bridgeFee', type: 'uint256', description: 'Bridge fee in basis points', defaultValue: '30' },
      ],
    },
    {
      id: 'demo-oracle',
      name: 'PriceOracle',
      contractType: 'Oracle',
      description: 'Chainlink data feed aggregator that provides tamper-resistant asset prices for on-chain calculations.',
      whyItExists: 'Generated from the Oracle (Chainlink) module. DeFi modules (Lending, DEX, Staking) require reliable on-chain prices to calculate ratios and trigger liquidations.',
      source: 'module',
      dependentModules: ['Oracle', 'DeFi Lending', 'DEX', 'Staking'],
      dependentEntities: ['PriceFeed', 'PriceSnapshot'],
      ifDisabled: 'On-chain price feeds stop. Lending liquidations, DEX pricing, and any price-dependent logic will fail or become exploitable.',
      enabled: true,
      constructorParams: [
        { name: 'feedAddress', type: 'address', description: 'Chainlink aggregator proxy address' },
        { name: 'stalePriceThreshold', type: 'uint256', description: 'Seconds before price is considered stale', defaultValue: '3600' },
      ],
    },
    {
      id: 'demo-multisig',
      name: 'MultiSigTreasury',
      contractType: 'MultiSig',
      description: 'Gnosis Safe-compatible multi-signature wallet for treasury management. Requires 3-of-5 signers to approve any transfer.',
      whyItExists: 'Generated from the MultiSig Wallet module. Protects treasury funds behind a distributed approval process so no single key controls funds.',
      source: 'module',
      dependentModules: ['MultiSig Wallet', 'Governance DAO', 'Treasury'],
      dependentEntities: ['MultiSigTx', 'Signature', 'Owner'],
      ifDisabled: 'Multi-sig approval disabled. Treasury becomes controlled by a single admin address (significant security risk).',
      enabled: true,
      constructorParams: [
        { name: 'owners', type: 'address[]', description: 'Initial signer addresses (5 recommended)' },
        { name: 'threshold', type: 'uint256', description: 'Required approvals', defaultValue: '3' },
      ],
    },
    {
      id: 'demo-airdrop',
      name: 'AirdropDistributor',
      contractType: 'Custom',
      description: 'Merkle-proof airdrop contract. Stores only the Merkle root on-chain; users prove eligibility with a proof from the snapshot.',
      whyItExists: 'Generated from the Airdrop module. Allows distributing tokens to tens of thousands of addresses without storing all addresses on-chain.',
      source: 'module',
      dependentModules: ['Airdrop', 'ERC-20 Token'],
      dependentEntities: ['AirdropClaim', 'Snapshot'],
      ifDisabled: 'Airdrop claims disabled. Users from the snapshot cannot claim their token allocations.',
      enabled: true,
      constructorParams: [
        { name: 'token', type: 'address', description: 'Token to distribute (CeruleaToken)' },
        { name: 'merkleRoot', type: 'bytes32', description: 'Root hash of the distribution Merkle tree' },
        { name: 'claimDeadline', type: 'uint256', description: 'Unix timestamp when claims expire', defaultValue: '1780000000' },
      ],
    },
    {
      id: 'demo-vesting',
      name: 'TeamVesting',
      contractType: 'Custom',
      description: 'Token vesting contract with 12-month cliff and 36-month linear release. Covers team, advisors, and investor allocations.',
      whyItExists: 'Generated from the Vesting Schedule module. Enforces token lock-ups on-chain so allocations cannot be dumped immediately.',
      source: 'module',
      dependentModules: ['Vesting Schedule', 'ERC-20 Token'],
      dependentEntities: ['VestingGrant', 'VestingRelease', 'Beneficiary'],
      ifDisabled: 'Vesting enforcement disabled. All locked tokens become immediately claimable, removing supply lockup protection.',
      enabled: true,
      constructorParams: [
        { name: 'token', type: 'address', description: 'Token being vested (CeruleaToken)' },
        { name: 'cliff', type: 'uint256', description: 'Cliff period in seconds', defaultValue: '31536000' },
        { name: 'duration', type: 'uint256', description: 'Total vesting period in seconds', defaultValue: '94608000' },
        { name: 'revocable', type: 'bool', description: 'Whether the contract owner can revoke grants', defaultValue: 'true' },
      ],
    },
    {
      id: 'demo-kyc',
      name: 'KYCRegistry',
      contractType: 'AccessControl',
      description: 'On-chain KYC attestation registry. The backend writes verified addresses after Sumsub verification; contracts check this before gated operations.',
      whyItExists: 'Generated from the KYC (Sumsub) module. Regulatory compliance requires verified-user-only access for certain DeFi and token sale features.',
      source: 'module',
      dependentModules: ['KYC / AML', 'Sumsub'],
      dependentEntities: ['KYCRecord', 'VerificationAttempt', 'GeoBlock'],
      ifDisabled: 'KYC verification checks disabled. Regulated features become accessible to unverified users compliance violation risk.',
      enabled: true,
      constructorParams: [
        { name: 'operator', type: 'address', description: 'Backend wallet authorized to write KYC attestations' },
        { name: 'expiryPeriod', type: 'uint256', description: 'Seconds until KYC attestation expires', defaultValue: '31536000' },
      ],
    },
    {
      id: 'demo-rbac',
      name: 'RoleManager',
      contractType: 'AccessControl',
      description: 'OpenZeppelin AccessControl implementation. Manages ADMIN, MINTER, OPERATOR, and PAUSER roles across all platform contracts.',
      whyItExists: 'Generated from the RBAC module. Provides a central role registry that all other contracts reference for permission checks.',
      source: 'module',
      dependentModules: ['RBAC / Access Control', 'ERC-20 Token', 'NFT Minting', 'Governance DAO'],
      dependentEntities: ['Role', 'RoleAssignment', 'PermissionLog'],
      ifDisabled: 'Centralized role management disabled. Each contract falls back to its own independent owner check no unified permission model.',
      enabled: true,
      constructorParams: [
        { name: 'admin', type: 'address', description: 'Initial default admin address' },
      ],
    },
  ];
}

// ─────────────────────────────────────────────────────────────────
// Main derivation function
// ─────────────────────────────────────────────────────────────────

export function deriveSmartContracts(
  selectedModules: string[],
  projectType: string | null,
  economics?: any
): SmartContract[] {
  const seen = new Set<string>();
  const result: SmartContract[] = [];

  for (const moduleId of selectedModules) {
    const lower = moduleId.toLowerCase();
    for (const [key, contractDef] of Object.entries(MODULE_CONTRACT_MAP)) {
      if (lower.includes(key) || key.includes(lower)) {
        if (!seen.has(contractDef.name)) {
          seen.add(contractDef.name);
          result.push({
            id: `module-${key}`,
            ...contractDef,
            dependentEntities: [],
            enabled: true,
          });
        }
      }
    }
  }

  // Add economics-derived contracts
  if (economics) {
    for (const c of getEconomicsContracts(economics, projectType || '')) {
      if (!seen.has(c.name)) {
        seen.add(c.name);
        result.push(c);
      }
    }
  }

  // If no contracts found and it's a blockchain project, add a minimal set
  if (result.length === 0 && projectType === 'blockchain') {
    result.push({
      id: 'default-token',
      name: 'NativeToken',
      contractType: 'ERC20',
      description: 'Default native token contract for your private blockchain.',
      whyItExists: 'Every Cerulea private blockchain requires a native token for gas fees.',
      source: 'template',
      dependentModules: [],
      dependentEntities: [],
      ifDisabled: 'Cannot run a blockchain without a native token.',
      enabled: true,
    });
  }

  return result;
}
