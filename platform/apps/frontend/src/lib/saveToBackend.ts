import type { StudioState } from '../context/StudioContext';

export type SaveOptions = {
  step: string;            // e.g., 'step1a', 'step3', etc.
  appId?: string | null;   // if you know the app; can be null for early drafts
  draftId?: string;        // pass the returned id to update the same draft
};

/**
 * Save current Studio state as a per-user draft.
 * Goes through /api rewrite so cookies are included automatically.
 */
export async function saveToBackend(
  state: StudioState,
  { step, appId = null, draftId }: SaveOptions
): Promise<{ ok: true; draft: any } | { ok: false; error: any }> {
  try {
    const res = await fetch('/api/drafts/upsert', {
      method: 'POST',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id: draftId, appId, step, payload: state }),
    });
    if (!res.ok) throw new Error(await res.text());
    const draft = await res.json();
    return { ok: true, draft };
  } catch (error) {
    console.error('[SAVE ERROR]', error);
    return { ok: false, error };
  }
}
