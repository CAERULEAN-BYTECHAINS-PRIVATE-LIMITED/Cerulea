export async function api<T = any>(url: string, init: RequestInit = {}) {
  const res = await fetch(url, {
    credentials: "include",
    headers: { "Content-Type": "application/json", ...(init.headers || {}) },
    ...init,
  });

  const contentType = res.headers.get("content-type") || "";
  const isJson = contentType.includes("application/json");

  const payload = isJson ? await res.json().catch(() => null) : await res.text().catch(() => "");
  if (!res.ok) {
    const msg =
      (payload && typeof payload === "object" && (payload.message || payload.error)) ||
      (typeof payload === "string" && payload) ||
      `Request failed (${res.status})`;
    throw new Error(msg);
  }

  return payload as T;
}

export const getApps = () => api("/api/apps");
export const createApp = (name: string) =>
  api("/api/apps", { method: "POST", body: JSON.stringify({ name }) });

export const getDrafts = () => api("/api/drafts");
export const upsertDraft = (input: { id?: string; appId?: string | null; step: string; payload?: any }) =>
  api("/api/drafts/upsert", { method: "POST", body: JSON.stringify(input) });

export const getDeployments = () => api("/api/deployments");
