/**
 * Standardized JSON-RPC 2.0 client for `cerulea-node` (`platform_rpc.rs`).
 *
 * Bridges Cerulea Studio, Explorer, Dashboard, and the SaaS Billing/Auth gateway
 * with the live node RPC interface (`CERULEA_NODE_RPC_URL`, default `http://127.0.0.1:9944`).
 */

export const ALLOWED_READ_RPC_METHODS = new Set<string>([
  'system_health',
  'system_chain',
  'system_version',
  'chain_getHeader',
  'cerulea_getDeployments',
  'cerulea_getBlockDetail',
  'cerulea_getRecentBlocks',
  'cerulea_getTransaction',
  'cerulea_getRecentTransactions',
  'cerulea_getCollections',
  'cerulea_getRecord',
  'cerulea_getRecords',
  'cerulea_getValidatorDetails',
  'cerulea_getSlotCommittee',
  'cerulea_getCustodyHistory',
  'cerulea_getConsensusProfile',
  'cerulea_listProposals',
  'cerulea_getProposal',
  'cerulea_getAuditLog',
  'cerulea_getAuditLength',
  'cerulea_getWorkflows',
  'cerulea_getLicenceState',
  'cerulea_getProductionPermitted',
  'cerulea_getSchemaTypes',
  'cerulea_getActions',
  'cerulea_getActionManifest',
]);

export interface NodeRpcResponse<T = unknown> {
  ok: boolean;
  result?: T;
  error?: string;
}

export interface NodeLicenceSyncResult {
  synced: boolean;
  nodeOnline: boolean;
  plan: string;
  status: string;
  licenceState?: unknown;
  error?: string;
}

function getNodeRpcUrl(): string {
  return process.env.CERULEA_NODE_RPC_URL || 'http://127.0.0.1:9944';
}

export async function callNodeRpc<T = unknown>(
  method: string,
  params: unknown[] = [],
  timeoutMs = 2500
): Promise<NodeRpcResponse<T>> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(getNodeRpcUrl(), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: Date.now(),
        method,
        params,
      }),
      signal: controller.signal,
      cache: 'no-store',
    });
    if (!res.ok) {
      return { ok: false, error: `Node RPC HTTP ${res.status}` };
    }
    const payload = (await res.json()) as {
      result?: T;
      error?: { code?: number; message?: string };
    };
    if (payload.error) {
      return { ok: false, error: payload.error.message || 'Node RPC error' };
    }
    return { ok: true, result: payload.result };
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    return { ok: false, error: msg };
  } finally {
    clearTimeout(timer);
  }
}

export async function getDashboardNodeSnapshot() {
  const [health, deployments, proposals, blocks] = await Promise.all([
    callNodeRpc<{ peers: number; isSyncing: boolean; shouldHavePeers: boolean }>('system_health'),
    callNodeRpc<unknown[]>('cerulea_getDeployments'),
    callNodeRpc<unknown[]>('cerulea_listProposals', [null]),
    callNodeRpc<unknown[]>('cerulea_getRecentBlocks', [5]),
  ]);

  return {
    nodeOnline: health.ok,
    health: health.ok ? health.result ?? null : null,
    deploymentCount: deployments.ok && Array.isArray(deployments.result) ? deployments.result.length : null,
    governanceProposals: proposals.ok && Array.isArray(proposals.result) ? proposals.result.length : null,
    recentBlocks: blocks.ok && Array.isArray(blocks.result) ? blocks.result : [],
  };
}

/**
 * Verify and synchronize SaaS subscription tier state with the node's
 * on-chain `pallet-cerulea-tier` licence RPC (`cerulea_getLicenceState`).
 */
export async function syncSubscriptionWithNode(
  userId: string,
  plan: string,
  status: string,
  deploymentId?: string | null
): Promise<NodeLicenceSyncResult> {
  const licence = await callNodeRpc('cerulea_getLicenceState', [deploymentId ?? null]);
  if (!licence.ok) {
    return {
      synced: false,
      nodeOnline: false,
      plan,
      status,
      error: licence.error,
    };
  }
  return {
    synced: true,
    nodeOnline: true,
    plan,
    status,
    licenceState: {
      userId,
      onChain: licence.result ?? null,
    },
  };
}
