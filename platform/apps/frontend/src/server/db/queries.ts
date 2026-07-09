import 'server-only';
import { getDb } from './sqlite';
import { nanoid } from 'nanoid';
import { z } from 'zod';

const nowIso = () => new Date().toISOString();

/* -------- Workspaces -------- */
export const WorkspaceCreate = z.object({
  name: z.string().min(1),
  slug: z.string().min(1).optional(),
});
export type Workspace = { id: string; name: string; slug: string | null; createdAt: string; updatedAt: string };

export function createWorkspace(input: z.infer<typeof WorkspaceCreate>): Workspace {
  const db = getDb();
  const id = nanoid(12);
  const createdAt = nowIso();
  const updatedAt = createdAt;

  db.prepare(
    `INSERT INTO workspaces (id,name,slug,createdAt,updatedAt) VALUES (@id,@name,@slug,@createdAt,@updatedAt)`
  ).run({ id, name: input.name, slug: input.slug ?? null, createdAt, updatedAt });

  return { id, name: input.name, slug: input.slug ?? null, createdAt, updatedAt };
}

export function listWorkspaces(): Workspace[] {
  const db = getDb();
  return db.prepare(`SELECT id,name,slug,createdAt,updatedAt FROM workspaces ORDER BY createdAt DESC`).all() as Workspace[];
}

/* -------- Projects -------- */
export const ProjectCreate = z.object({
  name: z.string().min(1),
  slug: z.string().optional(),          // you said no strict uniqueness
  description: z.string().optional(),
  projectType: z.enum(['dapp','blockchain']),
  templateId: z.string().optional(),
  workspaceId: z.string().optional().nullable(),
});
export type Project = z.infer<typeof ProjectCreate> & { id: string; createdAt: string; updatedAt: string };

export function createProject(input: z.infer<typeof ProjectCreate>): Project {
  const db = getDb();
  const id = nanoid(12);
  const createdAt = nowIso();
  const updatedAt = createdAt;

  db.prepare(`
    INSERT INTO projects (id,name,slug,description,projectType,templateId,workspaceId,createdAt,updatedAt)
    VALUES (@id,@name,@slug,@description,@projectType,@templateId,@workspaceId,@createdAt,@updatedAt)
  `).run({
    id,
    name: input.name,
    slug: input.slug ?? null,
    description: input.description ?? null,
    projectType: input.projectType,
    templateId: input.templateId ?? null,
    workspaceId: input.workspaceId ?? null,
    createdAt, updatedAt,
  });

  return { id, ...input, createdAt, updatedAt };
}

export function getProject(id: string): Project | null {
  const db = getDb();
  const row = db.prepare(`
    SELECT id,name,slug,description,projectType,templateId,workspaceId,createdAt,updatedAt
    FROM projects WHERE id=@id
  `).get({ id }) as Project | undefined;
  return row ?? null;
}

/* -------- Blueprints (Step 1) -------- */
export type BlueprintPayload = {
  modules: { moduleId: string; label?: string; group?: string; config?: Record<string, any> }[];
  graph: { nodes: any[]; edges: any[] };
};
export function getBlueprint(projectId: string): BlueprintPayload | null {
  const db = getDb();
  const row = db.prepare(`SELECT modules, graph FROM blueprints WHERE projectId=@projectId`).get({ projectId }) as { modules: string; graph: string } | undefined;
  if (!row) return null;
  return { modules: JSON.parse(row.modules), graph: JSON.parse(row.graph) };
}

export function upsertBlueprint(projectId: string, payload: BlueprintPayload) {
  const db = getDb();
  const now = nowIso();
  const existing = db.prepare(`SELECT projectId FROM blueprints WHERE projectId=@projectId`).get({ projectId });

  if (existing) {
    db.prepare(`UPDATE blueprints SET modules=@modules, graph=@graph, updatedAt=@updatedAt WHERE projectId=@projectId`).run({
      projectId, modules: JSON.stringify(payload.modules), graph: JSON.stringify(payload.graph), updatedAt: now,
    });
  } else {
    db.prepare(`INSERT INTO blueprints (projectId, modules, graph, updatedAt) VALUES (@projectId,@modules,@graph,@updatedAt)`).run({
      projectId, modules: JSON.stringify(payload.modules), graph: JSON.stringify(payload.graph), updatedAt: now,
    });
  }
}
