// apps/frontend/src/lib/idb.ts
// Minimal IndexedDB wrapper for local-first drafts

type DraftRecord = {
  id: string;            // `${projectId}:${bucket}`
  projectId: string;
  bucket: string;        // e.g. "step1", "uiBuilder"
  data: any;
  updatedAt: number;     // epoch ms
};

const DB_NAME = 'cerulea-studio';
const DB_VERSION = 1;
const STORE = 'drafts';

function withDB<T>(fn: (db: IDBDatabase) => void, onError: (err: any) => void) {
  const req = indexedDB.open(DB_NAME, DB_VERSION);
  req.onupgradeneeded = () => {
    const db = req.result;
    if (!db.objectStoreNames.contains(STORE)) {
      const store = db.createObjectStore(STORE, { keyPath: 'id' });
      store.createIndex('byProject', 'projectId', { unique: false });
    }
  };
  req.onsuccess = () => fn(req.result);
  req.onerror = () => onError(req.error);
}

function runTxn<T>(
  mode: IDBTransactionMode,
  work: (store: IDBObjectStore) => IDBRequest<T>
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    withDB<T>((db) => {
      const tx = db.transaction(STORE, mode);
      const store = tx.objectStore(STORE);
      const req = work(store);
      req.onsuccess = () => resolve(req.result as T);
      req.onerror = () => reject(req.error);
      tx.oncomplete = () => db.close();
      tx.onerror = () => {
        db.close();
        // tx.error might be null; rely on req.onerror above
      };
    }, reject);
  });
}

export async function idbSetDraft(projectId: string, bucket: string, data: any) {
  if (typeof indexedDB === 'undefined') return; // SSR / non-browser guard
  const rec: DraftRecord = {
    id: `${projectId}:${bucket}`,
    projectId,
    bucket,
    data,
    updatedAt: Date.now(),
  };
  await runTxn('readwrite', (store) => store.put(rec));
}

export async function idbGetDraft<T = any>(projectId: string, bucket: string): Promise<T | null> {
  if (typeof indexedDB === 'undefined') return null;
  const rec = (await runTxn('readonly', (store) =>
    store.get(`${projectId}:${bucket}`)
  )) as DraftRecord | undefined;
  return rec ? (rec.data as T) : null;
}

export async function idbDeleteDraft(projectId: string, bucket: string) {
  if (typeof indexedDB === 'undefined') return;
  await runTxn('readwrite', (store) => store.delete(`${projectId}:${bucket}`));
}

export async function idbListBuckets(projectId: string): Promise<string[]> {
  if (typeof indexedDB === 'undefined') return [];
  return new Promise((resolve, reject) => {
    withDB<string[]>((db) => {
      const tx = db.transaction(STORE, 'readonly');
      const idx = tx.objectStore(STORE).index('byProject');
      const req = idx.getAll();
      req.onsuccess = () => {
        const rows = (req.result as DraftRecord[]) || [];
        resolve(rows.map((r) => r.bucket));
        db.close();
      };
      req.onerror = () => {
        reject(req.error);
        db.close();
      };
    }, reject);
  });
}
