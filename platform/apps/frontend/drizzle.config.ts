// apps/frontend/drizzle.config.ts
import * as dotenv from 'dotenv';
dotenv.config({ path: '.env.local' }); // load DATABASE_URL from .env.local

import { defineConfig } from 'drizzle-kit';

if (!process.env.DATABASE_URL) {
  throw new Error('DATABASE_URL is missing. Put it in apps/frontend/.env.local');
}

export default defineConfig({
  schema: './db/schema.ts',       // adjust if your schema file lives elsewhere
  out: './drizzle',               // where migrations will be written
  dialect: 'postgresql',
  dbCredentials: {
    url: process.env.DATABASE_URL!,
  },
  verbose: true,
  strict: true,
});
