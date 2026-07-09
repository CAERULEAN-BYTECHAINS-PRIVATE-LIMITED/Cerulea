/**
 * Seed script: creates the test account (test@cerulea.app / test1234)
 * Run with: node scripts/seed-test-account.cjs
 */
const path = require('path');
const Database = require('better-sqlite3');
const bcrypt = require('bcrypt');
const crypto = require('crypto');

const DB_PATH = path.resolve(__dirname, '../.data/cerulea.sqlite');

async function seed() {
  const db = new Database(DB_PATH);
  db.pragma('foreign_keys = ON');

  const email = 'test@cerulea.app';
  const password = 'test1234';
  const name = 'Test Account';

  const existing = db.prepare('SELECT id FROM users WHERE email = ?').get(email);
  if (existing) {
    console.log('Test account already exists:', email);
    db.close();
    return;
  }

  const hashedPassword = await bcrypt.hash(password, 10);
  const userId = crypto.randomUUID();
  const profileId = crypto.randomUUID();

  db.prepare(
    `INSERT INTO users (id, email, hashedPassword, name, isTestAccount, createdAt, updatedAt)
     VALUES (?, ?, ?, ?, 'true', datetime('now'), datetime('now'))`
  ).run(userId, email, hashedPassword, name);

  db.prepare(
    `INSERT INTO profiles (id, userId, displayName, createdAt)
     VALUES (?, ?, ?, datetime('now'))`
  ).run(profileId, userId, name);

  console.log('✅ Test account created:');
  console.log('   Email:', email);
  console.log('   Password: test1234');
  console.log('   User ID:', userId);

  db.close();
}

seed().catch(console.error);
