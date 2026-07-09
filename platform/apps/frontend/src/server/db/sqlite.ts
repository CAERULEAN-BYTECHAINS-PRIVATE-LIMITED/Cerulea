// server-only ensures this never bundles client-side
import 'server-only';
import Database from 'better-sqlite3';
import type { Database as DatabaseInstance } from 'better-sqlite3';
import { existsSync, mkdirSync } from 'fs';
import { dirname, join } from 'path';

declare global {
  // eslint-disable-next-line no-var
  var __CER_DB__: DatabaseInstance | undefined;
}

function ensureDir(p: string) {
  const dir = dirname(p);
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
}

function runMigrations(db: DatabaseInstance) {
  // Foreign keys on
  db.pragma('foreign_keys = ON');
  db.pragma('journal_mode = WAL');
  db.pragma('synchronous = NORMAL');

  // Workspaces
  db.prepare(`
    CREATE TABLE IF NOT EXISTS workspaces (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      slug TEXT,
      createdAt TEXT NOT NULL,
      updatedAt TEXT NOT NULL
    )
  `).run();

  // Projects
  db.prepare(`
    CREATE TABLE IF NOT EXISTS projects (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      slug TEXT,
      description TEXT,
      projectType TEXT NOT NULL CHECK (projectType IN ('dapp','blockchain')),
      templateId TEXT,
      workspaceId TEXT,
      createdAt TEXT NOT NULL,
      updatedAt TEXT NOT NULL,
      FOREIGN KEY (workspaceId) REFERENCES workspaces(id) ON DELETE SET NULL
    )
  `).run();

  // Blueprints (Step 1)
  db.prepare(`
    CREATE TABLE IF NOT EXISTS blueprints (
      projectId TEXT PRIMARY KEY,
      modules TEXT NOT NULL,   -- JSON
      graph   TEXT NOT NULL,   -- JSON
      updatedAt TEXT NOT NULL,
      FOREIGN KEY (projectId) REFERENCES projects(id) ON DELETE CASCADE
    )
  `).run();

  // Schemas (Step 2)
  db.prepare(`
    CREATE TABLE IF NOT EXISTS schemas (
      projectId TEXT PRIMARY KEY,
      schemaJson TEXT NOT NULL,
      updatedAt TEXT NOT NULL,
      FOREIGN KEY (projectId) REFERENCES projects(id) ON DELETE CASCADE
    )
  `).run();

  // Logic (Step 2)
  db.prepare(`
    CREATE TABLE IF NOT EXISTS logic (
      projectId TEXT PRIMARY KEY,
      logicJson TEXT NOT NULL,
      updatedAt TEXT NOT NULL,
      FOREIGN KEY (projectId) REFERENCES projects(id) ON DELETE CASCADE
    )
  `).run();
}

export function getDb(): DatabaseInstance {
  if (globalThis.__CER_DB__) return globalThis.__CER_DB__;
  const dbPath =
    process.env.DB_FILE ||
    join(process.cwd(), 'apps', 'frontend', 'data', 'cerulea.db'); // monorepo-safe default

  ensureDir(dbPath);
  const db = new Database(dbPath, { fileMustExist: false });
  runMigrations(db);
  globalThis.__CER_DB__ = db;
  return db;
}
