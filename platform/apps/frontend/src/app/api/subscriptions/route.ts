import { NextResponse } from 'next/server';
import Stripe from 'stripe';
import { db } from '@/db/client';
import { subscriptions } from '@/db/schema';
import { eq } from 'drizzle-orm';
import { getSession } from '@/lib/auth';

export const dynamic = 'force-dynamic';

const stripe = process.env.STRIPE_SECRET_KEY
  ? new Stripe(process.env.STRIPE_SECRET_KEY, { apiVersion: '2026-03-25.dahlia' })
  : null;

export async function GET() {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });

  if (session.user.isTestAccount) {
    return NextResponse.json({
      ok: true,
      subscription: { plan: 'pro', status: 'active', isTestAccount: true },
    });
  }

  const [sub] = await db
    .select()
    .from(subscriptions)
    .where(eq(subscriptions.userId as any, session.user.id))
    .limit(1);

  return NextResponse.json({ ok: true, subscription: sub ?? null });
}

export async function DELETE() {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });

  if (!stripe) return NextResponse.json({ ok: false, error: 'Stripe not configured' }, { status: 503 });

  const [sub] = await db
    .select()
    .from(subscriptions)
    .where(eq(subscriptions.userId as any, session.user.id))
    .limit(1);

  if (!sub || !(sub as any).stripeSubscriptionId) {
    return NextResponse.json({ ok: false, error: 'No active subscription found' }, { status: 404 });
  }

  try {
    await stripe.subscriptions.cancel((sub as any).stripeSubscriptionId);
    await db
      .update(subscriptions)
      .set({ status: 'canceled' })
      .where(eq(subscriptions.userId as any, session.user.id));

    return NextResponse.json({ ok: true, message: 'Subscription canceled.' });
  } catch (err: any) {
    return NextResponse.json({ ok: false, error: err.message }, { status: 500 });
  }
}
