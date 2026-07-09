// apps/frontend/src/db/schema.ts
import { sqliteTable, text } from "drizzle-orm/sqlite-core";
import { sql } from "drizzle-orm";

export const workspaces = sqliteTable("workspaces", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
});

export const projects = sqliteTable("projects", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  slug: text("slug"),
  description: text("description"),
  projectType: text("projectType").notNull(), // 'dapp' | 'blockchain'
  workspaceId: text("workspaceId"),
  userId: text("userId"), // FK to users.id — per-user data isolation
  selectedTemplateIds: text("selectedTemplateIds").$type<unknown>(),
  blueprint: text("blueprint").$type<unknown>(),
  graph: text("graph").$type<unknown>(), // legacy, kept to avoid FK crashes
  schemaJson: text("schemaJson").$type<unknown>(),
  logicJson: text("logicJson").$type<unknown>(),
  economics: text("economics").$type<unknown>(),
  legacyMode: text("legacyMode"), // 'none' | 'connect' | 'port'
  status: text("status").default("draft"),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
  updatedAt: text("updatedAt").default(sql`datetime('now')`).notNull(),
});

export const users = sqliteTable("users", {
  id: text("id").primaryKey(),
  email: text("email").notNull(), // unique enforced at DB init
  hashedPassword: text("hashedPassword"), // nullable for future OAuth
  name: text("name"),
  isTestAccount: text("isTestAccount").default("false"),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
  updatedAt: text("updatedAt").default(sql`datetime('now')`).notNull(),
});

export const profiles = sqliteTable("profiles", {
  id: text("id").primaryKey(),
  userId: text("userId").notNull(),
  displayName: text("displayName"),
  avatarUrl: text("avatarUrl"),
  company: text("company"),
  role: text("role"),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
});

export const verificationTokens = sqliteTable("verificationTokens", {
  id: text("id").primaryKey(),
  identifier: text("identifier").notNull(),
  token: text("token").notNull(),
  expiresAt: text("expiresAt").notNull(),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
});

export const drafts = sqliteTable("drafts", {
  id: text("id").primaryKey(),
  projectId: text("projectId").notNull(),
  data: text("data").$type<unknown>(),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
  updatedAt: text("updatedAt").default(sql`datetime('now')`).notNull(),
});

export const aiThreads = sqliteTable("aiThreads", {
  id: text("id").primaryKey(),
  userId: text("userId"),
  projectId: text("projectId"),
  title: text("title"),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
  updatedAt: text("updatedAt").default(sql`datetime('now')`).notNull(),
});

export const aiMessages = sqliteTable("aiMessages", {
  id: text("id").primaryKey(),
  threadId: text("threadId").notNull(),
  role: text("role").notNull(), // "user" | "assistant" | "system"
  content: text("content").notNull(),
  meta: text("meta").$type<unknown>(),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
});

// ─── New tables ───────────────────────────────────────────────────────────────

export const subscriptions = sqliteTable("subscriptions", {
  id: text("id").primaryKey(),
  userId: text("userId").notNull(),
  plan: text("plan").notNull().default("free"), // free|developer|pro|enterprise
  stripeCustomerId: text("stripeCustomerId"),
  stripeSubscriptionId: text("stripeSubscriptionId"),
  status: text("status").notNull().default("inactive"), // active|inactive|trialing|canceled
  currentPeriodEnd: text("currentPeriodEnd"),
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
  updatedAt: text("updatedAt").default(sql`datetime('now')`).notNull(),
});

export const smartContracts = sqliteTable("smartContracts", {
  id: text("id").primaryKey(),
  projectId: text("projectId").notNull(),
  name: text("name").notNull(),
  contractType: text("contractType").notNull(), // ERC20|ERC721|ERC1155|Governance|Staking|Vault|Bridge|Oracle|Custom
  enabled: text("enabled").notNull().default("true"),
  description: text("description"),
  whyItExists: text("whyItExists"),
  ifDisabled: text("ifDisabled"),
  abi: text("abi").$type<unknown>(),
  bytecode: text("bytecode"),
  dependencies: text("dependencies").$type<unknown>(), // JSON: moduleId[]
  stepSources: text("stepSources").$type<unknown>(), // JSON: which steps define this
  source: text("source").default("module"), // 'module'|'economics'|'entity'
  createdAt: text("createdAt").default(sql`datetime('now')`).notNull(),
  updatedAt: text("updatedAt").default(sql`datetime('now')`).notNull(),
});

export const userPlanSelections = sqliteTable("userPlanSelections", {
  id: text("id").primaryKey(),
  userId: text("userId").notNull(),
  selectedPlan: text("selectedPlan").notNull(), // developer|pro|enterprise
  selectedAt: text("selectedAt").default(sql`datetime('now')`).notNull(),
});
