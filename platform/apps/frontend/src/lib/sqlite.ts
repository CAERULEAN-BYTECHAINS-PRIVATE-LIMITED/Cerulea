import Database from 'better-sqlite3';
import fs from 'fs';
import path from 'path';

declare global {
  // eslint-disable-next-line no-var
  var __ceruleaDb: Database.Database | undefined;
}

export function getDb() {
  if (global.__ceruleaDb) return global.__ceruleaDb;

  const dataDir = path.join(process.cwd(), 'var', 'data');
  fs.mkdirSync(dataDir, { recursive: true });
  const dbPath = path.join(dataDir, 'cerulea.db');

  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('synchronous = NORMAL');

  global.__ceruleaDb = db;
  return db;
}
