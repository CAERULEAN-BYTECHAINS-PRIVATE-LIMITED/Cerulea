// apps/frontend/src/lib/auth.ts
import type { NextAuthOptions } from "next-auth";
import CredentialsProvider from "next-auth/providers/credentials";
import { getServerSession } from "next-auth/next";
import { db } from "@/db/client";
import { users, subscriptions } from "@/db/schema";
import { eq } from "drizzle-orm";
import bcrypt from "bcrypt";

export const authOptions: NextAuthOptions = {
  session: { strategy: "jwt" },

  pages: {
    signIn: "/auth/login",
    signOut: "/auth/login",
    error: "/auth/login",
  },

  // Cross-subdomain cookie so studio.* and main domain share the session
  cookies: {
    sessionToken: {
      name: process.env.NODE_ENV === "production"
        ? "__Secure-next-auth.session-token"
        : "next-auth.session-token",
      options: {
        httpOnly: true,
        sameSite: "lax" as const,
        path: "/",
        secure: process.env.NODE_ENV === "production",
        ...(process.env.AUTH_COOKIE_DOMAIN
          ? { domain: process.env.AUTH_COOKIE_DOMAIN }
          : {}),
      },
    },
  },

  providers: [
    CredentialsProvider({
      name: "Email & Password",
      credentials: {
        email: { label: "Email", type: "email" },
        password: { label: "Password", type: "password" },
      },
      async authorize(credentials) {
        if (!credentials?.email || !credentials?.password) return null;

        const [user] = await db
          .select()
          .from(users)
          .where(eq(users.email, credentials.email.toLowerCase().trim()));

        if (!user?.hashedPassword) return null;

        const valid = await bcrypt.compare(credentials.password, user.hashedPassword);
        if (!valid) return null;

        // Fetch subscription plan to embed in token
        const [sub] = await db
          .select({ plan: subscriptions.plan, status: subscriptions.status })
          .from(subscriptions)
          .where(eq(subscriptions.userId, user.id))
          .limit(1);

        const isTest = user.isTestAccount === "true";
        const plan = isTest ? "pro" : (sub?.status === "active" ? sub.plan : "free");

        return {
          id: user.id,
          email: user.email,
          name: user.name ?? null,
          image: null,
          // custom fields — persisted in JWT via jwt callback
          plan,
          isTestAccount: isTest,
        };
      },
    }),
  ],

  callbacks: {
    async jwt({ token, user, trigger }) {
      if (user) {
        token.userId = (user as any).id;
        token.plan = (user as any).plan ?? "free";
        token.isTestAccount = (user as any).isTestAccount ?? false;
      }

      // Re-fetch plan from DB when session is explicitly refreshed (after subscription checkout)
      if (trigger === 'update' && token.userId) {
        try {
          const [sub] = await db
            .select({ plan: subscriptions.plan, status: subscriptions.status })
            .from(subscriptions)
            .where(eq(subscriptions.userId, token.userId as string))
            .limit(1);
          const isTest = token.isTestAccount as boolean;
          token.plan = isTest ? "pro" : (sub?.status === 'active' ? sub.plan : 'free');
        } catch { /* non-fatal — keep existing plan */ }
      }

      return token;
    },
    async session({ session, token }) {
      if (session.user) {
        (session.user as any).id = token.userId as string;
        (session.user as any).plan = token.plan as string;
        (session.user as any).isTestAccount = token.isTestAccount as boolean;
      }
      return session;
    },
  },
};

// Typed helper so we don't repeat authOptions everywhere
export function getSession() {
  return getServerSession(authOptions);
}
