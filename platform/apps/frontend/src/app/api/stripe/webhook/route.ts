import { NextResponse } from 'next/server';
import Stripe from 'stripe';
import { db } from '@/db/client';
import { subscriptions } from '@/db/schema';
import { eq } from 'drizzle-orm';

export const dynamic = 'force-dynamic';

function getStripe() {
  if (!process.env.STRIPE_SECRET_KEY) throw new Error('Stripe not configured');
  return new Stripe(process.env.STRIPE_SECRET_KEY, { apiVersion: '2026-03-25.dahlia' });
}

function getPlanFromPrice(): Record<string, string> {
  return {
    [process.env.STRIPE_DEVELOPER_PRICE_ID || '__dev__']: 'developer',
    [process.env.STRIPE_PRO_PRICE_ID || '__pro__']: 'pro',
  };
}

async function upsertSubscription(
  userId: string,
  plan: string,
  stripeCustomerId: string,
  stripeSubscriptionId: string,
  status: string,
  currentPeriodEnd: number | null
) {
  const [existing] = await db
    .select({ id: subscriptions.id })
    .from(subscriptions)
    .where(eq(subscriptions.userId as any, userId))
    .limit(1);

  const periodEndIso = currentPeriodEnd ? new Date(currentPeriodEnd * 1000).toISOString() : null;
  const id = existing?.id || crypto.randomUUID();

  if (existing) {
    await db
      .update(subscriptions)
      .set({ plan, stripeCustomerId, stripeSubscriptionId, status, currentPeriodEnd: periodEndIso as any })
      .where(eq(subscriptions.id as any, id));
  } else {
    await db.insert(subscriptions).values({
      id,
      userId,
      plan,
      stripeCustomerId,
      stripeSubscriptionId,
      status,
      currentPeriodEnd: periodEndIso as any,
    });
  }
}

export async function POST(req: Request) {
  if (!process.env.STRIPE_SECRET_KEY) {
    return NextResponse.json({ error: 'Stripe not configured' }, { status: 503 });
  }

  const stripe = getStripe();
  const PLAN_FROM_PRICE = getPlanFromPrice();

  const body = await req.text();
  const sig = req.headers.get('stripe-signature');

  if (!sig || !process.env.STRIPE_WEBHOOK_SECRET) {
    return NextResponse.json({ error: 'Missing signature or webhook secret' }, { status: 400 });
  }

  let event: Stripe.Event;
  try {
    event = stripe.webhooks.constructEvent(body, sig, process.env.STRIPE_WEBHOOK_SECRET);
  } catch (err: any) {
    console.error('Stripe webhook signature error:', err.message);
    return NextResponse.json({ error: `Webhook error: ${err.message}` }, { status: 400 });
  }

  try {
    switch (event.type) {
      case 'checkout.session.completed': {
        const session = event.data.object as Stripe.Checkout.Session;
        const userId = session.metadata?.userId;
        const plan = session.metadata?.plan;

        if (!userId || !plan) break;

        const sub = session.subscription
          ? await stripe.subscriptions.retrieve(session.subscription as string)
          : null;

        await upsertSubscription(
          userId,
          plan,
          session.customer as string,
          session.subscription as string,
          sub?.status ?? 'active',
          sub ? (sub as any).current_period_end : null
        );
        break;
      }

      case 'customer.subscription.updated': {
        const sub = event.data.object as Stripe.Subscription;
        const priceId = sub.items?.data?.[0]?.price?.id;
        const plan = PLAN_FROM_PRICE[priceId] || 'developer';

        // Find userId from existing subscription record by stripeSubscriptionId
        const [existing] = await db
          .select()
          .from(subscriptions)
          .where(eq(subscriptions.stripeSubscriptionId as any, sub.id))
          .limit(1);

        if (existing) {
          await upsertSubscription(
            (existing as any).userId,
            plan,
            sub.customer as string,
            sub.id,
            sub.status,
            (sub as any).current_period_end ?? null
          );
        }
        break;
      }

      case 'customer.subscription.deleted': {
        const sub = event.data.object as Stripe.Subscription;
        await db
          .update(subscriptions)
          .set({ status: 'canceled' })
          .where(eq(subscriptions.stripeSubscriptionId as any, sub.id));
        break;
      }
    }

    return NextResponse.json({ received: true });
  } catch (err: any) {
    console.error('Webhook processing error:', err);
    return NextResponse.json({ error: 'Webhook processing failed' }, { status: 500 });
  }
}
