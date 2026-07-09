// apps/frontend/db/index.ts
import 'dotenv/config';
import { drizzle } from 'drizzle-orm/node-postgres';
import { Pool } from 'pg';

const pool = new Pool({
  connectionString: process.env.DATABASE_URL,
  // ssl: { rejectUnauthorized: false }, // Neon usually works with ?sslmode=require in the URL
});

export const db = drizzle(pool);

// re-export schema tables for convenience
export * from './schema';
export * from "./client";
