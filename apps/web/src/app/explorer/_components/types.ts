/** Shapes returned by the read-only handlers under `src/app/api/chain/`. */

export interface IndexWindow {
  from: number;
  to: number;
  blocksIndexed: number;
  gapped: boolean;
}

export interface ExplorerExtrinsic {
  index: number;
  hash: string;
  section: string;
  method: string;
}

export interface ExplorerBlock {
  number: number;
  hash: string;
  timestamp: number;
  author: string | null;
  extrinsicCount: number;
  eventCount: number;
  finalized: boolean;
  extrinsics: ExplorerExtrinsic[];
}

export interface BlocksResponse {
  finalizedBlock: number;
  blocks: ExplorerBlock[];
  window: IndexWindow;
}

export interface ExplorerValidator {
  address: string;
  registered: boolean;
  posScore: number | null;
  trustScore: number | null;
  inferenceCount: number;
  stake: string | null;
  blocksAuthoredInWindow: number;
}

export interface ValidatorsResponse {
  validators: ExplorerValidator[];
  consensus: { epoch: number; posWeightBps: number; poiWeightBps: number };
  window: IndexWindow;
}

export interface ExplorerEvent {
  id: string;
  blockNumber: number;
  blockHash: string;
  timestamp: number;
  section: string;
  method: string;
  data: Record<string, unknown>;
  extrinsicIndex: number | null;
  txRef: string | null;
}

export interface EventsResponse {
  events: ExplorerEvent[];
  sections: { section: string; count: number }[];
  pramaanSections: string[];
  window: IndexWindow;
}

export interface SearchResponse {
  found: true;
  kind: 'transaction' | 'block';
  txRef?: string;
  extrinsicIndex?: number;
  extrinsic?: ExplorerExtrinsic | null;
  block: ExplorerBlock;
  events: ExplorerEvent[];
  finalizedBlock: number;
}
