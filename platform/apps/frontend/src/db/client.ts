// apps/frontend/src/db/client.ts
import fs from "fs";
import path from "path";
import Database from "better-sqlite3";
import { drizzle } from "drizzle-orm/better-sqlite3";
import * as schema from "./schema";

const DB_PATH =
  process.env.DRIZZLE_SQLITE_PATH ||
  path.resolve(process.cwd(), ".data", "cerulea.sqlite");

fs.mkdirSync(path.dirname(DB_PATH), { recursive: true });

const sqlite = new Database(DB_PATH);
sqlite.pragma("journal_mode = WAL");
sqlite.pragma("synchronous = NORMAL");
sqlite.pragma("foreign_keys = ON");

// Base tables (correct: IF NOT EXISTS)
sqlite.exec(`
CREATE TABLE IF NOT EXISTS workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  createdAt TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT,
  description TEXT,
  projectType TEXT NOT NULL,              -- 'dapp' | 'blockchain'
  workspaceId TEXT,
  createdAt TEXT NOT NULL DEFAULT (datetime('now')),
  updatedAt TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(workspaceId) REFERENCES workspaces(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_projects_workspace ON projects(workspaceId);
`);

// Auth tables
sqlite.exec(`
CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY,
  email TEXT NOT NULL,
  hashedPassword TEXT,
  name TEXT,
  isTestAccount TEXT DEFAULT 'false',
  createdAt TEXT NOT NULL DEFAULT (datetime('now')),
  updatedAt TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_unique ON users(email);

CREATE TABLE IF NOT EXISTS profiles (
  id TEXT PRIMARY KEY,
  userId TEXT NOT NULL,
  displayName TEXT,
  avatarUrl TEXT,
  company TEXT,
  role TEXT,
  createdAt TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(userId) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_profiles_userId ON profiles(userId);

CREATE TABLE IF NOT EXISTS verificationTokens (
  id TEXT PRIMARY KEY,
  identifier TEXT NOT NULL,
  token TEXT NOT NULL,
  expiresAt TEXT NOT NULL,
  createdAt TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_verification_identifier ON verificationTokens(identifier);
CREATE INDEX IF NOT EXISTS idx_verification_token ON verificationTokens(token);
`);

// Drafts + AI tables
sqlite.exec(`
CREATE TABLE IF NOT EXISTS drafts (
  id TEXT PRIMARY KEY,
  projectId TEXT NOT NULL,
  data TEXT,
  createdAt TEXT NOT NULL DEFAULT (datetime('now')),
  updatedAt TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(projectId) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_drafts_projectId ON drafts(projectId);

CREATE TABLE IF NOT EXISTS aiThreads (
  id TEXT PRIMARY KEY,
  userId TEXT,
  projectId TEXT,
  title TEXT,
  createdAt TEXT NOT NULL DEFAULT (datetime('now')),
  updatedAt TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(userId) REFERENCES users(id) ON DELETE SET NULL,
  FOREIGN KEY(projectId) REFERENCES projects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_aiThreads_userId ON aiThreads(userId);
CREATE INDEX IF NOT EXISTS idx_aiThreads_projectId ON aiThreads(projectId);

CREATE TABLE IF NOT EXISTS aiMessages (
  id TEXT PRIMARY KEY,
  threadId TEXT NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  meta TEXT,
  createdAt TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(threadId) REFERENCES aiThreads(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_aiMessages_threadId ON aiMessages(threadId);
`);

// Ensure columns exist (handles legacy writers and new features)
function ensureColumn(table: string, colName: string, colTypeWithDefault: string) {
  const info = sqlite.prepare(`PRAGMA table_info(${table})`).all() as Array<{ name: string }>;
  const exists = info.some((c) => c.name === colName);
  if (!exists) {
    sqlite.exec(`ALTER TABLE ${table} ADD COLUMN ${colName} ${colTypeWithDefault}`);
  }
}

// Align columns used by the app (legacy + current)
ensureColumn("projects", "selectedTemplateIds", "TEXT");
ensureColumn("projects", "blueprint", "TEXT");
ensureColumn("projects", "graph", "TEXT");
ensureColumn("projects", "schemaJson", "TEXT");
ensureColumn("projects", "logicJson", "TEXT");
ensureColumn("projects", "economics", "TEXT");
ensureColumn("projects", "status", `TEXT DEFAULT 'draft'`);
ensureColumn("projects", "userId", "TEXT");
ensureColumn("projects", "legacyMode", "TEXT DEFAULT 'none'");
// Migrate old passwordHash column → hashedPassword if it exists
{
  const info = sqlite.prepare(`PRAGMA table_info(users)`).all() as Array<{ name: string }>;
  const hasPwdHash = info.some((c) => c.name === "passwordHash");
  const hasHashedPwd = info.some((c) => c.name === "hashedPassword");
  if (hasPwdHash && !hasHashedPwd) {
    sqlite.exec(`ALTER TABLE users ADD COLUMN hashedPassword TEXT`);
    sqlite.exec(`UPDATE users SET hashedPassword = passwordHash`);
  } else if (!hasHashedPwd) {
    sqlite.exec(`ALTER TABLE users ADD COLUMN hashedPassword TEXT`);
  }
}
ensureColumn("users", "isTestAccount", `TEXT DEFAULT 'false'`);
ensureColumn("profiles", "company", "TEXT");
ensureColumn("profiles", "role", "TEXT");

// New tables
sqlite.exec(`
CREATE TABLE IF NOT EXISTS subscriptions (
  id TEXT PRIMARY KEY,
  userId TEXT NOT NULL,
  plan TEXT NOT NULL DEFAULT 'free',
  stripeCustomerId TEXT,
  stripeSubscriptionId TEXT,
  status TEXT NOT NULL DEFAULT 'inactive',
  currentPeriodEnd TEXT,
  createdAt TEXT NOT NULL DEFAULT (datetime('now')),
  updatedAt TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(userId) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_subscriptions_userId ON subscriptions(userId);

CREATE TABLE IF NOT EXISTS smartContracts (
  id TEXT PRIMARY KEY,
  projectId TEXT NOT NULL,
  name TEXT NOT NULL,
  contractType TEXT NOT NULL,
  enabled TEXT NOT NULL DEFAULT 'true',
  description TEXT,
  whyItExists TEXT,
  ifDisabled TEXT,
  abi TEXT,
  bytecode TEXT,
  dependencies TEXT,
  stepSources TEXT,
  source TEXT DEFAULT 'module',
  createdAt TEXT NOT NULL DEFAULT (datetime('now')),
  updatedAt TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(projectId) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_smartContracts_projectId ON smartContracts(projectId);

CREATE TABLE IF NOT EXISTS userPlanSelections (
  id TEXT PRIMARY KEY,
  userId TEXT NOT NULL,
  selectedPlan TEXT NOT NULL,
  selectedAt TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(userId) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_userPlanSelections_userId ON userPlanSelections(userId);
`);

// Backfill status for older rows
sqlite.exec(`UPDATE projects SET status='draft' WHERE status IS NULL`);

// Optional updatedAt trigger
try {
  sqlite.exec(`
    CREATE TRIGGER IF NOT EXISTS trg_projects_updatedAt
    AFTER UPDATE ON projects
    FOR EACH ROW
    BEGIN
      UPDATE projects SET updatedAt = datetime('now') WHERE id = NEW.id;
    END;
  `);
} catch {
  // ignore if not supported
}

// Optional updatedAt triggers for other tables that may use it
try {
  sqlite.exec(`
    CREATE TRIGGER IF NOT EXISTS trg_users_updatedAt
    AFTER UPDATE ON users
    FOR EACH ROW
    BEGIN
      UPDATE users SET updatedAt = datetime('now') WHERE id = NEW.id;
    END;
  `);
  sqlite.exec(`
    CREATE TRIGGER IF NOT EXISTS trg_drafts_updatedAt
    AFTER UPDATE ON drafts
    FOR EACH ROW
    BEGIN
      UPDATE drafts SET updatedAt = datetime('now') WHERE id = NEW.id;
    END;
  `);
  sqlite.exec(`
    CREATE TRIGGER IF NOT EXISTS trg_aiThreads_updatedAt
    AFTER UPDATE ON aiThreads
    FOR EACH ROW
    BEGIN
      UPDATE aiThreads SET updatedAt = datetime('now') WHERE id = NEW.id;
    END;
  `);
} catch {
  // ignore
}

export const db = drizzle(sqlite, { schema });
export { sqlite as rawSqlite, DB_PATH as databasePath };
